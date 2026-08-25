use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{fs, process::Command};

#[derive(Debug, Clone)]
pub struct SourceAudioIngestResult {
    pub artifact_path: String,
    pub validation_json: Value,
}

#[derive(Debug, Error)]
pub enum SourceMediaError {
    #[error("invalid media source input: {0}")]
    Invalid(String),
    #[error("media source audio ingest failed: {0}")]
    Ingest(String),
    #[error("media source audio validation failed: {0}")]
    Probe(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub async fn ingest_source_audio(
    source_id: &str,
    input_path: &str,
) -> Result<SourceAudioIngestResult, SourceMediaError> {
    let input_path = allowed_input_path(input_path).await?;
    let input_probe = probe(&input_path).await?;
    let input_audio = audio_stream(&input_probe).ok_or_else(|| {
        SourceMediaError::Invalid(format!("{} has no audio stream", input_path.display()))
    })?;
    let base = media_dir().await?.join("source-audio").join(source_id);
    fs::create_dir_all(&base).await?;
    let artifact_path = base.join(format!(
        "audio-{}.m4a",
        chrono::Utc::now().timestamp_millis()
    ));
    let partial_path = artifact_path.with_extension("partial.m4a");
    remove_if_exists(&artifact_path).await?;
    remove_if_exists(&partial_path).await?;

    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(&input_path)
        .arg("-vn")
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg("160k")
        .arg("-af")
        .arg("aresample=async=1000:first_pts=0")
        .arg(&partial_path)
        .output()
        .await
        .map_err(|error| {
            SourceMediaError::Ingest(format!("could not spawn ffmpeg audio ingest: {error}"))
        })?;
    if !output.status.success() {
        remove_if_exists(&partial_path).await?;
        return Err(SourceMediaError::Ingest(format!(
            "ffmpeg audio ingest exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let output_probe = probe(&partial_path).await?;
    let output_audio = audio_stream(&output_probe).ok_or_else(|| {
        SourceMediaError::Probe("ingested artifact has no audio stream".to_string())
    })?;
    let input_duration = duration_seconds(&input_probe);
    let output_duration = duration_seconds(&output_probe);
    let video_duration = video_stream(&input_probe)
        .and_then(stream_duration_seconds)
        .unwrap_or(0.0);
    let source_audio_duration = stream_duration_seconds(input_audio).unwrap_or(input_duration);
    let drift_ms = if video_duration > 0.0 {
        ((source_audio_duration - video_duration) * 1000.0).abs()
    } else {
        0.0
    };
    let min_duration = (source_audio_duration - 0.35).max(0.0);
    if output_duration < min_duration {
        return Err(SourceMediaError::Probe(format!(
            "ingested audio duration {output_duration:.2}s was shorter than source audio {source_audio_duration:.2}s"
        )));
    }

    let sample_rate = output_audio
        .get("sample_rate")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or_default();
    let channels = output_audio
        .get("channels")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    if sample_rate <= 0 || channels <= 0 {
        return Err(SourceMediaError::Probe(format!(
            "ingested audio validation failed: sample_rate={sample_rate}, channels={channels}"
        )));
    }

    let bytes = fs::read(&partial_path).await?;
    let sha256 = Sha256::digest(&bytes);
    fs::rename(&partial_path, &artifact_path).await?;
    Ok(SourceAudioIngestResult {
        artifact_path: artifact_path.to_string_lossy().to_string(),
        validation_json: json!({
            "playable": true,
            "artifact_kind": "media_source_audio_m4a",
            "source_id": source_id,
            "input_path": input_path,
            "format": "m4a",
            "codec": output_audio.get("codec_name").cloned().unwrap_or_else(|| json!("unknown")),
            "source_audio_duration_seconds": source_audio_duration,
            "source_video_duration_seconds": video_duration,
            "validated_duration_seconds": output_duration,
            "sample_rate": sample_rate,
            "channels": channels,
            "audio_video_drift_ms": drift_ms,
            "drift_status": if drift_ms <= 120.0 { "synced" } else { "warning" },
            "drift_correction_filter": "aresample=async=1000:first_pts=0",
            "drift_correction_active": true,
            "byte_length": bytes.len(),
            "sha256": format!("{sha256:x}"),
            "captured_at": chrono::Utc::now().to_rfc3339(),
            "media_source_audio": true,
            "drift_correction_ready": true,
            "input_probe_format": input_probe.get("format").cloned().unwrap_or_else(|| json!({})),
            "output_probe_format": output_probe.get("format").cloned().unwrap_or_else(|| json!({})),
            "streams": output_probe.get("streams").cloned().unwrap_or_else(|| json!([]))
        }),
    })
}

async fn allowed_input_path(input_path: &str) -> Result<PathBuf, SourceMediaError> {
    if input_path.trim().is_empty() {
        return Err(SourceMediaError::Invalid(
            "input_path must not be empty".to_string(),
        ));
    }
    let raw = PathBuf::from(input_path);
    if !raw.is_absolute() {
        return Err(SourceMediaError::Invalid(
            "input_path must be absolute".to_string(),
        ));
    }
    let input = fs::canonicalize(&raw).await?;
    let media = fs::canonicalize(media_dir().await?).await?;
    if !input.starts_with(&media) {
        return Err(SourceMediaError::Invalid(format!(
            "{} is outside VANTA_OBS_MEDIA_DIR",
            input.display()
        )));
    }
    Ok(input)
}

async fn probe(path: &Path) -> Result<Value, SourceMediaError> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-print_format")
        .arg("json")
        .arg("-show_format")
        .arg("-show_streams")
        .arg(path)
        .output()
        .await
        .map_err(|error| SourceMediaError::Probe(format!("could not spawn ffprobe: {error}")))?;
    if !output.status.success() {
        return Err(SourceMediaError::Probe(format!(
            "ffprobe exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

async fn remove_if_exists(path: &Path) -> Result<(), SourceMediaError> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

async fn media_dir() -> Result<PathBuf, SourceMediaError> {
    let base = std::env::var("VANTA_OBS_MEDIA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("vanta-obs-media"));
    fs::create_dir_all(&base).await?;
    Ok(base)
}

fn audio_stream(probed: &Value) -> Option<&Value> {
    streams(probed)
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("audio"))
}

fn video_stream(probed: &Value) -> Option<&Value> {
    streams(probed)
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("video"))
}

fn streams(probed: &Value) -> &[Value] {
    probed
        .get("streams")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn duration_seconds(probed: &Value) -> f64 {
    probed
        .pointer("/format/duration")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0)
}

fn stream_duration_seconds(stream: &Value) -> Option<f64> {
    stream
        .get("duration")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<f64>().ok())
}
