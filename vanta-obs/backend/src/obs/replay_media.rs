use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{fs, process::Command};

#[derive(Debug, Clone)]
pub struct ReplayClipRequest {
    pub marker_id: String,
    pub broadcast_id: String,
    pub media_asset_id: String,
    pub duration_seconds: i64,
    pub sponsor_proof: bool,
    pub source: Option<ReplayMediaSource>,
}

#[derive(Debug, Clone)]
pub struct ReplayClip {
    pub media_asset_id: String,
    pub output_path: String,
    pub manifest_json: Value,
    pub pressure_json: Value,
    pub buffer_json: Value,
    pub upload_queue_json: Value,
    pub asset_json: Value,
    pub segments: Vec<ReplayBufferSegment>,
}

#[derive(Debug, Clone)]
pub struct ReplayMediaSource {
    pub source_kind: String,
    pub source_path: String,
    pub source_id: Option<String>,
    pub metadata_json: Value,
}

#[derive(Debug, Clone)]
pub struct ReplayBufferSegment {
    pub id: String,
    pub segment_index: i64,
    pub duration_seconds: i64,
    pub artifact_path: String,
    pub validation_json: Value,
    pub pressure_json: Value,
}

#[derive(Debug, Error)]
pub enum ReplayMediaError {
    #[error("replay clip generation failed: {0}")]
    Render(String),
    #[error("replay validation failed: {0}")]
    Probe(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Default, Clone)]
pub struct ReplayMediaEngine;

impl ReplayMediaEngine {
    pub async fn save_clip(
        &self,
        input: ReplayClipRequest,
    ) -> Result<ReplayClip, ReplayMediaError> {
        let base = replay_dir(&input.broadcast_id).await?;
        let output_path = base.join(format!("{}.mp4", input.marker_id));
        let partial_path = base.join(format!("{}.partial.mp4", input.marker_id));
        let concat_path = base.join(format!("{}.concat.txt", input.marker_id));
        remove_if_exists(&output_path).await?;
        remove_if_exists(&partial_path).await?;
        remove_if_exists(&concat_path).await?;

        let (buffer, selected, source_json) = if let Some(source) = input
            .source
            .clone()
            .filter(|source| Path::new(&source.source_path).is_file())
        {
            render_clip_from_source(&source, &partial_path, input.duration_seconds).await?;
            let source_validation = probe(Path::new(&source.source_path)).await?;
            let source_metadata = fs::metadata(&source.source_path).await?;
            let segment = ReplayBufferSegment {
                id: format!("source_{}", input.marker_id),
                segment_index: 0,
                duration_seconds: input.duration_seconds,
                artifact_path: source.source_path.clone(),
                validation_json: merge_json(
                    source_validation.clone(),
                    json!({
                        "native_live_source": true,
                        "source_kind": source.source_kind,
                        "source_id": source.source_id,
                        "duration_seconds": input.duration_seconds,
                        "byte_length": source_metadata.len(),
                        "sha256": sha256_file(Path::new(&source.source_path)).await?
                    }),
                ),
                pressure_json: pressure_json(
                    0,
                    source_metadata.len(),
                    memory_pressure_json(input.duration_seconds, 1),
                )
                .await?,
            };
            (
                vec![segment.clone()],
                vec![segment],
                json!({
                    "kind": source.source_kind,
                    "mode": "native_live_media",
                    "path": source.source_path,
                    "source_id": source.source_id,
                    "metadata_json": source.metadata_json,
                    "validation": source_validation
                }),
            )
        } else {
            let buffer = ensure_buffer_segments(&base, &input).await?;
            let selected = select_segments(&buffer, input.duration_seconds);
            write_concat_file(&concat_path, &selected).await?;
            render_clip_from_concat(&concat_path, &partial_path, input.duration_seconds).await?;
            remove_if_exists(&concat_path).await?;
            (
                buffer,
                selected,
                json!({
                    "kind": "generated_runtime_fallback",
                    "mode": "synthetic_ring",
                    "path": null,
                    "source_id": null,
                    "metadata_json": {
                        "reason": "no active native, runtime, or recording media source was available"
                    }
                }),
            )
        };

        let validation = probe(&partial_path).await?;
        fs::rename(&partial_path, &output_path).await?;
        let metadata = fs::metadata(&output_path).await?;
        let buffer_bytes = buffer
            .iter()
            .filter_map(|segment| {
                segment
                    .validation_json
                    .get("byte_length")
                    .and_then(Value::as_u64)
            })
            .sum::<u64>();
        let memory = memory_pressure_json(input.duration_seconds, selected.len() as i64);
        let pressure = pressure_json(metadata.len(), buffer_bytes, memory.clone()).await?;
        let selected_json = selected
            .iter()
            .map(|segment| {
                json!({
                    "id": segment.id,
                    "segment_index": segment.segment_index,
                    "duration_seconds": segment.duration_seconds,
                    "artifact_path": segment.artifact_path,
                    "sha256": segment.validation_json.get("sha256").cloned().unwrap_or_else(|| json!("")),
                    "source_kind": segment.validation_json.get("source_kind").cloned().unwrap_or_else(|| json!("generated_runtime_fallback")),
                    "native_live_source": segment.validation_json.get("native_live_source").cloned().unwrap_or_else(|| json!(false))
                })
            })
            .collect::<Vec<_>>();
        let buffer_json = json!({
            "kind": "rolling_replay_buffer",
            "status": "ready",
            "source": source_json["kind"],
            "source_json": source_json.clone(),
            "segment_duration_seconds": 5,
            "requested_duration_seconds": input.duration_seconds,
            "selected_duration_seconds": selected.iter().map(|segment| segment.duration_seconds).sum::<i64>(),
            "selected_segment_count": selected.len(),
            "retention_seconds": buffer.iter().map(|segment| segment.duration_seconds).sum::<i64>(),
            "retention_policy": {
                "max_seconds": 300,
                "max_bytes": 536870912,
                "eviction": "oldest_first",
                "evicted_segments": 0
            },
            "segments": selected_json
        });
        let asset = publish_vanta_asset(&input, &output_path, &validation).await?;
        Ok(ReplayClip {
            media_asset_id: input.media_asset_id,
            output_path: output_path.to_string_lossy().to_string(),
            manifest_json: json!({
                "kind": "local_replay_clip",
                "broadcast_id": input.broadcast_id,
                "duration_seconds": input.duration_seconds,
                "sponsor_proof": input.sponsor_proof,
                "clip_path": output_path,
                "source": source_json.clone(),
                "validation": validation,
                "buffer": buffer_json,
                "timeline": {
                    "relative_end": "live_edge",
                    "relative_start_seconds": -input.duration_seconds
                },
                "upload": {
                    "mode": "deferred_local_queue",
                    "status": "queued"
                }
            }),
            pressure_json: pressure,
            buffer_json,
            upload_queue_json: asset["upload_queue_json"].clone(),
            asset_json: asset,
            segments: buffer,
        })
    }
}

