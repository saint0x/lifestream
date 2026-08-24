use super::*;

pub(crate) fn validate_probed_media(job: &UploadJob, media: &ProbedMedia) -> AppResult<()> {
    let container = media.container_format.as_deref().ok_or_else(|| {
        AppError::BadRequest("media container could not be identified".to_string())
    })?;
    let container_allowed = container.split(',').map(|value| value.trim()).any(|value| {
        matches!(
            value,
            "mov" | "mp4" | "m4a" | "3gp" | "3g2" | "mj2" | "matroska" | "webm" | "mpegts"
        )
    });
    if !container_allowed {
        return Err(AppError::BadRequest(format!(
            "unsupported media container: {container}"
        )));
    }

    if !media.has_video {
        return Err(AppError::BadRequest(
            "upload must contain at least one video stream".to_string(),
        ));
    }
    if !media.has_audio {
        return Err(AppError::BadRequest(
            "upload must contain at least one audio stream".to_string(),
        ));
    }

    let width = media
        .width
        .ok_or_else(|| AppError::BadRequest("video width could not be determined".to_string()))?;
    let height = media
        .height
        .ok_or_else(|| AppError::BadRequest("video height could not be determined".to_string()))?;
    if width < 144 || height < 144 {
        return Err(AppError::BadRequest(
            "video resolution is below the supported minimum of 144p".to_string(),
        ));
    }
    if width > 7680 || height > 4320 {
        return Err(AppError::BadRequest(
            "video resolution exceeds the supported maximum of 8k".to_string(),
        ));
    }

    let frame_rate = media.frame_rate.ok_or_else(|| {
        AppError::BadRequest("video frame rate could not be determined".to_string())
    })?;
    if !(1.0..=120.0).contains(&frame_rate) {
        return Err(AppError::BadRequest(format!(
            "video frame rate {frame_rate:.2}fps is outside the supported range"
        )));
    }

    if media.duration_sec <= 0.0 {
        return Err(AppError::BadRequest(
            "media duration must be greater than zero".to_string(),
        ));
    }

    let max_duration_sec = match job.kind.as_str() {
        "clip" => 15.0 * 60.0,
        "trailer" => 20.0 * 60.0,
        "episode" => 4.0 * 60.0 * 60.0,
        "film" | "video" | "vod" | "live_archive" => 8.0 * 60.0 * 60.0,
        other => {
            return Err(AppError::BadRequest(format!(
                "unsupported upload job kind: {other}"
            )));
        }
    };
    if media.duration_sec > max_duration_sec {
        return Err(AppError::BadRequest(format!(
            "{} uploads cannot exceed {:.0} minutes",
            job.kind,
            max_duration_sec / 60.0
        )));
    }

    let video_codec = media
        .video_codec
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("video codec could not be determined".to_string()))?;
    if !matches!(
        video_codec,
        "h264" | "hevc" | "vp9" | "av1" | "mpeg4" | "prores"
    ) {
        return Err(AppError::BadRequest(format!(
            "unsupported video codec: {video_codec}"
        )));
    }

    let audio_codec = media
        .audio_codec
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("audio codec could not be determined".to_string()))?;
    if !matches!(
        audio_codec,
        "aac" | "mp3" | "opus" | "flac" | "alac" | "ac3" | "eac3" | "pcm_s16le"
    ) {
        return Err(AppError::BadRequest(format!(
            "unsupported audio codec: {audio_codec}"
        )));
    }

    let audio_sample_rate_hz = media.audio_sample_rate_hz.ok_or_else(|| {
        AppError::BadRequest("audio sample rate could not be determined".to_string())
    })?;
    if !(8_000..=192_000).contains(&audio_sample_rate_hz) {
        return Err(AppError::BadRequest(format!(
            "audio sample rate {audio_sample_rate_hz}Hz is outside the supported range"
        )));
    }

    let audio_channels = media.audio_channels.ok_or_else(|| {
        AppError::BadRequest("audio channel count could not be determined".to_string())
    })?;
    if !(1..=8).contains(&audio_channels) {
        return Err(AppError::BadRequest(format!(
            "audio channel count {audio_channels} is outside the supported range"
        )));
    }

    if let Some(bitrate_bps) = media.bitrate_bps {
        if !(32_000..=200_000_000).contains(&bitrate_bps) {
            return Err(AppError::BadRequest(format!(
                "media bitrate {bitrate_bps}bps is outside the supported range"
            )));
        }
    }

    Ok(())
}

pub(crate) fn classify_media_processing_error(error: &AppError) -> (String, bool) {
    match error {
        AppError::BadRequest(message) => (message.clone(), false),
        AppError::Internal(message) => (
            format!("internal media processing failure: {message}"),
            true,
        ),
        AppError::MediaPipeline(message) => (message.clone(), true),
        AppError::Io(error) => (format!("io failure during media processing: {error}"), true),
        AppError::Serialization(error) => (
            format!("invalid media probe payload during processing: {error}"),
            false,
        ),
        AppError::NotFound => (
            "required media processing resource was not found".to_string(),
            false,
        ),
        AppError::Unauthorized => ("unauthorized media processing attempt".to_string(), false),
        AppError::Forbidden => ("forbidden media processing attempt".to_string(), false),
        AppError::Conflict(message) => (message.clone(), false),
        AppError::PaymentRequired(message) => (message.clone(), false),
        AppError::RateLimited => ("media processing rate limited".to_string(), true),
        AppError::Database(error) => (
            format!("database failure during media processing: {error}"),
            false,
        ),
    }
}
