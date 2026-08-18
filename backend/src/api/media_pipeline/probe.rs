use super::*;

#[derive(Clone, Debug)]
pub(crate) struct ProbedMedia {
    pub(crate) container_format: Option<String>,
    pub(crate) duration_sec: f64,
    pub(crate) width: Option<i64>,
    pub(crate) height: Option<i64>,
    pub(crate) frame_rate: Option<f64>,
    pub(crate) video_codec: Option<String>,
    pub(crate) audio_codec: Option<String>,
    pub(crate) audio_sample_rate_hz: Option<i64>,
    pub(crate) audio_channels: Option<i64>,
    pub(crate) has_video: bool,
    pub(crate) has_audio: bool,
    pub(crate) bitrate_bps: Option<i64>,
    pub(crate) audio_streams: Vec<ProbedAudioStream>,
    pub(crate) subtitle_streams: Vec<ProbedSubtitleStream>,
}

#[derive(Clone, Debug)]
pub(crate) struct ProbedAudioStream {
    pub(crate) stream_index: i64,
    pub(crate) codec: Option<String>,
    pub(crate) language: Option<String>,
    pub(crate) sample_rate_hz: Option<i64>,
    pub(crate) channels: Option<i64>,
}

#[derive(Clone, Debug)]
pub(crate) struct ProbedSubtitleStream {
    pub(crate) stream_index: i64,
    pub(crate) codec: Option<String>,
    pub(crate) language: Option<String>,
}

pub(crate) async fn probe_media(path: &FsPath) -> AppResult<ProbedMedia> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-print_format")
        .arg("json")
        .arg("-show_format")
        .arg("-show_streams")
        .arg(path)
        .output()
        .await?;

    if !output.status.success() {
        return Err(AppError::MediaPipeline(format!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let payload: Value = serde_json::from_slice(&output.stdout)?;
    let format = payload.get("format").cloned().unwrap_or_else(|| json!({}));
    let streams = payload
        .get("streams")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let video_stream = streams.iter().find(|stream| {
        stream
            .get("codec_type")
            .and_then(Value::as_str)
            .map(|codec_type| codec_type == "video")
            .unwrap_or(false)
    });
    let audio_streams = streams
        .iter()
        .filter(|stream| {
            stream
                .get("codec_type")
                .and_then(Value::as_str)
                .map(|codec_type| codec_type == "audio")
                .unwrap_or(false)
        })
        .filter_map(|stream| {
            let stream_index = stream.get("index").and_then(Value::as_i64)?;
            Some(ProbedAudioStream {
                stream_index,
                codec: stream
                    .get("codec_name")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                language: stream
                    .get("tags")
                    .and_then(|tags| tags.get("language"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                sample_rate_hz: stream
                    .get("sample_rate")
                    .and_then(Value::as_str)
                    .and_then(|value| value.parse::<i64>().ok()),
                channels: stream.get("channels").and_then(Value::as_i64),
            })
        })
        .collect::<Vec<_>>();
    let audio_stream = audio_streams.first();
    let subtitle_streams = streams
        .iter()
        .filter(|stream| {
            stream
                .get("codec_type")
                .and_then(Value::as_str)
                .map(|codec_type| codec_type == "subtitle")
                .unwrap_or(false)
        })
        .filter_map(|stream| {
            let stream_index = stream.get("index").and_then(Value::as_i64)?;
            Some(ProbedSubtitleStream {
                stream_index,
                codec: stream
                    .get("codec_name")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                language: stream
                    .get("tags")
                    .and_then(|tags| tags.get("language"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
            })
        })
        .collect::<Vec<_>>();

    Ok(ProbedMedia {
        container_format: format
            .get("format_name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        duration_sec: format
            .get("duration")
            .and_then(Value::as_str)
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(0.0),
        width: video_stream
            .and_then(|stream| stream.get("width"))
            .and_then(Value::as_i64),
        height: video_stream
            .and_then(|stream| stream.get("height"))
            .and_then(Value::as_i64),
        frame_rate: video_stream
            .and_then(|stream| stream.get("avg_frame_rate"))
            .and_then(Value::as_str)
            .and_then(parse_ffprobe_ratio),
        video_codec: video_stream
            .and_then(|stream| stream.get("codec_name"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        audio_codec: audio_stream.and_then(|stream| stream.codec.clone()),
        audio_sample_rate_hz: audio_stream.and_then(|stream| stream.sample_rate_hz),
        audio_channels: audio_stream.and_then(|stream| stream.channels),
        has_video: video_stream.is_some(),
        has_audio: audio_stream.is_some(),
        bitrate_bps: format
            .get("bit_rate")
            .and_then(Value::as_str)
            .and_then(|value| value.parse::<i64>().ok()),
        audio_streams,
        subtitle_streams,
    })
}

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
        AppError::PaymentRequired(message) => (message.clone(), false),
        AppError::RateLimited => ("media processing rate limited".to_string(), true),
        AppError::Database(error) => (
            format!("database failure during media processing: {error}"),
            false,
        ),
    }
}

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