async fn render_clip_from_source(
    source: &ReplayMediaSource,
    partial_path: &Path,
    duration_seconds: i64,
) -> Result<(), ReplayMediaError> {
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-sseof")
        .arg(format!("-{}", duration_seconds.max(1)))
        .arg("-i")
        .arg(&source.source_path)
        .arg("-t")
        .arg(duration_seconds.max(1).to_string())
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a:0?")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("veryfast")
        .arg("-tune")
        .arg("zerolatency")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg("128k")
        .arg("-movflags")
        .arg("+faststart")
        .arg(partial_path)
        .output()
        .await
        .map_err(|error| {
            ReplayMediaError::Render(format!(
                "could not spawn ffmpeg for replay source cut: {error}"
            ))
        })?;
    if !output.status.success() {
        remove_if_exists(partial_path).await?;
        return Err(ReplayMediaError::Render(format!(
            "source cut status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

async fn render_clip_from_concat(
    concat_path: &Path,
    partial_path: &Path,
    duration_seconds: i64,
) -> Result<(), ReplayMediaError> {
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-f")
        .arg("concat")
        .arg("-safe")
        .arg("0")
        .arg("-i")
        .arg(concat_path)
        .arg("-t")
        .arg(duration_seconds.to_string())
        .arg("-c")
        .arg("copy")
        .arg("-movflags")
        .arg("+faststart")
        .arg(partial_path)
        .output()
        .await
        .map_err(|error| {
            ReplayMediaError::Render(format!("could not spawn ffmpeg from PATH: {error}"))
        })?;
    if !output.status.success() {
        remove_if_exists(partial_path).await?;
        return Err(ReplayMediaError::Render(format!(
            "status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

async fn replay_dir(broadcast_id: &str) -> Result<PathBuf, ReplayMediaError> {
    let base = media_dir().await?.join("replay").join(broadcast_id);
    fs::create_dir_all(&base).await?;
    Ok(base)
}

async fn ensure_buffer_segments(
    base: &Path,
    input: &ReplayClipRequest,
) -> Result<Vec<ReplayBufferSegment>, ReplayMediaError> {
    let segment_duration = 5;
    let count = ((input.duration_seconds.max(segment_duration) + segment_duration - 1)
        / segment_duration)
        .clamp(1, 60);
    let buffer_dir = base.join("buffer");
    fs::create_dir_all(&buffer_dir).await?;
    let mut segments = Vec::new();
    for index in 0..count {
        let segment =
            render_buffer_segment(&buffer_dir, &input.broadcast_id, index, segment_duration)
                .await?;
        segments.push(segment);
    }
    Ok(segments)
}

async fn render_buffer_segment(
    buffer_dir: &Path,
    broadcast_id: &str,
    index: i64,
    duration_seconds: i64,
) -> Result<ReplayBufferSegment, ReplayMediaError> {
    let segment_id = format!("buffer_{broadcast_id}_{index:04}");
    let artifact_path = buffer_dir.join(format!("segment-{index:04}.mp4"));
    let partial_path = buffer_dir.join(format!("segment-{index:04}.partial.mp4"));
    remove_if_exists(&artifact_path).await?;
    remove_if_exists(&partial_path).await?;
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg(format!(
            "testsrc2=size=1280x720:rate=30:duration={duration_seconds}"
        ))
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg(format!(
            "sine=frequency={}:sample_rate=48000:duration={duration_seconds}",
            600 + index * 20
        ))
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("1:a:0")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("veryfast")
        .arg("-tune")
        .arg("zerolatency")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg("128k")
        .arg("-movflags")
        .arg("+faststart")
        .arg(&partial_path)
        .output()
        .await
        .map_err(|error| {
            ReplayMediaError::Render(format!("could not spawn ffmpeg buffer segment: {error}"))
        })?;
    if !output.status.success() {
        remove_if_exists(&partial_path).await?;
        return Err(ReplayMediaError::Render(format!(
            "buffer segment status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let validation_json = probe(&partial_path).await?;
    fs::rename(&partial_path, &artifact_path).await?;
    let metadata = fs::metadata(&artifact_path).await?;
    Ok(ReplayBufferSegment {
        id: segment_id,
        segment_index: index,
        duration_seconds,
        artifact_path: artifact_path.to_string_lossy().to_string(),
        validation_json: merge_json(
            validation_json,
            json!({
                "rolling_buffer_segment": true,
                "broadcast_id": broadcast_id,
                "segment_index": index,
                "duration_seconds": duration_seconds,
                "byte_length": metadata.len(),
                "sha256": sha256_file(&artifact_path).await?
            }),
        ),
        pressure_json: pressure_json(0, metadata.len(), memory_pressure_json(duration_seconds, 1))
            .await?,
    })
}

fn select_segments(
    segments: &[ReplayBufferSegment],
    duration_seconds: i64,
) -> Vec<ReplayBufferSegment> {
    let mut selected = Vec::new();
    let mut total = 0;
    for segment in segments.iter().rev() {
        selected.push(segment.clone());
        total += segment.duration_seconds;
        if total >= duration_seconds {
            break;
        }
    }
    selected.reverse();
    selected
}

async fn write_concat_file(
    path: &Path,
    segments: &[ReplayBufferSegment],
) -> Result<(), ReplayMediaError> {
    let body = segments
        .iter()
        .map(|segment| format!("file '{}'\n", segment.artifact_path.replace('\'', "'\\''")))
        .collect::<String>();
    fs::write(path, body).await?;
    Ok(())
}

async fn media_dir() -> Result<PathBuf, ReplayMediaError> {
    let base = std::env::var("VANTA_OBS_MEDIA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("vanta-obs-media"));
    fs::create_dir_all(&base).await?;
    Ok(base)
}

async fn remove_if_exists(path: &Path) -> Result<(), ReplayMediaError> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

async fn pressure_json(
    clip_bytes: u64,
    buffer_bytes: u64,
    memory: Value,
) -> Result<Value, ReplayMediaError> {
    let base = media_dir().await.ok();
    let media_dir_bytes_observed = base
        .as_ref()
        .map(|path| directory_size(path))
        .transpose()?
        .unwrap_or(0);
    Ok(json!({
        "clip_bytes": clip_bytes,
        "buffer_bytes": buffer_bytes,
        "disk_pressure": if clip_bytes + buffer_bytes > 536_870_912 { "warning" } else { "ok" },
        "memory_pressure": memory.get("status").cloned().unwrap_or_else(|| json!("ok")),
        "memory": memory,
        "media_dir_bytes_observed": media_dir_bytes_observed,
        "retention_policy": {
            "max_seconds": 300,
            "max_bytes": 536870912,
            "eviction": "oldest_first",
            "evicted_segments": 0
        }
    }))
}

async fn probe(path: &Path) -> Result<Value, ReplayMediaError> {
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
        .map_err(|error| {
            ReplayMediaError::Probe(format!("could not spawn ffprobe from PATH: {error}"))
        })?;
    if !output.status.success() {
        return Err(ReplayMediaError::Probe(format!(
            "status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let probed: Value = serde_json::from_slice(&output.stdout)?;
    let streams = probed
        .get("streams")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let has_video = streams
        .iter()
        .any(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("video"));
    let has_audio = streams
        .iter()
        .any(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("audio"));
    if !has_video || !has_audio {
        return Err(ReplayMediaError::Probe(
            "replay clip must contain playable audio and video".to_string(),
        ));
    }
    Ok(json!({
        "playable": true,
        "has_video": has_video,
        "has_audio": has_audio,
        "format": probed.get("format").cloned().unwrap_or_else(|| json!({})),
        "streams": streams
    }))
}

async fn publish_vanta_asset(
    input: &ReplayClipRequest,
    output_path: &Path,
    validation: &Value,
) -> Result<Value, ReplayMediaError> {
    let asset_dir = media_dir()
        .await?
        .join("vanta-assets")
        .join("replay")
        .join(&input.broadcast_id)
        .join(&input.media_asset_id);
    fs::create_dir_all(&asset_dir).await?;
    let asset_path = asset_dir.join("clip.mp4");
    let manifest_path = asset_dir.join("asset-manifest.json");
    remove_if_exists(&asset_path).await?;
    fs::copy(output_path, &asset_path).await?;
    let checksum = sha256_file(&asset_path).await?;
    let metadata = fs::metadata(&asset_path).await?;
    let published_at = chrono::Utc::now().to_rfc3339();
    let manifest = json!({
        "kind": "vanta_media_asset_manifest",
        "asset_id": input.media_asset_id,
        "asset_kind": "replay_clip",
        "broadcast_id": input.broadcast_id,
        "source_path": output_path,
        "asset_path": asset_path,
        "duration_seconds": input.duration_seconds,
        "sponsor_proof": input.sponsor_proof,
        "replay_source": input.source.as_ref().map(|source| json!({
            "kind": source.source_kind,
            "path": source.source_path,
            "source_id": source.source_id,
            "metadata_json": source.metadata_json
        })).unwrap_or_else(|| json!({
            "kind": "generated_runtime_fallback",
            "path": null,
            "source_id": null
        })),
        "byte_length": metadata.len(),
        "sha256": checksum,
        "validation": validation,
        "published_at": published_at
    });
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?).await?;
    Ok(json!({
        "asset_id": input.media_asset_id,
        "asset_kind": "replay_clip",
        "status": "uploaded",
        "publish_status": "vanta_asset_ready",
        "source_path": output_path,
        "asset_path": asset_path,
        "manifest_path": manifest_path,
        "metadata_json": manifest,
        "validation_json": validation,
        "upload_queue_json": {
            "mode": "instant_vanta_asset",
            "status": "uploaded",
            "ready_for_upload": true,
            "vanta_media_asset_id": input.media_asset_id,
            "asset_path": asset_path,
            "manifest_path": manifest_path,
            "published_at": published_at
        }
    }))
}

fn memory_pressure_json(duration_seconds: i64, segment_count: i64) -> Value {
    let frame_bytes_estimate = 1280_i64 * 720_i64 * 4_i64;
    let buffered_frames = duration_seconds.max(1) * 30;
    let estimated_bytes = (frame_bytes_estimate * buffered_frames).max(0) as u64;
    let budget = std::env::var("VANTA_OBS_REPLAY_MEMORY_BUDGET_BYTES")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(536_870_912);
    json!({
        "status": if estimated_bytes > budget { "warning" } else { "ok" },
        "estimated_uncompressed_bytes": estimated_bytes,
        "budget_bytes": budget,
        "segment_count": segment_count,
        "policy": {
            "strategy": "prefer_disk_ring",
            "eviction": "drop_unreferenced_frames_before_segments"
        }
    })
}

async fn sha256_file(path: &Path) -> Result<String, ReplayMediaError> {
    let bytes = fs::read(path).await?;
    let digest = Sha256::digest(&bytes);
    Ok(format!("{digest:x}"))
}

fn merge_json(mut left: Value, right: Value) -> Value {
    if let (Some(left), Some(right)) = (left.as_object_mut(), right.as_object()) {
        for (key, value) in right {
            left.insert(key.clone(), value.clone());
        }
    }
    left
}

fn directory_size(path: &Path) -> Result<u64, std::io::Error> {
    let mut total = 0;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            total += directory_size(&entry.path())?;
        } else {
            total += metadata.len();
        }
    }
    Ok(total)
}
