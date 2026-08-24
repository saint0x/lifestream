use super::fs::{directory_size, make_even_dimension};
use super::*;

pub(crate) const VOD_HLS_VIDEO_CODEC: &str = "libx264";
pub(crate) const VOD_HLS_AUDIO_CODEC: &str = "aac";
pub(crate) const VOD_HLS_AUDIO_BITRATE_BPS: i64 = 128_000;
pub(crate) const VOD_HLS_AUDIO_SAMPLE_RATE_HZ: i64 = 48_000;
pub(crate) const VOD_HLS_AUDIO_CHANNELS: i64 = 2;
pub(crate) const VOD_HLS_SEGMENT_DURATION_SEC: i64 = 6;
const VOD_HLS_GOP_FRAMES: i64 = 48;
const VOD_HLS_LADDER: [(i64, i64, i64, i64); 5] = [
    (426, 240, 700_000, 96_000),
    (640, 360, 1_200_000, 96_000),
    (854, 480, 2_200_000, 128_000),
    (1280, 720, 4_500_000, 128_000),
    (1920, 1080, 8_000_000, 192_000),
];

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
            .arg(VOD_HLS_AUDIO_CODEC)
            .arg("-b:a")
            .arg(format!("{}k", VOD_HLS_AUDIO_BITRATE_BPS / 1000))
            .arg("-ac")
            .arg(VOD_HLS_AUDIO_CHANNELS.to_string())
            .arg("-ar")
            .arg(VOD_HLS_AUDIO_SAMPLE_RATE_HZ.to_string())
            .arg("-f")
            .arg("hls")
            .arg("-hls_time")
            .arg(VOD_HLS_SEGMENT_DURATION_SEC.to_string())
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
            bitrate_bps: VOD_HLS_AUDIO_BITRATE_BPS,
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
                .arg(VOD_HLS_VIDEO_CODEC)
                .arg("-preset")
                .arg("veryfast")
                .arg("-pix_fmt")
                .arg("yuv420p")
                .arg("-g")
                .arg(VOD_HLS_GOP_FRAMES.to_string())
                .arg("-keyint_min")
                .arg(VOD_HLS_GOP_FRAMES.to_string())
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
            .arg(VOD_HLS_SEGMENT_DURATION_SEC.to_string())
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
    let mut planned = Vec::new();
    let mut seen_dimensions = std::collections::HashSet::new();

    for (max_width, max_height, video_bitrate_bps, audio_bitrate_bps) in VOD_HLS_LADDER {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn probed_video(width: i64, height: i64) -> ProbedMedia {
        ProbedMedia {
            container_format: Some("mp4".to_string()),
            duration_sec: 120.0,
            width: Some(width),
            height: Some(height),
            frame_rate: Some(24.0),
            video_codec: Some("h264".to_string()),
            audio_codec: Some("aac".to_string()),
            audio_sample_rate_hz: Some(48_000),
            audio_channels: Some(2),
            has_video: true,
            has_audio: true,
            bitrate_bps: Some(8_000_000),
            audio_streams: Vec::new(),
            subtitle_streams: Vec::new(),
        }
    }

    #[test]
    fn vod_hls_policy_is_standard_hls_h264_aac() {
        assert_eq!(VOD_HLS_VIDEO_CODEC, "libx264");
        assert_eq!(VOD_HLS_AUDIO_CODEC, "aac");
        assert_eq!(VOD_HLS_AUDIO_BITRATE_BPS, 128_000);
        assert_eq!(VOD_HLS_AUDIO_SAMPLE_RATE_HZ, 48_000);
        assert_eq!(VOD_HLS_AUDIO_CHANNELS, 2);
        assert_eq!(VOD_HLS_SEGMENT_DURATION_SEC, 6);
    }

    #[test]
    fn vod_hls_ladder_caps_at_source_resolution() {
        let variants = plan_hls_variants(&probed_video(1280, 720)).expect("variants");

        assert_eq!(
            variants
                .iter()
                .map(|variant| variant.label.as_str())
                .collect::<Vec<_>>(),
            vec!["240p", "360p", "480p", "720p"]
        );
        assert_eq!(variants.last().expect("last").video_bitrate_bps, 4_500_000);
    }
}
