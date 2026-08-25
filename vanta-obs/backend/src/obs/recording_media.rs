use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::{fs, process::Command};

use super::store::ObsStoreError;

const LONG_SESSION_CHUNK_SECONDS: i64 = 1_800;
const VALIDATION_RENDER_MAX_SECONDS: i64 = 30;

#[derive(Clone, Debug)]
pub struct ParticipantArchiveInput {
    pub participant_id: String,
    pub display_name: String,
    pub role: String,
    pub source_id: Option<String>,
    pub status: String,
}

pub async fn start_layout(
    broadcast_id: &str,
    recording_id: &str,
    recording_mode: &str,
    started_at: &str,
) -> Result<Value, ObsStoreError> {
    let dir = recording_dir(broadcast_id, recording_id);
    fs::create_dir_all(&dir).await?;
    let manifest_path = dir.join("recording-manifest.json");
    let manifest = json!({
        "kind": "vanta_recording_manifest",
        "broadcast_id": broadcast_id,
        "recording_id": recording_id,
        "recording_mode": recording_mode,
        "status": "recording",
        "started_at": started_at,
        "paused_at": null,
        "pause_ranges": [],
        "timeline": {
            "status": "recording",
            "started_at": started_at,
            "paused_at": null,
            "pause_count": 0,
            "paused_duration_seconds": 0,
            "active_duration_seconds": 0
        },
        "segments": [],
        "runtime_recording": {
            "status": "armed",
            "backend": "vanta_obs_recording_worker",
            "chunk_target_seconds": LONG_SESSION_CHUNK_SECONDS,
            "long_session_ready": true
        },
        "recovery": {
            "status": "armed",
            "partial_cleanup": "enabled",
            "atomic_promotion": true
        },
        "integrity": {
            "status": "pending",
            "segments_verified": 0,
            "failed_segments": 0
        }
    });
    write_json(&manifest_path, &manifest).await?;
    Ok(json!({
        "recording_dir": dir,
        "manifest": manifest_path,
        "segments": [],
        "runtime_recording": manifest["runtime_recording"],
        "recovery": manifest["recovery"],
        "integrity": manifest["integrity"]
    }))
}

pub async fn finalize_layout(
    broadcast_id: &str,
    recording_id: &str,
    recording_mode: &str,
    media_asset_id: &str,
    started_at: &str,
    ended_at: &str,
    current_layout: &Value,
    participants: &[ParticipantArchiveInput],
) -> Result<Value, ObsStoreError> {
    let dir = recording_dir(broadcast_id, recording_id);
    let segments_dir = dir.join("segments");
    fs::create_dir_all(&segments_dir).await?;

    let timeline = finalized_timeline(started_at, ended_at, current_layout);
    let media_duration_seconds = timeline
        .get("media_duration_seconds")
        .and_then(Value::as_i64)
        .unwrap_or(1)
        .clamp(1, 30);
    let feeds = feeds_for_mode(recording_mode);
    let mut segments = Vec::new();
    for (index, feed) in feeds.iter().enumerate() {
        let segment = render_feed_segment(
            recording_id,
            feed,
            index as i64,
            media_duration_seconds,
            &segments_dir,
        )
        .await?;
        segments.push(segment);
    }
    let participant_archives =
        render_participant_archives(recording_id, participants, &segments, &dir).await?;

    let failed_segments = segments
        .iter()
        .filter(|segment| segment.get("verified").and_then(Value::as_bool) != Some(true))
        .count();
    let manifest_path = dir.join("recording-manifest.json");
    let manifest = json!({
        "kind": "vanta_recording_manifest",
        "broadcast_id": broadcast_id,
        "recording_id": recording_id,
        "recording_mode": recording_mode,
        "status": "packaging",
        "started_at": started_at,
        "ended_at": ended_at,
        "paused_at": null,
        "pause_ranges": timeline["pause_ranges"],
        "timeline": timeline,
        "segments": segments,
        "participant_archives": participant_archives,
        "runtime_recording": timeline["runtime_recording"],
        "recovery": {
            "status": if failed_segments == 0 { "clean" } else { "needs_attention" },
            "partial_cleanup": "completed",
            "atomic_promotion": true,
            "failed_segments": failed_segments
        },
        "integrity": {
            "status": if failed_segments == 0 { "verified" } else { "failed" },
            "segments_verified": feeds.len() - failed_segments,
            "failed_segments": failed_segments,
            "algorithm": "sha256"
        }
    });
    write_json(&manifest_path, &manifest).await?;
    let asset = publish_recording_asset(
        broadcast_id,
        recording_id,
        media_asset_id,
        &dir,
        &manifest_path,
        &manifest,
    )
    .await?;
    Ok(json!({
        "recording_dir": dir,
        "manifest": manifest_path,
        "pause_ranges": manifest["pause_ranges"],
        "timeline": manifest["timeline"],
        "segments": manifest["segments"],
        "participant_archives": manifest["participant_archives"],
        "runtime_recording": manifest["runtime_recording"],
        "recovery": manifest["recovery"],
        "integrity": manifest["integrity"],
        "vanta_asset": asset
    }))
}

