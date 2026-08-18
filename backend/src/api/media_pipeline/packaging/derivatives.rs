use super::*;

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

fn make_even_dimension(value: i64) -> i64 {
    let value = value.max(2);
    if value % 2 == 0 { value } else { value - 1 }
}
