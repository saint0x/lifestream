use super::*;

pub(crate) async fn verify_media_integrity(
    input_path: &FsPath,
    media: &ProbedMedia,
) -> AppResult<()> {
    let mut command = Command::new("ffmpeg");
    command.arg("-v").arg("error").arg("-i").arg(input_path);
    if media.has_video {
        command.arg("-map").arg("0:v:0");
    }
    if media.has_audio {
        command.arg("-map").arg("0:a:0");
    }
    command
        .arg("-threads")
        .arg("1")
        .arg("-max_muxing_queue_size")
        .arg("1024")
        .arg("-f")
        .arg("null")
        .arg("-");

    let output = command.output().await?;
    if !output.status.success() {
        return Err(AppError::MediaPipeline(format!(
            "ffmpeg integrity verification failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    Ok(())
}
