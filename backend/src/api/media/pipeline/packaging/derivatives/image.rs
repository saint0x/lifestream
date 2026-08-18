use super::*;

fn capture_offset(duration_sec: f64) -> &'static str {
    if duration_sec >= 5.0 {
        "00:00:05"
    } else {
        "00:00:00"
    }
}

pub(crate) async fn generate_poster(
    input_path: &FsPath,
    output_path: &FsPath,
    duration_sec: f64,
) -> AppResult<()> {
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-ss")
        .arg(capture_offset(duration_sec))
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
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-ss")
        .arg(capture_offset(duration_sec))
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
