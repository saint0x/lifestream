use super::assets::NewMediaPreviewTrack;
use super::probe::ProbedSubtitleStream;
use super::*;

#[derive(Clone, Debug)]
pub(crate) struct ImageDerivativePlan {
    pub(crate) label: &'static str,
    pub(crate) max_width: i64,
    pub(crate) max_height: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct HlsVariantPlan {
    pub(crate) label: String,
    pub(crate) width: i64,
    pub(crate) height: i64,
    pub(crate) video_bitrate_bps: i64,
    pub(crate) bandwidth_bps: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct GeneratedHlsVariant {
    pub(crate) plan: HlsVariantPlan,
    pub(crate) relative_playlist_path: String,
    pub(crate) file_size_bytes: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct GeneratedHlsSubtitleTrack {
    pub(crate) relative_path: String,
    pub(crate) language: String,
    pub(crate) name: String,
    pub(crate) is_default: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct GeneratedHlsAudioTrack {
    pub(crate) label: String,
    pub(crate) language: String,
    pub(crate) codec: String,
    pub(crate) bitrate_bps: i64,
    pub(crate) relative_playlist_path: String,
    pub(crate) file_size_bytes: i64,
    pub(crate) is_default: bool,
    pub(crate) is_dubbed: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct GeneratedHlsPackage {
    pub(crate) master_relative_path: String,
    pub(crate) variants: Vec<GeneratedHlsVariant>,
    pub(crate) audio_tracks: Vec<GeneratedHlsAudioTrack>,
    pub(crate) subtitle_tracks: Vec<GeneratedHlsSubtitleTrack>,
}

pub(crate) async fn generate_poster(
    input_path: &FsPath,
    output_path: &FsPath,
    duration_sec: f64,
) -> AppResult<()> {
    let capture_offset = if duration_sec >= 5.0 {
        "00:00:05"
    } else {
        "00:00:00"
    };
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-ss")
        .arg(capture_offset)
        .arg("-i")
        .arg(input_path)
        .arg("-frames:v")
        .arg("1")
        .arg("-q:v")
        .arg("2")
        .arg(output_path)
        .output()
        .await?;

    if !output.status.success() {
        return Err(AppError::MediaPipeline(format!(
            "ffmpeg poster generation failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    Ok(())
}

pub(crate) async fn generate_thumbnail(
    input_path: &FsPath,
    output_path: &FsPath,
    duration_sec: f64,
    width: i64,
    height: i64,
) -> AppResult<()> {
    let capture_offset = if duration_sec >= 5.0 {
        "00:00:05"
    } else {
        "00:00:00"
    };
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-ss")
        .arg(capture_offset)
        .arg("-i")
        .arg(input_path)
        .arg("-frames:v")
        .arg("1")
        .arg("-vf")
        .arg(format!("scale={width}:{height}"))
        .arg("-q:v")
        .arg("3")
        .arg(output_path)
        .output()
        .await?;

    if !output.status.success() {
        return Err(AppError::MediaPipeline(format!(
            "ffmpeg thumbnail generation failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    Ok(())
}

fn build_timeline_preview_timestamps(duration_sec: f64) -> Vec<f64> {
    const MAX_PREVIEW_FRAMES: usize = 60;
    const MIN_PREVIEW_INTERVAL_SEC: f64 = 5.0;

    if duration_sec <= 0.0 {
        return vec![0.0];
    }

    let interval_sec = (duration_sec / MAX_PREVIEW_FRAMES as f64).max(MIN_PREVIEW_INTERVAL_SEC);
    let mut timestamps = Vec::new();
    let mut cursor = 0.0_f64;
    while cursor < duration_sec && timestamps.len() < MAX_PREVIEW_FRAMES {
        timestamps.push(cursor);
        cursor += interval_sec;
    }
    if timestamps.is_empty() {
        timestamps.push(0.0);
    }
    timestamps
}

pub(crate) fn format_webvtt_timestamp(timestamp_sec: f64) -> String {
    let total_millis = (timestamp_sec.max(0.0) * 1000.0).round() as i64;
    let hours = total_millis / 3_600_000;
    let minutes = (total_millis % 3_600_000) / 60_000;
    let seconds = (total_millis % 60_000) / 1000;
    let millis = total_millis % 1000;
    format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}")
}

pub(crate) async fn generate_timeline_preview_track(
    input_path: &FsPath,
    image_output_path: &FsPath,
    vtt_output_path: &FsPath,
    image_relative_path: &str,
    vtt_relative_path: &str,
    duration_sec: f64,
    source_width: i64,
    source_height: i64,
) -> AppResult<NewMediaPreviewTrack> {
    let timestamps = build_timeline_preview_timestamps(duration_sec);
    let frame_count = timestamps.len() as i64;
    let columns_count = (timestamps.len().min(10).max(1)) as i64;
    let rows_count = ((timestamps.len() as i64) + columns_count - 1) / columns_count;
    let interval_sec = if timestamps.len() > 1 {
        timestamps[1] - timestamps[0]
    } else {
        duration_sec.max(1.0)
    };
    let (tile_width, tile_height) =
        scaled_dimensions_for_rung(source_width, source_height, 320, 180);

    let fps_denominator = interval_sec.max(0.001);
    let sprite_output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(input_path)
        .arg("-frames:v")
        .arg("1")
        .arg("-vf")
        .arg(format!(
            "fps=1/{fps_denominator:.6},scale={tile_width}:{tile_height}:force_original_aspect_ratio=decrease:force_divisible_by=2,pad={tile_width}:{tile_height}:(ow-iw)/2:(oh-ih)/2,tile={}x{}",
            columns_count, rows_count
        ))
        .arg("-q:v")
        .arg("4")
        .arg(image_output_path)
        .output()
        .await?;

    if !sprite_output.status.success() {
        return Err(AppError::MediaPipeline(format!(
            "ffmpeg timeline preview generation failed: {}",
            String::from_utf8_lossy(&sprite_output.stderr).trim()
        )));
    }

    let image_name = PathBuf::from(image_relative_path)
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .ok_or_else(|| {
            AppError::MediaPipeline("invalid timeline preview image path".to_string())
        })?;
    let mut vtt = String::from("WEBVTT\n\n");
    for (index, start_sec) in timestamps.iter().enumerate() {
        let end_sec = timestamps
            .get(index + 1)
            .copied()
            .unwrap_or(duration_sec.max(*start_sec + 0.001));
        let end_sec = end_sec.max(*start_sec + 0.001);
        let column = (index as i64) % columns_count;
        let row = (index as i64) / columns_count;
        let x = column * tile_width;
        let y = row * tile_height;
        vtt.push_str(&format!(
            "{} --> {}\n{}#xywh={},{},{},{}\n\n",
            format_webvtt_timestamp(*start_sec),
            format_webvtt_timestamp(end_sec),
            image_name,
            x,
            y,
            tile_width,
            tile_height
        ));
    }
    tokio::fs::write(vtt_output_path, vtt).await?;

    Ok(NewMediaPreviewTrack {
        label: "timeline_preview".to_string(),
        image_relative_path: image_relative_path.to_string(),
        vtt_relative_path: vtt_relative_path.to_string(),
        tile_width,
        tile_height,
        columns_count,
        rows_count,
        interval_sec,
        frame_count,
        is_default: true,
    })
}

pub(crate) async fn extract_subtitle_stream_to_webvtt(
    input_path: &FsPath,
    stream: &ProbedSubtitleStream,
    output_path: &FsPath,
) -> AppResult<()> {
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(input_path)
        .arg("-map")
        .arg(format!("0:{}", stream.stream_index))
        .arg("-c:s")
        .arg("webvtt")
        .arg("-f")
        .arg("webvtt")
        .arg(output_path)
        .output()
        .await?;

    if !output.status.success() {
        return Err(AppError::MediaPipeline(format!(
            "ffmpeg subtitle normalization failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    Ok(())
}

pub(crate) fn subtitle_codec_supported_for_normalization(codec: Option<&str>) -> bool {
    matches!(
        codec,
        Some("subrip" | "webvtt" | "mov_text" | "ass" | "ssa")
    )
}

pub(crate) fn build_image_derivative_plans(
    media: &ProbedMedia,
) -> AppResult<Vec<ImageDerivativePlan>> {
    let width = media
        .width
        .ok_or_else(|| AppError::BadRequest("video width could not be determined".to_string()))?;
    let height = media
        .height
        .ok_or_else(|| AppError::BadRequest("video height could not be determined".to_string()))?;
    let mut plans = Vec::new();

    for candidate in [
        ImageDerivativePlan {
            label: "card_thumbnail",
            max_width: 640,
            max_height: 360,
        },
        ImageDerivativePlan {
            label: "player_thumbnail",
            max_width: 1280,
            max_height: 720,
        },
    ] {
        let candidate_dimensions =
            scaled_dimensions_for_rung(width, height, candidate.max_width, candidate.max_height);
        if candidate_dimensions.0 < 144 || candidate_dimensions.1 < 144 {
            continue;
        }
        if plans.iter().any(|plan: &ImageDerivativePlan| {
            scaled_dimensions_for_rung(width, height, plan.max_width, plan.max_height)
                == candidate_dimensions
        }) {
            continue;
        }
        plans.push(candidate);
    }

    Ok(plans)
}

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

pub(crate) fn scaled_dimensions_for_rung(
    source_width: i64,
    source_height: i64,
    max_width: i64,
    max_height: i64,
) -> (i64, i64) {
    let width_ratio = max_width as f64 / source_width as f64;
    let height_ratio = max_height as f64 / source_height as f64;
    let scale = width_ratio.min(height_ratio).min(1.0);

    let scaled_width = make_even_dimension(((source_width as f64) * scale).round() as i64);
    let scaled_height = make_even_dimension(((source_height as f64) * scale).round() as i64);
    (scaled_width.max(2), scaled_height.max(2))
}

pub(crate) fn make_even_dimension(value: i64) -> i64 {
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

pub(crate) async fn validate_generated_hls_package(
    master_path: &FsPath,
    package: &GeneratedHlsPackage,
) -> AppResult<()> {
    if package.variants.is_empty() {
        return Err(AppError::MediaPipeline(
            "generated HLS package did not produce any playback variants".to_string(),
        ));
    }

    let master_body = tokio::fs::read_to_string(master_path).await?;
    let stream_inf_count = master_body
        .lines()
        .filter(|line| line.starts_with("#EXT-X-STREAM-INF"))
        .count();
    if stream_inf_count != package.variants.len() {
        return Err(AppError::MediaPipeline(format!(
            "generated HLS master manifest expected {} stream entries but found {}",
            package.variants.len(),
            stream_inf_count
        )));
    }

    let master_dir = master_path.parent().ok_or_else(|| {
        AppError::MediaPipeline("generated HLS master manifest has no parent directory".to_string())
    })?;
    let subtitle_media_lines = master_body
        .lines()
        .filter(|line| line.starts_with("#EXT-X-MEDIA:TYPE=SUBTITLES"))
        .collect::<Vec<_>>();
    let audio_media_lines = master_body
        .lines()
        .filter(|line| line.starts_with("#EXT-X-MEDIA:TYPE=AUDIO"))
        .collect::<Vec<_>>();
    if audio_media_lines.len() != package.audio_tracks.len() {
        return Err(AppError::MediaPipeline(format!(
            "generated HLS master manifest expected {} audio track entries but found {}",
            package.audio_tracks.len(),
            audio_media_lines.len()
        )));
    }
    if subtitle_media_lines.len() != package.subtitle_tracks.len() {
        return Err(AppError::MediaPipeline(format!(
            "generated HLS master manifest expected {} subtitle track entries but found {}",
            package.subtitle_tracks.len(),
            subtitle_media_lines.len()
        )));
    }
    let listed_variant_paths = master_body
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect::<Vec<_>>();
    if listed_variant_paths.len() != package.variants.len() {
        return Err(AppError::MediaPipeline(format!(
            "generated HLS master manifest expected {} variant playlist paths but found {}",
            package.variants.len(),
            listed_variant_paths.len()
        )));
    }

    for variant in &package.variants {
        if !listed_variant_paths
            .iter()
            .any(|path| path == &variant.relative_playlist_path)
        {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS master manifest is missing variant playlist {}",
                variant.relative_playlist_path
            )));
        }

        let playlist_path = master_dir.join(&variant.relative_playlist_path);
        let playlist_body = tokio::fs::read_to_string(&playlist_path).await?;
        if !playlist_body.contains("#EXTM3U") {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS variant playlist {} is missing EXTM3U header",
                variant.relative_playlist_path
            )));
        }
        if !playlist_body.contains("#EXTINF") {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS variant playlist {} does not contain media segments",
                variant.relative_playlist_path
            )));
        }

        let playlist_dir = playlist_path.parent().ok_or_else(|| {
            AppError::MediaPipeline(format!(
                "generated HLS variant playlist {} has no parent directory",
                variant.relative_playlist_path
            ))
        })?;
        let segment_paths = playlist_body
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect::<Vec<_>>();
        if segment_paths.is_empty() {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS variant playlist {} does not list any media segments",
                variant.relative_playlist_path
            )));
        }
        for segment in segment_paths {
            let segment_path = playlist_dir.join(segment);
            let metadata = tokio::fs::metadata(&segment_path).await?;
            if !metadata.is_file() || metadata.len() == 0 {
                return Err(AppError::MediaPipeline(format!(
                    "generated HLS segment {} is missing or empty",
                    segment_path.display()
                )));
            }
        }
    }

    for track in &package.audio_tracks {
        if !master_body.contains(&format!("URI=\"{}\"", track.relative_playlist_path)) {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS master manifest is missing audio track {}",
                track.relative_playlist_path
            )));
        }
        let playlist_path = master_dir.join(&track.relative_playlist_path);
        let playlist_body = tokio::fs::read_to_string(&playlist_path).await?;
        if !playlist_body.contains("#EXTM3U") || !playlist_body.contains("#EXTINF") {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS audio playlist {} is incomplete",
                track.relative_playlist_path
            )));
        }
    }

    for track in &package.subtitle_tracks {
        if !master_body.contains(&format!("URI=\"{}\"", track.relative_path)) {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS master manifest is missing subtitle track {}",
                track.relative_path
            )));
        }
        let subtitle_path = master_dir.join(&track.relative_path);
        let subtitle_body = tokio::fs::read_to_string(&subtitle_path).await?;
        if !subtitle_body.starts_with("WEBVTT") {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS subtitle track {} is missing WEBVTT header",
                track.relative_path
            )));
        }
    }

    Ok(())
}
