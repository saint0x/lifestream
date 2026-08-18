use super::*;

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
