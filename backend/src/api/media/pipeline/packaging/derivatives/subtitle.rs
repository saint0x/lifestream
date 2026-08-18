use super::*;

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
