use super::*;

pub(crate) async fn generate_hls(
    input_path: &FsPath,
    output_path: &FsPath,
    media: &ProbedMedia,
    subtitle_tracks: &[GeneratedHlsSubtitleTrack],
) -> AppResult<GeneratedHlsPackage> {
    if let Some(parent) = output_path.parent() {
        if tokio::fs::try_exists(parent).await? {
            let _ = tokio::fs::remove_dir_all(parent).await;
        }
        tokio::fs::create_dir_all(parent).await?;
    }

    let output_dir = output_path
        .parent()
        .ok_or_else(|| AppError::MediaPipeline("invalid playback output directory".to_string()))?;
    let plans = plan_hls_variants(media)?;
    let mut variants = Vec::with_capacity(plans.len());
    let mut audio_tracks = Vec::with_capacity(media.audio_streams.len().max(1));

    for (ordinal, stream) in media.audio_streams.iter().enumerate() {
        let language = stream.language.clone().unwrap_or_else(|| "und".to_string());
        let label = if ordinal == 0 {
            format!("audio-{language}")
        } else {
            format!("audio-{language}-{}", ordinal + 1)
        };
        let track_dir = output_dir.join(&label);
        tokio::fs::create_dir_all(&track_dir).await?;
        let playlist_path = track_dir.join("playlist.m3u8");
        let segment_pattern = track_dir.join("segment_%03d.aac");

        let output = Command::new("ffmpeg")
            .arg("-y")
            .arg("-i")
            .arg(input_path)
            .arg("-map")
            .arg(format!("0:{}", stream.stream_index))
            .arg("-vn")
            .arg("-c:a")
            .arg("aac")
            .arg("-b:a")
            .arg("128k")
            .arg("-ac")
            .arg("2")
            .arg("-ar")
            .arg("48000")
            .arg("-f")
            .arg("hls")
            .arg("-hls_time")
            .arg("6")
            .arg("-hls_playlist_type")
            .arg("vod")
            .arg("-hls_segment_filename")
            .arg(&segment_pattern)
            .arg(&playlist_path)
            .output()
            .await?;
        if !output.status.success() {
            return Err(AppError::MediaPipeline(format!(
                "ffmpeg audio hls packaging failed for {}: {}",
                label,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        audio_tracks.push(GeneratedHlsAudioTrack {
            label: label.clone(),
            language: language.clone(),
            codec: "aac".to_string(),
            bitrate_bps: 128_000,
            relative_playlist_path: format!("{label}/playlist.m3u8"),
            file_size_bytes: directory_size(&track_dir).await?,
            is_default: ordinal == 0,
            is_dubbed: ordinal > 0
                && language
                    != media.audio_streams[0]
                        .language
                        .clone()
                        .unwrap_or_else(|| "und".to_string()),
        });
    }

    for plan in &plans {
        let variant_dir = output_dir.join(&plan.label);
        tokio::fs::create_dir_all(&variant_dir).await?;
        let playlist_path = variant_dir.join("playlist.m3u8");
        let segment_pattern = variant_dir.join("segment_%03d.ts");

        let mut command = Command::new("ffmpeg");
        command
            .arg("-y")
            .arg("-i")
            .arg(input_path)
            .arg("-map")
            .arg("0:v:0");

        if media.has_video {
            command
                .arg("-c:v")
                .arg("libx264")
                .arg("-preset")
                .arg("veryfast")
                .arg("-pix_fmt")
                .arg("yuv420p")
                .arg("-g")
                .arg("48")
                .arg("-keyint_min")
                .arg("48")
                .arg("-sc_threshold")
                .arg("0")
                .arg("-vf")
                .arg(format!("scale={}:{}", plan.width, plan.height))
                .arg("-maxrate")
                .arg(format!("{}k", (plan.video_bitrate_bps / 1000).max(300)))
                .arg("-bufsize")
                .arg(format!(
                    "{}k",
                    ((plan.video_bitrate_bps * 2) / 1000).max(600)
                ))
                .arg("-an");
        } else {
            command.arg("-vn");
        }

        command
            .arg("-f")
            .arg("hls")
            .arg("-hls_time")
            .arg("6")
            .arg("-hls_playlist_type")
            .arg("vod")
            .arg("-hls_segment_filename")
            .arg(&segment_pattern)
            .arg(&playlist_path);

        let output = command.output().await?;
        if !output.status.success() {
            return Err(AppError::MediaPipeline(format!(
                "ffmpeg hls packaging failed for {}: {}",
                plan.label,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        variants.push(GeneratedHlsVariant {
            plan: plan.clone(),
            relative_playlist_path: format!("{}/playlist.m3u8", plan.label),
            file_size_bytes: directory_size(&variant_dir).await?,
        });
    }

    write_hls_master_manifest(output_path, &variants, &audio_tracks, subtitle_tracks).await?;
    let package = GeneratedHlsPackage {
        master_relative_path: output_path.to_string_lossy().to_string(),
        variants,
        audio_tracks,
        subtitle_tracks: subtitle_tracks.to_vec(),
    };
    validate_generated_hls_package(output_path, &package).await?;
    Ok(package)
}

pub(crate) fn plan_hls_variants(media: &ProbedMedia) -> AppResult<Vec<HlsVariantPlan>> {
    let width = media
        .width
        .ok_or_else(|| AppError::BadRequest("video width could not be determined".to_string()))?;
    let height = media
        .height
        .ok_or_else(|| AppError::BadRequest("video height could not be determined".to_string()))?;
    let ladder = [
        (426_i64, 240_i64, 700_000_i64, 96_000_i64),
        (640, 360, 1_200_000, 96_000),
        (854, 480, 2_200_000, 128_000),
        (1280, 720, 4_500_000, 128_000),
        (1920, 1080, 8_000_000, 192_000),
    ];
    let mut planned = Vec::new();
    let mut seen_dimensions = std::collections::HashSet::new();

    for (max_width, max_height, video_bitrate_bps, audio_bitrate_bps) in ladder {
        let (scaled_width, scaled_height) =
            scaled_dimensions_for_rung(width, height, max_width, max_height);
        if scaled_width < 144 || scaled_height < 144 {
            continue;
        }
        if !seen_dimensions.insert((scaled_width, scaled_height)) {
            continue;
        }
        planned.push(HlsVariantPlan {
            label: format!("{}p", scaled_height),
            width: scaled_width,
            height: scaled_height,
            video_bitrate_bps,
            bandwidth_bps: video_bitrate_bps + audio_bitrate_bps,
        });
    }

    if planned.is_empty() {
        let fallback_width = make_even_dimension(width.max(144));
        let fallback_height = make_even_dimension(height.max(144));
        planned.push(HlsVariantPlan {
            label: format!("{}p", fallback_height),
            width: fallback_width,
            height: fallback_height,
            video_bitrate_bps: 1_200_000,
            bandwidth_bps: 1_296_000,
        });
    }

    Ok(planned)
}

pub(crate) async fn write_hls_master_manifest(
    output_path: &FsPath,
    variants: &[GeneratedHlsVariant],
    audio_tracks: &[GeneratedHlsAudioTrack],
    subtitle_tracks: &[GeneratedHlsSubtitleTrack],
) -> AppResult<()> {
    let mut body = String::from("#EXTM3U\n#EXT-X-VERSION:3\n");
    for track in audio_tracks {
        body.push_str(&format!(
            "#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"audio\",NAME=\"{}\",LANGUAGE=\"{}\",AUTOSELECT=YES,DEFAULT={},URI=\"{}\"\n",
            track.label,
            track.language,
            if track.is_default { "YES" } else { "NO" },
            track.relative_playlist_path
        ));
    }
    for track in subtitle_tracks {
        body.push_str(&format!(
            "#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"captions\",NAME=\"{}\",LANGUAGE=\"{}\",AUTOSELECT=YES,DEFAULT={},FORCED=NO,URI=\"{}\"\n",
            track.name,
            track.language,
            if track.is_default { "YES" } else { "NO" },
            track.relative_path
        ));
    }
    for variant in variants {
        let codecs = "avc1.64001f,mp4a.40.2";
        body.push_str(&format!(
            "#EXT-X-STREAM-INF:BANDWIDTH={},AVERAGE-BANDWIDTH={},RESOLUTION={}x{},CODECS=\"{}\"{}{}\n{}\n",
            variant.plan.bandwidth_bps,
            variant.plan.bandwidth_bps,
            variant.plan.width,
            variant.plan.height,
            codecs,
            if audio_tracks.is_empty() {
                String::new()
            } else {
                ",AUDIO=\"audio\"".to_string()
            },
            if subtitle_tracks.is_empty() {
                String::new()
            } else {
                ",SUBTITLES=\"captions\"".to_string()
            },
            variant.relative_playlist_path
        ));
    }
    tokio::fs::write(output_path, body).await?;
    Ok(())
}

fn make_even_dimension(value: i64) -> i64 {
    let value = value.max(2);
    if value % 2 == 0 { value } else { value - 1 }
}

async fn directory_size(path: &FsPath) -> AppResult<i64> {
    let mut total = 0_i64;
    let mut entries = tokio::fs::read_dir(path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.metadata().await?;
        if metadata.is_file() {
            total += metadata.len() as i64;
        }
    }
    Ok(total)
}