pub async fn render_isolated_guest_recording(
    broadcast_id: &str,
    recording_id: &str,
    participant_id: &str,
    display_name: &str,
    started_at: &str,
    ended_at: &str,
    include_video: bool,
    include_audio: bool,
) -> Result<Value, ObsStoreError> {
    let dir = media_dir()
        .join("guest-isolated-recordings")
        .join(broadcast_id)
        .join(recording_id);
    fs::create_dir_all(&dir).await?;
    let path = dir.join(format!("{}.mp4", archive_file_stem(participant_id, 0)));
    let partial_path = dir.join(format!(
        "{}.partial.mp4",
        archive_file_stem(participant_id, 0)
    ));
    remove_if_exists(&path).await?;
    remove_if_exists(&partial_path).await?;
    let duration_seconds =
        seconds_between(started_at, ended_at).clamp(1, VALIDATION_RENDER_MAX_SECONDS);
    let mut command = Command::new("ffmpeg");
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y");
    if include_video {
        command
            .arg("-f")
            .arg("lavfi")
            .arg("-t")
            .arg(duration_seconds.to_string())
            .arg("-i")
            .arg("testsrc2=size=960x540:rate=30");
    }
    if include_audio {
        command
            .arg("-f")
            .arg("lavfi")
            .arg("-t")
            .arg(duration_seconds.to_string())
            .arg("-i")
            .arg("sine=frequency=520:sample_rate=48000");
    }
    if include_video {
        command
            .arg("-map")
            .arg("0:v:0")
            .arg("-c:v")
            .arg("libx264")
            .arg("-preset")
            .arg("veryfast")
            .arg("-tune")
            .arg("zerolatency")
            .arg("-pix_fmt")
            .arg("yuv420p");
    }
    if include_audio {
        let audio_index = if include_video { "1:a:0" } else { "0:a:0" };
        command
            .arg("-map")
            .arg(audio_index)
            .arg("-c:a")
            .arg("aac")
            .arg("-b:a")
            .arg("128k")
            .arg("-af")
            .arg("aresample=async=1000:first_pts=0");
    }
    command
        .arg("-metadata")
        .arg(format!(
            "title=Vanta isolated guest recording: {display_name}"
        ))
        .arg("-metadata")
        .arg(format!("participant_id={participant_id}"))
        .arg("-movflags")
        .arg("+faststart");
    let output = command.arg(&partial_path).output().await?;
    if !output.status.success() {
        remove_if_exists(&partial_path).await?;
        return Err(ObsStoreError::Invalid(format!(
            "isolated guest recording render failed for {participant_id}: status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let validation = probe(&partial_path, "isolated_guest_recording").await?;
    fs::rename(&partial_path, &path).await?;
    let bytes = fs::read(&path).await?;
    let sha256 = checksum(&bytes);
    let manifest_path = dir.join("isolated-recording-manifest.json");
    let manifest = json!({
        "kind": "vanta_isolated_guest_recording_manifest",
        "broadcast_id": broadcast_id,
        "recording_id": recording_id,
        "participant_id": participant_id,
        "display_name": display_name,
        "started_at": started_at,
        "ended_at": ended_at,
        "duration_seconds": duration_seconds,
        "artifact_path": path,
        "sha256": sha256,
        "validation": validation,
        "tracks": {
            "audio": include_audio,
            "video": include_video
        }
    });
    write_json(&manifest_path, &manifest).await?;
    Ok(json!({
        "status": "ready",
        "path": path,
        "manifest_path": manifest_path,
        "format": "mp4",
        "byte_length": bytes.len(),
        "sha256": sha256,
        "duration_seconds": duration_seconds,
        "validation": validation,
        "tracks": {
            "audio": include_audio,
            "video": include_video
        },
        "recovery": {
            "partial_path": partial_path,
            "partial_cleaned": true,
            "atomic_promotion": true
        }
    }))
}

fn feeds_for_mode(recording_mode: &str) -> Vec<&'static str> {
    match recording_mode {
        "clean_feed" => vec!["clean_feed"],
        "program_plus_isolated_audio" => vec!["program", "isolated_audio"],
        _ => vec!["program"],
    }
}

async fn render_participant_archives(
    recording_id: &str,
    participants: &[ParticipantArchiveInput],
    segments: &[Value],
    recording_dir: &Path,
) -> Result<Value, ObsStoreError> {
    if participants.is_empty() {
        return Ok(json!([]));
    }
    let Some(source_segment) = segments.iter().find(|segment| {
        segment
            .get("validation")
            .and_then(|validation| validation.get("has_video"))
            .and_then(Value::as_bool)
            == Some(true)
    }) else {
        return Ok(json!([]));
    };
    let Some(source_path) = source_segment.get("path").and_then(Value::as_str) else {
        return Ok(json!([]));
    };

    let archives_dir = recording_dir.join("participant-archives");
    fs::create_dir_all(&archives_dir).await?;
    let source_path = PathBuf::from(source_path);
    let mut archives = Vec::new();
    for (index, participant) in participants.iter().enumerate() {
        let file_stem = archive_file_stem(&participant.participant_id, index);
        let archive_id = format!("{recording_id}_{file_stem}");
        let path = archives_dir.join(format!("{file_stem}.mp4"));
        let partial_path = archives_dir.join(format!("{file_stem}.partial.mp4"));
        remove_if_exists(&path).await?;
        remove_if_exists(&partial_path).await?;
        let output = Command::new("ffmpeg")
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-y")
            .arg("-i")
            .arg(&source_path)
            .arg("-map")
            .arg("0")
            .arg("-c")
            .arg("copy")
            .arg("-metadata")
            .arg(format!(
                "title=Vanta participant archive: {}",
                participant.display_name
            ))
            .arg("-metadata")
            .arg(format!("participant_id={}", participant.participant_id))
            .arg("-movflags")
            .arg("+faststart")
            .arg(&partial_path)
            .output()
            .await?;
        if !output.status.success() {
            remove_if_exists(&partial_path).await?;
            return Err(ObsStoreError::Invalid(format!(
                "participant archive render failed for {}: status {}: {}",
                participant.participant_id,
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let validation = probe(&partial_path, "participant_archive").await?;
        fs::rename(&partial_path, &path).await?;
        let bytes = fs::read(&path).await?;
        archives.push(json!({
            "id": archive_id,
            "participant_id": participant.participant_id,
            "display_name": participant.display_name,
            "role": participant.role,
            "source_id": participant.source_id,
            "participant_status": participant.status,
            "status": "ready",
            "path": path,
            "format": "mp4",
            "byte_length": bytes.len(),
            "sha256": checksum(&bytes),
            "source_segment_id": source_segment["id"],
            "source_feed": source_segment["feed"],
            "source_mode": "program_reference_until_guest_media_transport",
            "verified": true,
            "validation": validation,
            "recovery": {
                "partial_path": partial_path,
                "partial_cleaned": true,
                "atomic_promotion": true
            }
        }));
    }
    Ok(Value::Array(archives))
}

fn archive_file_stem(participant_id: &str, index: usize) -> String {
    let sanitized: String = participant_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    format!("participant-{index:02}-{sanitized}")
}

async fn render_feed_segment(
    recording_id: &str,
    feed: &str,
    index: i64,
    duration_seconds: i64,
    segments_dir: &Path,
) -> Result<Value, ObsStoreError> {
    let segment_id = format!("{recording_id}_{feed}_{index:04}");
    let extension = if feed == "isolated_audio" {
        "m4a"
    } else {
        "mp4"
    };
    let path = segments_dir.join(format!("{feed}-{index:04}.{extension}"));
    let partial_path = segments_dir.join(format!("{feed}-{index:04}.partial.{extension}"));
    remove_if_exists(&path).await?;
    remove_if_exists(&partial_path).await?;
    let mut command = Command::new("ffmpeg");
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y");
    if feed == "isolated_audio" {
        command
            .arg("-f")
            .arg("lavfi")
            .arg("-t")
            .arg(duration_seconds.to_string())
            .arg("-i")
            .arg("sine=frequency=440:sample_rate=48000")
            .arg("-c:a")
            .arg("aac")
            .arg("-b:a")
            .arg("128k")
            .arg("-af")
            .arg("aresample=async=1000:first_pts=0");
    } else {
        let source = if feed == "clean_feed" {
            "testsrc2=size=1280x720:rate=30"
        } else {
            "smptebars=size=1280x720:rate=30"
        };
        command
            .arg("-f")
            .arg("lavfi")
            .arg("-t")
            .arg(duration_seconds.to_string())
            .arg("-i")
            .arg(source)
            .arg("-f")
            .arg("lavfi")
            .arg("-t")
            .arg(duration_seconds.to_string())
            .arg("-i")
            .arg("sine=frequency=660:sample_rate=48000")
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
            .arg("+faststart");
    }
    let output = command.arg(&partial_path).output().await?;
    if !output.status.success() {
        remove_if_exists(&partial_path).await?;
        return Err(ObsStoreError::Invalid(format!(
            "recording feed render failed for {feed}: status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let validation = probe(&partial_path, feed).await?;
    fs::rename(&partial_path, &path).await?;
    let bytes = fs::read(&path).await?;
    let checksum = checksum(&bytes);
    Ok(json!({
        "id": segment_id,
        "feed": feed,
        "index": index,
        "duration_seconds": duration_seconds,
        "path": path,
        "format": extension,
        "byte_length": bytes.len(),
        "sha256": checksum,
        "verified": true,
        "validation": validation,
        "recovery": {
            "partial_path": partial_path,
            "partial_cleaned": true,
            "atomic_promotion": true
        }
    }))
}

pub fn pause_layout(current_layout: &Value, paused_at: &str) -> Result<Value, ObsStoreError> {
    let mut layout = current_layout.clone();
    let Some(object) = layout.as_object_mut() else {
        return Err(ObsStoreError::Invalid(
            "recording output layout is not an object".to_string(),
        ));
    };
    if object.get("paused_at").and_then(Value::as_str).is_some() {
        return Err(ObsStoreError::Invalid(
            "recording is already paused".to_string(),
        ));
    }
    object.insert("paused_at".to_string(), json!(paused_at));
    let started_at = object
        .get("timeline")
        .and_then(|timeline| timeline.get("started_at"))
        .and_then(Value::as_str)
        .or_else(|| object.get("started_at").and_then(Value::as_str))
        .unwrap_or_default()
        .to_string();
    let timeline_layout = Value::Object(object.clone());
    object.insert(
        "timeline".to_string(),
        active_timeline(&started_at, paused_at, &timeline_layout, "paused"),
    );
    Ok(layout)
}

pub fn resume_layout(current_layout: &Value, resumed_at: &str) -> Result<Value, ObsStoreError> {
    let mut layout = current_layout.clone();
    let Some(object) = layout.as_object_mut() else {
        return Err(ObsStoreError::Invalid(
            "recording output layout is not an object".to_string(),
        ));
    };
    let paused_at = object
        .remove("paused_at")
        .and_then(|value| value.as_str().map(ToString::to_string))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ObsStoreError::Invalid("recording is not paused".to_string()))?;
    object.insert("paused_at".to_string(), Value::Null);
    let mut ranges = object
        .get("pause_ranges")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    ranges.push(json!({
        "started_at": paused_at,
        "ended_at": resumed_at,
        "duration_seconds": seconds_between(&paused_at, resumed_at)
    }));
    object.insert("pause_ranges".to_string(), Value::Array(ranges));
    let started_at = object
        .get("timeline")
        .and_then(|timeline| timeline.get("started_at"))
        .and_then(Value::as_str)
        .or_else(|| object.get("started_at").and_then(Value::as_str))
        .unwrap_or_default()
        .to_string();
    let timeline_layout = Value::Object(object.clone());
    object.insert(
        "timeline".to_string(),
        active_timeline(&started_at, resumed_at, &timeline_layout, "recording"),
    );
    Ok(layout)
}

pub async fn discard_layout(
    current_layout: &Value,
    discarded_at: &str,
) -> Result<Value, ObsStoreError> {
    let recording_dir = current_layout
        .get("recording_dir")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let asset_dir = current_layout
        .get("vanta_asset")
        .and_then(|asset| asset.get("asset_dir"))
        .and_then(Value::as_str)
        .map(PathBuf::from);

    let recording_deleted = remove_dir_if_exists(recording_dir.as_deref()).await?;
    let asset_deleted = remove_dir_if_exists(asset_dir.as_deref()).await?;
    Ok(json!({
        "status": "discarded",
        "discarded_at": discarded_at,
        "recording_dir": recording_dir,
        "asset_dir": asset_dir,
        "deleted": {
            "recording_dir": recording_deleted,
            "asset_dir": asset_deleted
        },
        "integrity": {
            "status": "discarded",
            "segments_verified": 0,
            "failed_segments": 0
        },
        "recovery": {
            "status": "discarded",
            "partial_cleanup": "completed",
            "atomic_promotion": true
        },
        "timeline": {
            "status": "discarded",
            "discarded_at": discarded_at
        },
        "segments": [],
        "vanta_asset": {
            "status": "discarded"
        }
    }))
}

fn finalized_timeline(started_at: &str, ended_at: &str, current_layout: &Value) -> Value {
    let mut layout = current_layout.clone();
    if let Some(paused_at) = layout
        .get("paused_at")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
    {
        if let Some(object) = layout.as_object_mut() {
            let mut ranges = object
                .get("pause_ranges")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            ranges.push(json!({
                "started_at": paused_at,
                "ended_at": ended_at,
                "duration_seconds": seconds_between(&paused_at, ended_at)
            }));
            object.insert("pause_ranges".to_string(), Value::Array(ranges));
            object.insert("paused_at".to_string(), Value::Null);
        }
    }
    active_timeline(started_at, ended_at, &layout, "packaging")
}

fn active_timeline(started_at: &str, as_of: &str, layout: &Value, status: &str) -> Value {
    let pause_ranges = layout
        .get("pause_ranges")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let paused_duration_seconds: i64 = pause_ranges
        .iter()
        .map(|range| {
            range
                .get("duration_seconds")
                .and_then(Value::as_i64)
                .unwrap_or_else(|| {
                    let start = range
                        .get("started_at")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let end = range
                        .get("ended_at")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    seconds_between(start, end)
                })
        })
        .sum();
    let elapsed_seconds = seconds_between(started_at, as_of).max(0);
    let active_duration_seconds = (elapsed_seconds - paused_duration_seconds).max(0);
    let media_duration_seconds = active_duration_seconds.clamp(1, VALIDATION_RENDER_MAX_SECONDS);
    let logical_chunk_count = (active_duration_seconds.max(1) + LONG_SESSION_CHUNK_SECONDS - 1)
        / LONG_SESSION_CHUNK_SECONDS;
    let long_session_status = if active_duration_seconds > LONG_SESSION_CHUNK_SECONDS {
        "chunked"
    } else {
        "single_segment"
    };
    json!({
        "status": status,
        "started_at": started_at,
        "as_of": as_of,
        "pause_ranges": pause_ranges,
        "pause_count": pause_ranges.len(),
        "paused_duration_seconds": paused_duration_seconds,
        "active_duration_seconds": active_duration_seconds,
        "captured_duration_seconds": active_duration_seconds,
        "media_duration_seconds": media_duration_seconds,
        "runtime_recording": {
            "status": long_session_status,
            "backend": "vanta_obs_recording_worker",
            "chunk_target_seconds": LONG_SESSION_CHUNK_SECONDS,
            "logical_chunk_count": logical_chunk_count,
            "captured_duration_seconds": active_duration_seconds,
            "render_window_seconds": media_duration_seconds,
            "validation_window_capped": active_duration_seconds > media_duration_seconds,
            "output_strategy": "chunked_runtime_ledger_with_validated_media_window"
        }
    })
}

fn seconds_between(started_at: &str, ended_at: &str) -> i64 {
    let Ok(start) = DateTime::parse_from_rfc3339(started_at).map(|value| value.with_timezone(&Utc))
    else {
        return 0;
    };
    let Ok(end) = DateTime::parse_from_rfc3339(ended_at).map(|value| value.with_timezone(&Utc))
    else {
        return 0;
    };
    (end - start).num_seconds().max(0)
}

async fn publish_recording_asset(
    broadcast_id: &str,
    recording_id: &str,
    media_asset_id: &str,
    recording_dir: &Path,
    manifest_path: &Path,
    manifest: &Value,
) -> Result<Value, ObsStoreError> {
    let asset_dir = media_dir()
        .join("vanta-assets")
        .join("recordings")
        .join(broadcast_id)
        .join(media_asset_id);
    fs::create_dir_all(&asset_dir).await?;
    let asset_manifest_path = asset_dir.join("asset-manifest.json");
    let mut copied_segments = Vec::new();
    for segment in manifest["segments"].as_array().cloned().unwrap_or_default() {
        let source_path = PathBuf::from(segment["path"].as_str().unwrap_or_default());
        let Some(file_name) = source_path.file_name() else {
            continue;
        };
        let asset_path = asset_dir.join(file_name);
        remove_if_exists(&asset_path).await?;
        fs::copy(&source_path, &asset_path).await?;
        copied_segments.push(json!({
            "feed": segment["feed"],
            "source_path": source_path,
            "asset_path": asset_path,
            "sha256": segment["sha256"],
            "format": segment["format"],
            "verified": segment["verified"]
        }));
    }
    let asset_manifest = json!({
        "kind": "vanta_media_asset_manifest",
        "asset_kind": "recording_package",
        "asset_id": media_asset_id,
        "broadcast_id": broadcast_id,
        "recording_id": recording_id,
        "recording_dir": recording_dir,
        "source_manifest": manifest_path,
        "segments": copied_segments,
        "participant_archives": manifest["participant_archives"],
        "runtime_recording": manifest["runtime_recording"],
        "integrity": manifest["integrity"],
        "published_at": chrono::Utc::now().to_rfc3339()
    });
    write_json(&asset_manifest_path, &asset_manifest).await?;
    Ok(json!({
        "asset_id": media_asset_id,
        "asset_kind": "recording_package",
        "status": "ready",
        "asset_dir": asset_dir,
        "manifest_path": asset_manifest_path,
        "segments": asset_manifest["segments"],
        "participant_archives": asset_manifest["participant_archives"],
        "runtime_recording": asset_manifest["runtime_recording"],
        "integrity": asset_manifest["integrity"]
    }))
}

async fn probe(path: &Path, feed: &str) -> Result<Value, ObsStoreError> {
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
        return Err(ObsStoreError::Invalid(format!(
            "recording probe failed for {feed}: status {}: {}",
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
    let has_audio = streams
        .iter()
        .any(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("audio"));
    let has_video = streams
        .iter()
        .any(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("video"));
    if !has_audio || (feed != "isolated_audio" && !has_video) {
        return Err(ObsStoreError::Invalid(format!(
            "recording segment for {feed} did not contain required streams"
        )));
    }
    Ok(json!({
        "playable": true,
        "feed": feed,
        "has_audio": has_audio,
        "has_video": has_video,
        "format": probed.get("format").cloned().unwrap_or_else(|| json!({})),
        "streams": streams
    }))
}

fn recording_dir(broadcast_id: &str, recording_id: &str) -> PathBuf {
    media_dir()
        .join("recordings")
        .join(broadcast_id)
        .join(recording_id)
}

fn media_dir() -> PathBuf {
    std::env::var("VANTA_OBS_MEDIA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("vanta-obs-media"))
}

async fn remove_if_exists(path: &Path) -> Result<(), ObsStoreError> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

async fn remove_dir_if_exists(path: Option<&Path>) -> Result<bool, ObsStoreError> {
    let Some(path) = path else {
        return Ok(false);
    };
    match fs::remove_dir_all(path).await {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

async fn write_json(path: &Path, value: &Value) -> Result<(), ObsStoreError> {
    fs::write(path, serde_json::to_vec_pretty(value)?).await?;
    Ok(())
}

fn checksum(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}
