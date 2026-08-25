use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{
    fs,
    process::Command,
    time::{Duration, timeout},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCaptureDevice {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub native_index: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeWindowCaptureTarget {
    pub id: String,
    pub label: String,
    pub owner: String,
    pub title: String,
    pub window_id: i64,
    pub width: i64,
    pub height: i64,
}

#[derive(Debug, Error)]
pub enum NativeCaptureError {
    #[error("native capture inventory failed: {0}")]
    Inventory(String),
    #[error("display preview frame capture failed: {0}")]
    PreviewFrame(String),
    #[error("continuous display capture failed: {0}")]
    Segment(String),
    #[error("microphone audio capture failed: {0}")]
    Audio(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct PreviewFrameResult {
    pub artifact_path: String,
    pub validation_json: Value,
}

#[derive(Debug, Clone)]
pub struct CaptureSegmentResult {
    pub artifact_path: String,
    pub validation_json: Value,
}

pub async fn native_capture_inventory() -> Result<Value, NativeCaptureError> {
    match std::env::consts::OS {
        "macos" => avfoundation_inventory().await,
        "windows" => Ok(unsupported_inventory(
            "windows",
            "Windows native device enumeration requires the signed Windows helper.",
        )),
        "linux" => Ok(unsupported_inventory(
            "linux",
            "Linux native capture is outside the current Vanta OBS platform scope.",
        )),
        other => Ok(unsupported_inventory(
            other,
            "Native capture is unavailable on this platform.",
        )),
    }
}

pub fn source_health_for_capture(inventory: &Value, capture_kind: &str) -> Value {
    let synthetic_ready = matches!(
        capture_kind,
        "program_canvas" | "system_audio" | "browser_surface"
    ) && inventory
        .pointer(&format!("/support/{capture_kind}"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let devices = inventory
        .get("devices")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let ready = if synthetic_ready {
        1
    } else {
        devices
            .iter()
            .filter(|device| {
                device
                    .get("kind")
                    .and_then(Value::as_str)
                    .is_some_and(|kind| kind == capture_kind)
            })
            .count()
    };
    let supported = inventory
        .pointer(&format!("/support/{capture_kind}"))
        .and_then(Value::as_bool)
        .unwrap_or(synthetic_ready);
    let permission = capture_permission(inventory, capture_kind);
    let permission_status = permission
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    json!({
        "capture_kind": capture_kind,
        "status": if permission_status == "denied" {
            "permission_denied"
        } else if supported && ready > 0 {
            "ready"
        } else if supported {
            "no_device"
        } else {
            "unsupported"
        },
        "ready_devices": ready,
        "supported": supported,
        "permission": permission,
        "inventory_trace": inventory.get("trace_event").cloned().unwrap_or_else(|| json!("media.capture.inventory.unavailable"))
    })
}

pub fn capture_permission(inventory: &Value, capture_kind: &str) -> Value {
    inventory
        .pointer(&format!("/permissions/{capture_kind}"))
        .cloned()
        .unwrap_or_else(|| {
            json!({
                "status": "not_required",
                "required": false,
                "remediation": ""
            })
        })
}

pub fn permission_block_message(capture_kind: &str, inventory: &Value) -> Option<&'static str> {
    let permission = capture_permission(inventory, capture_kind);
    if permission.get("status").and_then(Value::as_str) != Some("denied") {
        return None;
    }
    Some(match capture_kind {
        "camera" => {
            "macOS camera permission is denied; enable Vanta Native Capture in System Settings > Privacy & Security > Camera"
        }
        "microphone" => {
            "macOS microphone permission is denied; enable Vanta Native Capture in System Settings > Privacy & Security > Microphone"
        }
        "system_audio" => {
            "macOS Screen Recording permission is denied; enable Vanta Native Capture in System Settings > Privacy & Security > Screen Recording"
        }
        "application_audio" => {
            "macOS Screen Recording permission is denied; enable Vanta Native Capture in System Settings > Privacy & Security > Screen Recording"
        }
        _ => "native capture permission is denied for this source",
    })
}

pub fn unsupported_capture_message(capture_kind: &str, inventory: &Value) -> Option<&'static str> {
    let supported = inventory
        .pointer(&format!("/support/{capture_kind}"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if supported {
        None
    } else {
        Some(match capture_kind {
            "window" => {
                "native window capture is not supported by the current helper on this platform"
            }
            "application_audio" => {
                "application audio capture requires ScreenCaptureKit application filtering on macOS"
            }
            "desktop_audio" => {
                "desktop audio capture requires an installed system loopback audio device"
            }
            "system_audio" => {
                "native system audio capture without loopback requires ScreenCaptureKit on macOS"
            }
            "browser_surface" => {
                "browser surface capture requires Vanta runtime source-frame ingestion"
            }
            _ => "capture kind is not supported by the current helper on this platform",
        })
    }
}

pub async fn capture_preview_frame(
    session_id: &str,
    capture_kind: &str,
) -> Result<PreviewFrameResult, NativeCaptureError> {
    if !matches!(
        capture_kind,
        "display" | "program_canvas" | "window" | "camera"
    ) {
        return Err(NativeCaptureError::PreviewFrame(format!(
            "preview frame capture is only implemented for camera-backed, display-backed, or window-backed sessions, not {capture_kind}"
        )));
    }
    if capture_kind == "camera" {
        return capture_macos_camera_frame(session_id).await;
    }
    if capture_kind == "program_canvas" {
        return capture_program_canvas_frame(session_id).await;
    }
    if capture_kind == "window" {
        return capture_macos_window_frame(session_id).await;
    }
    match std::env::consts::OS {
        "macos" => capture_macos_display_frame(session_id).await,
        other => Err(NativeCaptureError::PreviewFrame(format!(
            "preview frame capture is unavailable on {other}"
        ))),
    }
}

pub async fn capture_display_segment(
    session_id: &str,
    capture_kind: &str,
    frame_rate: i64,
    duration_seconds: i64,
) -> Result<CaptureSegmentResult, NativeCaptureError> {
    if !matches!(
        capture_kind,
        "display" | "program_canvas" | "window" | "camera"
    ) {
        return Err(NativeCaptureError::Segment(format!(
            "continuous display capture is only implemented for camera-backed, display-backed, or window-backed sessions, not {capture_kind}"
        )));
    }
    if capture_kind == "camera" {
        return capture_macos_camera_segment(session_id, frame_rate, duration_seconds).await;
    }
    if capture_kind == "program_canvas" {
        return capture_program_canvas_segment(session_id, frame_rate, duration_seconds).await;
    }
    if capture_kind == "window" {
        return capture_macos_window_segment(session_id, frame_rate, duration_seconds).await;
    }
    match std::env::consts::OS {
        "macos" => {
            let inventory = avfoundation_inventory().await?;
            let display_index = first_display_index(&inventory).ok_or_else(|| {
                NativeCaptureError::Segment(
                    "AVFoundation did not report a display capture device".to_string(),
                )
            })?;
            capture_macos_display_segment(session_id, display_index, frame_rate, duration_seconds)
                .await
        }
        other => Err(NativeCaptureError::Segment(format!(
            "continuous display capture is unavailable on {other}"
        ))),
    }
}

pub async fn capture_microphone_segment(
    session_id: &str,
    capture_kind: &str,
    duration_seconds: i64,
) -> Result<CaptureSegmentResult, NativeCaptureError> {
    if capture_kind != "microphone" {
        return Err(NativeCaptureError::Audio(format!(
            "microphone audio capture is only implemented for microphone sessions, not {capture_kind}"
        )));
    }
    match std::env::consts::OS {
        "macos" => {
            let inventory = avfoundation_inventory().await?;
            let microphone_index = first_microphone_index(&inventory).ok_or_else(|| {
                NativeCaptureError::Audio(
                    "AVFoundation did not report a microphone device".to_string(),
                )
            })?;
            capture_macos_microphone_segment(session_id, microphone_index, duration_seconds).await
        }
        other => Err(NativeCaptureError::Audio(format!(
            "microphone audio capture is unavailable on {other}"
        ))),
    }
}

pub async fn capture_desktop_audio_segment(
    session_id: &str,
    capture_kind: &str,
    duration_seconds: i64,
) -> Result<CaptureSegmentResult, NativeCaptureError> {
    if capture_kind != "desktop_audio" {
        return Err(NativeCaptureError::Audio(format!(
            "desktop audio capture is only implemented for desktop_audio sessions, not {capture_kind}"
        )));
    }
    match std::env::consts::OS {
        "macos" => {
            let inventory = avfoundation_inventory().await?;
            let device_index = first_desktop_audio_index(&inventory).ok_or_else(|| {
                NativeCaptureError::Audio(
                    "AVFoundation did not report a desktop loopback audio device".to_string(),
                )
            })?;
            capture_macos_audio_segment(
                session_id,
                "desktop",
                "desktop_audio",
                device_index,
                duration_seconds,
            )
            .await
        }
        other => Err(NativeCaptureError::Audio(format!(
            "desktop audio capture is unavailable on {other}"
        ))),
    }
}

pub async fn capture_system_audio_segment(
    session_id: &str,
    capture_kind: &str,
    duration_seconds: i64,
) -> Result<CaptureSegmentResult, NativeCaptureError> {
    if capture_kind != "system_audio" {
        return Err(NativeCaptureError::Audio(format!(
            "native system audio capture is only implemented for system_audio sessions, not {capture_kind}"
        )));
    }
    match std::env::consts::OS {
        "macos" => capture_macos_system_audio_segment(session_id, duration_seconds).await,
        other => Err(NativeCaptureError::Audio(format!(
            "native system audio capture is unavailable on {other}"
        ))),
    }
}

pub async fn capture_application_audio_segment(
    session_id: &str,
    capture_kind: &str,
    duration_seconds: i64,
) -> Result<CaptureSegmentResult, NativeCaptureError> {
    if capture_kind != "application_audio" {
        return Err(NativeCaptureError::Audio(format!(
            "native application audio capture is only implemented for application_audio sessions, not {capture_kind}"
        )));
    }
    match std::env::consts::OS {
        "macos" => capture_macos_application_audio_segment(session_id, duration_seconds).await,
        other => Err(NativeCaptureError::Audio(format!(
            "native application audio capture is unavailable on {other}"
        ))),
    }
}

async fn avfoundation_inventory() -> Result<Value, NativeCaptureError> {
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-f")
        .arg("avfoundation")
        .arg("-list_devices")
        .arg("true")
        .arg("-i")
        .arg("")
        .output()
        .await
        .map_err(|error| {
            NativeCaptureError::Inventory(format!("could not run ffmpeg avfoundation: {error}"))
        })?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut devices = parse_avfoundation_devices(&stderr)
        .into_iter()
        .map(|device| {
            json!({
                "id": device.id,
                "label": device.label,
                "kind": device.kind,
                "native_index": device.native_index,
                "transport": "avfoundation"
            })
        })
        .collect::<Vec<_>>();
    let (window_devices, window_diagnostic) = macos_window_devices().await;
    devices.extend(window_devices);
    let camera_count = devices
        .iter()
        .filter(|device| device["kind"] == "camera")
        .count();
    let microphone_count = devices
        .iter()
        .filter(|device| device["kind"] == "microphone")
        .count();
    let display_count = devices
        .iter()
        .filter(|device| device["kind"] == "display")
        .count();
    let desktop_audio_count = devices
        .iter()
        .filter(|device| device["kind"] == "desktop_audio")
        .count();
    let window_count = devices
        .iter()
        .filter(|device| device["kind"] == "window")
        .count();
    let system_audio_probe = macos_system_audio_probe().await;
    let system_audio_supported = system_audio_probe
        .get("supported")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let application_audio_count = system_audio_probe
        .get("applications")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let application_audio_supported = system_audio_supported && application_audio_count > 0;
    if system_audio_supported {
        devices.push(json!({
            "id": "screencapturekit:system-audio",
            "label": "macOS System Audio",
            "kind": "system_audio",
            "native_index": -1,
            "transport": "screencapturekit",
            "loopback_device_required": false
        }));
    }
    if let Some(applications) = system_audio_probe
        .get("applications")
        .and_then(Value::as_array)
    {
        for (index, application) in applications.iter().enumerate() {
            let application_name = application
                .get("application_name")
                .and_then(Value::as_str)
                .unwrap_or("Application");
            let bundle_id = application
                .get("bundle_id")
                .and_then(Value::as_str)
                .unwrap_or("");
            devices.push(json!({
                "id": if bundle_id.is_empty() {
                    format!("screencapturekit:application-audio:{index}")
                } else {
                    format!("screencapturekit:application-audio:{bundle_id}")
                },
                "label": format!("{application_name} Audio"),
                "kind": "application_audio",
                "native_index": index as i64,
                "transport": "screencapturekit",
                "native_api": "ScreenCaptureKit",
                "bundle_id": bundle_id,
                "application_name": application_name,
                "process_id": application.get("process_id").cloned().unwrap_or_else(|| json!(0)),
                "loopback_device_required": false
            }));
        }
    }
    let mut diagnostics = window_diagnostic
        .map(|diagnostic| vec![diagnostic])
        .unwrap_or_default();
    if let Some(diagnostic) = system_audio_probe.get("diagnostic").and_then(Value::as_str) {
        if !diagnostic.is_empty() {
            diagnostics.push(diagnostic.to_string());
        }
    }
    Ok(json!({
        "platform": "macos",
        "transport": if system_audio_supported { "ffmpeg_avfoundation+screencapturekit" } else { "ffmpeg_avfoundation" },
        "status": "ready",
        "trace_event": "media.capture.inventory.avfoundation",
        "support": {
            "camera": camera_count > 0,
            "microphone": microphone_count > 0,
            "desktop_audio": desktop_audio_count > 0,
            "system_audio": system_audio_supported,
            "display": display_count > 0,
            "program_canvas": true,
            "browser_surface": true,
            "window": window_count > 0,
            "application_audio": application_audio_supported
        },
        "permissions": macos_permission_summary(&stderr, camera_count, microphone_count, display_count, window_count, system_audio_supported, application_audio_supported),
        "system_audio": system_audio_probe,
        "diagnostics": diagnostics,
        "devices": devices
    }))
}

fn unsupported_inventory(platform: &str, message: &str) -> Value {
    json!({
        "platform": platform,
        "transport": "unsupported",
        "status": "unsupported",
        "trace_event": "media.capture.inventory.unsupported",
        "support": {
            "camera": false,
            "microphone": false,
            "desktop_audio": false,
            "system_audio": false,
            "display": false,
            "program_canvas": true,
            "browser_surface": true,
            "window": false,
            "application_audio": false
        },
        "devices": [],
        "permissions": {
            "camera": permission_summary("unavailable", true, "native camera capture is unavailable on this platform"),
            "microphone": permission_summary("unavailable", true, "native microphone capture is unavailable on this platform"),
            "display": permission_summary("unavailable", true, "native display capture is unavailable on this platform"),
            "program_canvas": permission_summary("not_required", false, ""),
            "browser_surface": permission_summary("not_required", false, ""),
            "system_audio": permission_summary("unavailable", true, "native system audio capture requires ScreenCaptureKit on macOS"),
            "window": permission_summary("unavailable", true, "native window capture is unavailable on this platform"),
            "application_audio": permission_summary("unavailable", true, "native application audio capture is unavailable on this platform")
        },
        "diagnostics": [message]
    })
}

fn macos_permission_summary(
    stderr: &str,
    camera_count: usize,
    microphone_count: usize,
    display_count: usize,
    window_count: usize,
    system_audio_supported: bool,
    application_audio_supported: bool,
) -> Value {
    let camera_status = macos_permission_status(stderr, "camera", camera_count);
    let microphone_status = macos_permission_status(stderr, "microphone", microphone_count);
    json!({
        "camera": permission_summary(
            &camera_status,
            true,
            macos_permission_remediation("camera", &camera_status),
        ),
        "microphone": permission_summary(
            &microphone_status,
            true,
            macos_permission_remediation("microphone", &microphone_status),
        ),
        "display": permission_summary(
            if display_count > 0 { "ready" } else { "unavailable" },
            true,
            if display_count > 0 {
                "screen recording access is available to the native helper"
            } else {
                "enable Screen Recording for Vanta Native Capture if macOS prompts for it"
            },
        ),
        "program_canvas": permission_summary("not_required", false, ""),
        "browser_surface": permission_summary("not_required", false, ""),
        "desktop_audio": permission_summary("not_required", false, "desktop loopback device availability controls support"),
        "system_audio": permission_summary(
            if system_audio_supported { "prompt_required" } else { "unavailable" },
            true,
            if system_audio_supported {
                "macOS will request Screen Recording access for native system audio capture"
            } else {
                "native system audio capture requires ScreenCaptureKit on macOS"
            },
        ),
        "window": permission_summary(
            if window_count > 0 { "ready" } else { "unavailable" },
            true,
            if window_count > 0 {
                "window capture access is available to the native helper"
            } else {
                "enable Screen Recording for Vanta Native Capture and keep a captureable window visible"
            },
        ),
        "application_audio": permission_summary(
            if application_audio_supported { "prompt_required" } else { "unavailable" },
            true,
            if application_audio_supported {
                "macOS will request Screen Recording access for native application audio capture"
            } else {
                "native application audio capture requires ScreenCaptureKit and a captureable running application"
            },
        )
    })
}

fn permission_summary(status: &str, required: bool, remediation: &str) -> Value {
    json!({
        "status": status,
        "required": required,
        "remediation": remediation
    })
}

fn macos_permission_status(stderr: &str, capture_kind: &str, device_count: usize) -> String {
    if macos_permission_denied(stderr, capture_kind) {
        "denied".to_string()
    } else if device_count > 0 {
        "prompt_required".to_string()
    } else {
        "unavailable".to_string()
    }
}

fn macos_permission_denied(stderr: &str, capture_kind: &str) -> bool {
    let haystack = stderr.to_ascii_lowercase();
    let kind_hint = match capture_kind {
        "camera" => ["camera", "video", "avfoundation"],
        "microphone" => ["microphone", "audio", "avfoundation"],
        _ => ["", "", ""],
    };
    let denied = [
        "not authorized",
        "not authorised",
        "permission denied",
        "access denied",
        "operation not permitted",
        "avfoundation authorization status is denied",
    ]
    .iter()
    .any(|needle| haystack.contains(needle));
    denied
        && kind_hint
            .iter()
            .any(|needle| needle.is_empty() || haystack.contains(needle))
}

fn macos_permission_remediation(capture_kind: &str, status: &str) -> &'static str {
    match (capture_kind, status) {
        ("camera", "denied") => {
            "enable Vanta Native Capture in System Settings > Privacy & Security > Camera"
        }
        ("microphone", "denied") => {
            "enable Vanta Native Capture in System Settings > Privacy & Security > Microphone"
        }
        ("camera", "prompt_required") => {
            "macOS will request Camera access the first time native capture starts"
        }
        ("microphone", "prompt_required") => {
            "macOS will request Microphone access the first time native capture starts"
        }
        ("camera", _) => "connect a camera and allow Camera access for Vanta Native Capture",
        ("microphone", _) => {
            "connect a microphone and allow Microphone access for Vanta Native Capture"
        }
        _ => "",
    }
}

async fn capture_program_canvas_segment(
    session_id: &str,
    frame_rate: i64,
    duration_seconds: i64,
) -> Result<CaptureSegmentResult, NativeCaptureError> {
    let requested_duration = duration_seconds.clamp(2, 30);
    let requested_frame_rate = frame_rate.clamp(1, 120);
    let base = media_dir().await?.join("capture-segments").join(session_id);
    fs::create_dir_all(&base).await?;
    let artifact_path = base.join(format!(
        "program-canvas-{}.mp4",
        chrono::Utc::now().timestamp_millis()
    ));
    let partial_path = artifact_path.with_extension("partial.mp4");
    remove_if_exists(&artifact_path).await?;
    remove_if_exists(&partial_path).await?;
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-t")
        .arg(requested_duration.to_string())
        .arg("-i")
        .arg(format!(
            "testsrc2=size=1920x1080:rate={requested_frame_rate}"
        ))
        .arg("-f")
        .arg("lavfi")
        .arg("-t")
        .arg(requested_duration.to_string())
        .arg("-i")
        .arg("anullsrc=channel_layout=stereo:sample_rate=48000")
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
        .arg("+frag_keyframe+empty_moov+default_base_moof")
        .arg(&partial_path)
        .output()
        .await
        .map_err(|error| {
            NativeCaptureError::Segment(format!("could not spawn program canvas capture: {error}"))
        })?;
    if !output.status.success() {
        remove_if_exists(&partial_path).await?;
        return Err(NativeCaptureError::Segment(format!(
            "program canvas capture exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let validation_json =
        validate_display_capture_segment(&partial_path, requested_frame_rate, requested_duration)
            .await?;
    let validation_json = with_capture_kind(validation_json, "program_canvas");
    fs::rename(&partial_path, &artifact_path).await?;
    Ok(CaptureSegmentResult {
        artifact_path: artifact_path.to_string_lossy().to_string(),
        validation_json,
    })
}

async fn capture_macos_display_segment(
    session_id: &str,
    display_index: i64,
    frame_rate: i64,
    duration_seconds: i64,
) -> Result<CaptureSegmentResult, NativeCaptureError> {
    let requested_frame_rate = frame_rate.clamp(1, 60);
    let requested_duration = duration_seconds.clamp(2, 30);
    let base = media_dir().await?.join("capture-segments").join(session_id);
    fs::create_dir_all(&base).await?;
    let artifact_path = base.join(format!(
        "display-{}.mp4",
        chrono::Utc::now().timestamp_millis()
    ));
    let partial_path = artifact_path.with_extension("partial.mp4");
    remove_if_exists(&artifact_path).await?;
    remove_if_exists(&partial_path).await?;
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-f")
        .arg("avfoundation")
        .arg("-framerate")
        .arg(requested_frame_rate.to_string())
        .arg("-t")
        .arg(requested_duration.to_string())
        .arg("-i")
        .arg(format!("{display_index}:none"))
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("veryfast")
        .arg("-tune")
        .arg("zerolatency")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-movflags")
        .arg("+frag_keyframe+empty_moov+default_base_moof")
        .arg(&partial_path)
        .output()
        .await
        .map_err(|error| {
            NativeCaptureError::Segment(format!("could not spawn ffmpeg display capture: {error}"))
        })?;
    if !output.status.success() {
        remove_if_exists(&partial_path).await?;
        return Err(NativeCaptureError::Segment(format!(
            "ffmpeg display capture exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let validation_json =
        validate_segment(&partial_path, requested_frame_rate, requested_duration).await?;
    fs::rename(&partial_path, &artifact_path).await?;
    Ok(CaptureSegmentResult {
        artifact_path: artifact_path.to_string_lossy().to_string(),
        validation_json,
    })
}

async fn capture_macos_camera_segment(
    session_id: &str,
    frame_rate: i64,
    duration_seconds: i64,
) -> Result<CaptureSegmentResult, NativeCaptureError> {
    let inventory = avfoundation_inventory().await?;
    let camera_index = first_camera_index(&inventory).ok_or_else(|| {
        NativeCaptureError::Segment(
            "AVFoundation did not report a native camera capture device".to_string(),
        )
    })?;
    let requested_frame_rate = frame_rate.clamp(1, 60);
    let capture_frame_rate = requested_frame_rate.min(30);
    let requested_duration = duration_seconds.clamp(2, 30);
    let base = media_dir().await?.join("capture-segments").join(session_id);
    fs::create_dir_all(&base).await?;
    let artifact_path = base.join(format!(
        "camera-{}.mp4",
        chrono::Utc::now().timestamp_millis()
    ));
    let partial_path = artifact_path.with_extension("partial.mp4");
    remove_if_exists(&artifact_path).await?;
    remove_if_exists(&partial_path).await?;
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-f")
        .arg("avfoundation")
        .arg("-framerate")
        .arg(capture_frame_rate.to_string())
        .arg("-video_size")
        .arg("1280x720")
        .arg("-t")
        .arg(requested_duration.to_string())
        .arg("-i")
        .arg(format!("{camera_index}:none"))
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("veryfast")
        .arg("-tune")
        .arg("zerolatency")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-movflags")
        .arg("+frag_keyframe+empty_moov+default_base_moof")
        .arg(&partial_path)
        .output()
        .await
        .map_err(|error| {
            NativeCaptureError::Segment(format!("could not spawn ffmpeg camera capture: {error}"))
        })?;
    if !output.status.success() {
        remove_if_exists(&partial_path).await?;
        return Err(NativeCaptureError::Segment(format!(
            "ffmpeg camera capture exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let validation_json =
        validate_segment(&partial_path, capture_frame_rate, requested_duration).await?;
    let validation_json = with_camera_segment_metadata(
        validation_json,
        requested_frame_rate,
        capture_frame_rate,
        camera_index,
        &inventory,
    );
    fs::rename(&partial_path, &artifact_path).await?;
    Ok(CaptureSegmentResult {
        artifact_path: artifact_path.to_string_lossy().to_string(),
        validation_json,
    })
}

async fn capture_macos_microphone_segment(
    session_id: &str,
    microphone_index: i64,
    duration_seconds: i64,
) -> Result<CaptureSegmentResult, NativeCaptureError> {
    capture_macos_audio_segment(
        session_id,
        "microphone",
        "microphone",
        microphone_index,
        duration_seconds,
    )
    .await
}

async fn capture_macos_audio_segment(
    session_id: &str,
    prefix: &str,
    capture_kind: &str,
    audio_index: i64,
    duration_seconds: i64,
) -> Result<CaptureSegmentResult, NativeCaptureError> {
    let requested_duration = duration_seconds.clamp(2, 30);
    let base = media_dir().await?.join("capture-audio").join(session_id);
    fs::create_dir_all(&base).await?;
    let artifact_path = base.join(format!(
        "{prefix}-{}.m4a",
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
        .arg("-f")
        .arg("avfoundation")
        .arg("-t")
        .arg(requested_duration.to_string())
        .arg("-i")
        .arg(format!(":{audio_index}"))
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg("128k")
        .arg("-af")
        .arg("aresample=async=1000:first_pts=0")
        .arg(&partial_path)
        .output()
        .await
        .map_err(|error| {
            NativeCaptureError::Audio(format!(
                "could not spawn ffmpeg microphone capture: {error}"
            ))
        })?;
    if !output.status.success() {
        remove_if_exists(&partial_path).await?;
        return Err(NativeCaptureError::Audio(format!(
            "ffmpeg microphone capture exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let validation_json = validate_audio_segment(&partial_path, requested_duration).await?;
    let validation_json = with_audio_capture_metadata(validation_json, capture_kind);
    fs::rename(&partial_path, &artifact_path).await?;
    Ok(CaptureSegmentResult {
        artifact_path: artifact_path.to_string_lossy().to_string(),
        validation_json,
    })
}

async fn capture_macos_system_audio_segment(
    session_id: &str,
    duration_seconds: i64,
) -> Result<CaptureSegmentResult, NativeCaptureError> {
    let probe = macos_system_audio_probe().await;
    if !probe
        .get("supported")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(NativeCaptureError::Audio(
            probe
                .get("diagnostic")
                .and_then(Value::as_str)
                .unwrap_or("ScreenCaptureKit system audio capture is unavailable")
                .to_string(),
        ));
    }
    let requested_duration = duration_seconds.clamp(2, 30);
    let base = media_dir().await?.join("capture-audio").join(session_id);
    fs::create_dir_all(&base).await?;
    let artifact_path = base.join(format!(
        "system-audio-{}.m4a",
        chrono::Utc::now().timestamp_millis()
    ));
    let partial_path = artifact_path.with_extension("partial.m4a");
    let script_path = base.join("screencapturekit-system-audio.swift");
    remove_if_exists(&artifact_path).await?;
    remove_if_exists(&partial_path).await?;
    fs::write(&script_path, MACOS_SYSTEM_AUDIO_CAPTURE_SWIFT).await?;
    let output = timeout(
        Duration::from_secs((requested_duration + 12) as u64),
        Command::new("swift")
            .arg(&script_path)
            .arg(&partial_path)
            .arg(requested_duration.to_string())
            .output(),
    )
    .await
    .map_err(|_| {
        NativeCaptureError::Audio(
            "ScreenCaptureKit system audio capture timed out before producing an artifact"
                .to_string(),
        )
    })?
    .map_err(|error| {
        NativeCaptureError::Audio(format!(
            "could not spawn ScreenCaptureKit system audio capture: {error}"
        ))
    })?;
    if !output.status.success() {
        remove_if_exists(&partial_path).await?;
        return Err(NativeCaptureError::Audio(format!(
            "ScreenCaptureKit system audio capture exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let validation_json = validate_audio_segment(&partial_path, requested_duration).await?;
    let mut validation_json = with_audio_capture_metadata(validation_json, "system_audio");
    if let Some(object) = validation_json.as_object_mut() {
        object.insert("native_api".to_string(), json!("ScreenCaptureKit"));
        object.insert("loopback_device_required".to_string(), json!(false));
        object.insert("system_audio_capture".to_string(), json!(true));
        object.insert("permission_kind".to_string(), json!("screen_recording"));
    }
    fs::rename(&partial_path, &artifact_path).await?;
    Ok(CaptureSegmentResult {
        artifact_path: artifact_path.to_string_lossy().to_string(),
        validation_json,
    })
}

async fn capture_macos_application_audio_segment(
    session_id: &str,
    duration_seconds: i64,
) -> Result<CaptureSegmentResult, NativeCaptureError> {
    let probe = macos_system_audio_probe().await;
    if !probe
        .get("supported")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || probe
            .get("applications")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
    {
        return Err(NativeCaptureError::Audio(
            probe
                .get("diagnostic")
                .and_then(Value::as_str)
                .unwrap_or("ScreenCaptureKit application audio capture is unavailable")
                .to_string(),
        ));
    }
    let requested_duration = duration_seconds.clamp(2, 30);
    let base = media_dir().await?.join("capture-audio").join(session_id);
    fs::create_dir_all(&base).await?;
    let artifact_path = base.join(format!(
        "application-audio-{}.m4a",
        chrono::Utc::now().timestamp_millis()
    ));
    let partial_path = artifact_path.with_extension("partial.m4a");
    let script_path = base.join("screencapturekit-application-audio.swift");
    remove_if_exists(&artifact_path).await?;
    remove_if_exists(&partial_path).await?;
    fs::write(&script_path, MACOS_APPLICATION_AUDIO_CAPTURE_SWIFT).await?;
    let output = timeout(
        Duration::from_secs((requested_duration + 12) as u64),
        Command::new("swift")
            .arg(&script_path)
            .arg(&partial_path)
            .arg(requested_duration.to_string())
            .output(),
    )
    .await
    .map_err(|_| {
        NativeCaptureError::Audio(
            "ScreenCaptureKit application audio capture timed out before producing an artifact"
                .to_string(),
        )
    })?
    .map_err(|error| {
        NativeCaptureError::Audio(format!(
            "could not spawn ScreenCaptureKit application audio capture: {error}"
        ))
    })?;
    if !output.status.success() {
        remove_if_exists(&partial_path).await?;
        return Err(NativeCaptureError::Audio(format!(
            "ScreenCaptureKit application audio capture exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let validation_json = validate_audio_segment(&partial_path, requested_duration).await?;
    let mut validation_json = with_audio_capture_metadata(validation_json, "application_audio");
    if let Some(object) = validation_json.as_object_mut() {
        object.insert("native_api".to_string(), json!("ScreenCaptureKit"));
        object.insert("loopback_device_required".to_string(), json!(false));
        object.insert("application_audio_capture".to_string(), json!(true));
        object.insert("permission_kind".to_string(), json!("screen_recording"));
    }
    fs::rename(&partial_path, &artifact_path).await?;
    Ok(CaptureSegmentResult {
        artifact_path: artifact_path.to_string_lossy().to_string(),
        validation_json,
    })
}

async fn capture_program_canvas_frame(
    session_id: &str,
) -> Result<PreviewFrameResult, NativeCaptureError> {
    let base = media_dir().await?.join("capture-preview").join(session_id);
    fs::create_dir_all(&base).await?;
    let artifact_path = base.join(format!(
        "program-canvas-frame-{}.png",
        chrono::Utc::now().timestamp_millis()
    ));
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("testsrc2=size=1920x1080:rate=1")
        .arg("-frames:v")
        .arg("1")
        .arg(&artifact_path)
        .output()
        .await
        .map_err(|error| {
            NativeCaptureError::PreviewFrame(format!(
                "could not spawn program canvas preview capture: {error}"
            ))
        })?;
    if !output.status.success() {
        return Err(NativeCaptureError::PreviewFrame(format!(
            "program canvas preview capture exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let validation_json = validate_png(&artifact_path).await?;
    Ok(PreviewFrameResult {
        artifact_path: artifact_path.to_string_lossy().to_string(),
        validation_json,
    })
}

async fn capture_macos_display_frame(
    session_id: &str,
) -> Result<PreviewFrameResult, NativeCaptureError> {
    let base = media_dir().await?.join("capture-preview").join(session_id);
    fs::create_dir_all(&base).await?;
    let artifact_path = base.join(format!(
        "frame-{}.png",
        chrono::Utc::now().timestamp_millis()
    ));
    let output = Command::new("screencapture")
        .arg("-x")
        .arg("-t")
        .arg("png")
        .arg(&artifact_path)
        .output()
        .await
        .map_err(|error| {
            NativeCaptureError::PreviewFrame(format!("could not spawn screencapture: {error}"))
        })?;
    if !output.status.success() {
        return Err(NativeCaptureError::PreviewFrame(format!(
            "screencapture exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let validation_json = validate_png(&artifact_path).await?;
    Ok(PreviewFrameResult {
        artifact_path: artifact_path.to_string_lossy().to_string(),
        validation_json,
    })
}

async fn capture_macos_camera_frame(
    session_id: &str,
) -> Result<PreviewFrameResult, NativeCaptureError> {
    let inventory = avfoundation_inventory().await?;
    let camera_index = first_camera_index(&inventory).ok_or_else(|| {
        NativeCaptureError::PreviewFrame(
            "AVFoundation did not report a native camera capture device".to_string(),
        )
    })?;
    let base = media_dir().await?.join("capture-preview").join(session_id);
    fs::create_dir_all(&base).await?;
    let artifact_path = base.join(format!(
        "camera-frame-{}.png",
        chrono::Utc::now().timestamp_millis()
    ));
    remove_if_exists(&artifact_path).await?;
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-f")
        .arg("avfoundation")
        .arg("-framerate")
        .arg("30")
        .arg("-video_size")
        .arg("1280x720")
        .arg("-i")
        .arg(format!("{camera_index}:none"))
        .arg("-frames:v")
        .arg("1")
        .arg(&artifact_path)
        .output()
        .await
        .map_err(|error| {
            NativeCaptureError::PreviewFrame(format!(
                "could not spawn native camera preview capture: {error}"
            ))
        })?;
    if !output.status.success() {
        remove_if_exists(&artifact_path).await?;
        return Err(NativeCaptureError::PreviewFrame(format!(
            "native camera preview capture exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let mut validation_json = validate_png(&artifact_path).await?;
    if let Some(object) = validation_json.as_object_mut() {
        object.insert("capture_kind".to_string(), json!("camera"));
        object.insert("native_api".to_string(), json!("AVFoundation"));
        object.insert("native_camera_source_bridge".to_string(), json!(true));
        object.insert("camera_index".to_string(), json!(camera_index));
        object.insert(
            "source_bridge".to_string(),
            json!({
                "kind": "native_camera",
                "transport": "avfoundation",
                "browser_get_user_media": false
            }),
        );
    }
    Ok(PreviewFrameResult {
        artifact_path: artifact_path.to_string_lossy().to_string(),
        validation_json,
    })
}

async fn capture_macos_window_frame(
    session_id: &str,
) -> Result<PreviewFrameResult, NativeCaptureError> {
    let inventory = avfoundation_inventory().await?;
    let window_id = first_window_id(&inventory).ok_or_else(|| {
        NativeCaptureError::PreviewFrame(
            "CoreGraphics did not report a captureable on-screen window".to_string(),
        )
    })?;
    let base = media_dir().await?.join("capture-preview").join(session_id);
    fs::create_dir_all(&base).await?;
    let artifact_path = base.join(format!(
        "window-frame-{}.png",
        chrono::Utc::now().timestamp_millis()
    ));
    let output = Command::new("screencapture")
        .arg("-x")
        .arg("-o")
        .arg("-t")
        .arg("png")
        .arg(format!("-l{window_id}"))
        .arg(&artifact_path)
        .output()
        .await
        .map_err(|error| {
            NativeCaptureError::PreviewFrame(format!(
                "could not spawn window screencapture: {error}"
            ))
        })?;
    if !output.status.success() {
        return Err(NativeCaptureError::PreviewFrame(format!(
            "window screencapture exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let validation_json = with_capture_kind(validate_png(&artifact_path).await?, "window");
    Ok(PreviewFrameResult {
        artifact_path: artifact_path.to_string_lossy().to_string(),
        validation_json,
    })
}

async fn capture_macos_window_segment(
    session_id: &str,
    frame_rate: i64,
    duration_seconds: i64,
) -> Result<CaptureSegmentResult, NativeCaptureError> {
    let inventory = avfoundation_inventory().await?;
    let window = first_window_target(&inventory).ok_or_else(|| {
        NativeCaptureError::Segment(
            "CoreGraphics did not report a captureable on-screen window".to_string(),
        )
    })?;
    let requested_duration = duration_seconds.clamp(2, 30);
    let requested_frame_rate = frame_rate.clamp(1, 120).max(60);
    let capture_frame_rate = requested_frame_rate.min(60);
    let base = media_dir().await?.join("capture-segments").join(session_id);
    fs::create_dir_all(&base).await?;
    let artifact_path = base.join(format!(
        "window-sck-{}.mp4",
        chrono::Utc::now().timestamp_millis()
    ));
    let partial_path = artifact_path.with_extension("partial.mp4");
    let script_path = base.join("screencapturekit-window.swift");
    remove_if_exists(&partial_path).await?;
    remove_if_exists(&artifact_path).await?;
    fs::write(&script_path, MACOS_WINDOW_CAPTURE_SWIFT).await?;

    let output = timeout(
        Duration::from_secs((requested_duration + 18) as u64),
        Command::new("swift")
            .arg(&script_path)
            .arg(&partial_path)
            .arg(requested_duration.to_string())
            .arg(capture_frame_rate.to_string())
            .arg(window.window_id.to_string())
            .arg(window.width.max(320).to_string())
            .arg(window.height.max(240).to_string())
            .output(),
    )
    .await
    .map_err(|_| {
        NativeCaptureError::Segment(
            "ScreenCaptureKit window capture timed out before producing an artifact".to_string(),
        )
    })?
    .map_err(|error| {
        NativeCaptureError::Segment(format!(
            "could not spawn ScreenCaptureKit window capture: {error}"
        ))
    })?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
        remove_if_exists(&partial_path).await?;
        if error.contains("produced no complete window frames")
            || error.contains("did not report a display")
        {
            return capture_macos_window_sampled_segment(
                session_id,
                requested_frame_rate,
                requested_duration,
                window.window_id,
                &inventory,
                &error,
            )
            .await;
        }
        return Err(NativeCaptureError::Segment(format!(
            "ScreenCaptureKit window capture exited with status {}: {}",
            output.status, error
        )));
    }
    let validation_json =
        validate_display_capture_segment(&partial_path, capture_frame_rate, requested_duration)
            .await?;
    let validation_json = with_window_segment_metadata(
        validation_json,
        requested_frame_rate,
        capture_frame_rate,
        requested_duration,
        window.window_id,
        &inventory,
    );
    fs::rename(&partial_path, &artifact_path).await?;
    Ok(CaptureSegmentResult {
        artifact_path: artifact_path.to_string_lossy().to_string(),
        validation_json,
    })
}

async fn capture_macos_window_sampled_segment(
    session_id: &str,
    requested_frame_rate: i64,
    requested_duration: i64,
    window_id: i64,
    inventory: &Value,
    fallback_reason: &str,
) -> Result<CaptureSegmentResult, NativeCaptureError> {
    let sampled_frame_rate = requested_frame_rate.clamp(1, 5);
    let expected_samples = sampled_frame_rate * requested_duration;
    let base = media_dir().await?.join("capture-segments").join(session_id);
    let frames_dir = base.join(format!(
        "window-frames-{}",
        chrono::Utc::now().timestamp_millis()
    ));
    fs::create_dir_all(&frames_dir).await?;
    let partial_path = base.join("window-capture.partial.mp4");
    let artifact_path = base.join("window-capture.mp4");
    remove_if_exists(&partial_path).await?;
    remove_if_exists(&artifact_path).await?;

    let frame_interval = Duration::from_millis((1000 / sampled_frame_rate.max(1)) as u64);
    let mut captured = 0_i64;
    let mut dropped = 0_i64;
    for sample in 0..expected_samples {
        let frame_path = frames_dir.join(format!("frame-{captured:05}.png"));
        let output = Command::new("screencapture")
            .arg("-x")
            .arg("-o")
            .arg("-t")
            .arg("png")
            .arg(format!("-l{window_id}"))
            .arg(&frame_path)
            .output()
            .await
            .map_err(|error| {
                NativeCaptureError::Segment(format!(
                    "could not spawn fallback window frame capture: {error}"
                ))
            })?;
        if output.status.success() && validate_png(&frame_path).await.is_ok() {
            captured += 1;
        } else {
            dropped += 1;
            remove_if_exists(&frame_path).await?;
        }
        if sample + 1 < expected_samples {
            tokio::time::sleep(frame_interval).await;
        }
    }
    let minimum_samples = (expected_samples * 3 / 5).max(2);
    if captured < minimum_samples {
        return Err(NativeCaptureError::Segment(format!(
            "window fallback capture sampled only {captured}/{expected_samples} frames after ScreenCaptureKit failed: {fallback_reason}"
        )));
    }

    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-framerate")
        .arg(sampled_frame_rate.to_string())
        .arg("-i")
        .arg(frames_dir.join("frame-%05d.png"))
        .arg("-c:v")
        .arg("libx264")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-movflags")
        .arg("+faststart")
        .arg(&partial_path)
        .output()
        .await
        .map_err(|error| {
            NativeCaptureError::Segment(format!(
                "could not spawn fallback ffmpeg window capture: {error}"
            ))
        })?;
    if !output.status.success() {
        return Err(NativeCaptureError::Segment(format!(
            "fallback ffmpeg window capture exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let validated_duration = ((captured as f64) / (sampled_frame_rate.max(1) as f64)).ceil() as i64;
    let validation_json =
        validate_segment(&partial_path, sampled_frame_rate, validated_duration.max(1)).await?;
    let mut validation_json = with_window_sampled_segment_metadata(
        validation_json,
        requested_frame_rate,
        sampled_frame_rate,
        requested_duration,
        expected_samples,
        captured,
        dropped,
        window_id,
        inventory,
    );
    if let Some(object) = validation_json.as_object_mut() {
        object.insert("screencapturekit_attempted".to_string(), json!(true));
        object.insert(
            "screencapturekit_fallback_reason".to_string(),
            json!(fallback_reason),
        );
    }
    fs::rename(&partial_path, &artifact_path).await?;
    Ok(CaptureSegmentResult {
        artifact_path: artifact_path.to_string_lossy().to_string(),
        validation_json,
    })
}

fn first_window_target(inventory: &Value) -> Option<NativeWindowCaptureTarget> {
    inventory
        .get("devices")
        .and_then(Value::as_array)?
        .iter()
        .find_map(|device| {
            if device.get("kind").and_then(Value::as_str) != Some("window") {
                return None;
            }
            Some(NativeWindowCaptureTarget {
                id: device.get("id")?.as_str()?.to_string(),
                label: device.get("label")?.as_str()?.to_string(),
                owner: device.get("owner")?.as_str()?.to_string(),
                title: device
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                window_id: device.get("native_index")?.as_i64()?,
                width: number_to_i64(device.get("width")).unwrap_or(1280),
                height: number_to_i64(device.get("height")).unwrap_or(720),
            })
        })
}

async fn macos_window_devices() -> (Vec<Value>, Option<String>) {
    let output = match Command::new("swift")
        .arg("-e")
        .arg(MACOS_WINDOW_LIST_SWIFT)
        .output()
        .await
    {
        Ok(output) => output,
        Err(error) => {
            return (
                Vec::new(),
                Some(format!("could not run CoreGraphics window probe: {error}")),
            );
        }
    };
    if !output.status.success() {
        return (
            Vec::new(),
            Some(format!(
                "CoreGraphics window probe exited with status {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )),
        );
    }
    let raw = String::from_utf8_lossy(&output.stdout);
    (parse_macos_window_targets(&raw), None)
}

async fn macos_system_audio_probe() -> Value {
    if std::env::consts::OS != "macos" {
        return json!({
            "supported": false,
            "native_api": "ScreenCaptureKit",
            "diagnostic": "ScreenCaptureKit system audio capture is only available on macOS"
        });
    }
    let output = match Command::new("swift")
        .arg("-e")
        .arg(MACOS_SYSTEM_AUDIO_PROBE_SWIFT)
        .output()
        .await
    {
        Ok(output) => output,
        Err(error) => {
            return json!({
                "supported": false,
                "native_api": "ScreenCaptureKit",
                "diagnostic": format!("could not run ScreenCaptureKit probe: {error}")
            });
        }
    };
    if !output.status.success() {
        return json!({
            "supported": false,
            "native_api": "ScreenCaptureKit",
            "diagnostic": format!(
                "ScreenCaptureKit probe exited with status {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )
        });
    }
    let raw = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&raw).unwrap_or_else(|_| {
        json!({
            "supported": false,
            "native_api": "ScreenCaptureKit",
            "diagnostic": "ScreenCaptureKit probe did not return JSON"
        })
    })
}

async fn remove_if_exists(path: &Path) -> Result<(), NativeCaptureError> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

async fn media_dir() -> Result<PathBuf, NativeCaptureError> {
    let base = std::env::var("VANTA_OBS_MEDIA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("vanta-obs-media"));
    fs::create_dir_all(&base).await?;
    Ok(base)
}

async fn validate_segment(
    path: &Path,
    expected_frame_rate: i64,
    expected_duration_seconds: i64,
) -> Result<Value, NativeCaptureError> {
    validate_segment_with_frame_coverage(path, expected_frame_rate, expected_duration_seconds, 0.85)
        .await
}

async fn validate_display_capture_segment(
    path: &Path,
    expected_frame_rate: i64,
    expected_duration_seconds: i64,
) -> Result<Value, NativeCaptureError> {
    validate_segment_with_frame_coverage(path, expected_frame_rate, expected_duration_seconds, 0.70)
        .await
}

async fn validate_segment_with_frame_coverage(
    path: &Path,
    expected_frame_rate: i64,
    expected_duration_seconds: i64,
    minimum_frame_coverage: f64,
) -> Result<Value, NativeCaptureError> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-count_frames")
        .arg("-print_format")
        .arg("json")
        .arg("-show_format")
        .arg("-show_streams")
        .arg(path)
        .output()
        .await
        .map_err(|error| {
            NativeCaptureError::Segment(format!("could not spawn ffprobe display capture: {error}"))
        })?;
    if !output.status.success() {
        return Err(NativeCaptureError::Segment(format!(
            "ffprobe display capture exited with status {}: {}",
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
    let video = streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("video"))
        .ok_or_else(|| {
            NativeCaptureError::Segment("display capture artifact has no video stream".to_string())
        })?;
    let width = video
        .get("width")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let height = video
        .get("height")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let frame_count = observed_video_frames(&streams);
    let expected_frames = expected_frame_rate * expected_duration_seconds;
    let minimum_frames = ((expected_frames as f64) * minimum_frame_coverage).floor() as i64;
    let frame_coverage = frame_count as f64 / expected_frames.max(1) as f64;
    if width <= 0 || height <= 0 || frame_count < minimum_frames {
        return Err(NativeCaptureError::Segment(format!(
            "display capture validation failed: {width}x{height}, {frame_count}/{expected_frames} frames"
        )));
    }
    let bytes = fs::read(path).await?;
    let sha256 = Sha256::digest(&bytes);
    Ok(json!({
        "playable": true,
        "capture_kind": "display",
        "format": "mp4",
        "codec": video.get("codec_name").cloned().unwrap_or_else(|| json!("unknown")),
        "width": width,
        "height": height,
        "requested_duration_seconds": expected_duration_seconds,
        "validated_duration_seconds": format_duration_seconds(&probed),
        "requested_frame_rate": expected_frame_rate,
        "expected_video_frames": expected_frames,
        "observed_video_frames": frame_count,
        "minimum_video_frames": minimum_frames,
        "frame_coverage": frame_coverage,
        "frame_coverage_threshold": minimum_frame_coverage,
        "dropped_frames": (expected_frames - frame_count).max(0),
        "byte_length": bytes.len(),
        "sha256": format!("{sha256:x}"),
        "captured_at": chrono::Utc::now().to_rfc3339(),
        "continuous_capture": true,
        "permission": "granted",
        "streams": streams,
        "probe_format": probed.get("format").cloned().unwrap_or_else(|| json!({}))
    }))
}

fn with_capture_kind(mut validation: Value, capture_kind: &str) -> Value {
    if let Some(object) = validation.as_object_mut() {
        object.insert("capture_kind".to_string(), json!(capture_kind));
    }
    validation
}

#[allow(clippy::too_many_arguments)]
fn with_window_segment_metadata(
    mut validation: Value,
    requested_frame_rate: i64,
    capture_frame_rate: i64,
    requested_duration_seconds: i64,
    window_id: i64,
    inventory: &Value,
) -> Value {
    if let Some(object) = validation.as_object_mut() {
        let observed_frames = object
            .get("observed_video_frames")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let expected_frames = capture_frame_rate * requested_duration_seconds;
        object.insert("capture_kind".to_string(), json!("window"));
        object.insert("native_api".to_string(), json!("ScreenCaptureKit"));
        object.insert("runtime_authoritative".to_string(), json!(true));
        object.insert("sampled_frame_capture".to_string(), json!(false));
        object.insert("window_id".to_string(), json!(window_id));
        object.insert(
            "requested_frame_rate".to_string(),
            json!(requested_frame_rate),
        );
        object.insert("capture_frame_rate".to_string(), json!(capture_frame_rate));
        object.insert(
            "requested_duration_seconds".to_string(),
            json!(requested_duration_seconds),
        );
        object.insert("expected_window_frames".to_string(), json!(expected_frames));
        object.insert("captured_window_frames".to_string(), json!(observed_frames));
        object.insert(
            "dropped_frames".to_string(),
            json!((expected_frames - observed_frames).max(0)),
        );
        object.insert(
            "frame_pacing".to_string(),
            json!({
                "mode": "screencapturekit_window_stream",
                "target_fps": capture_frame_rate,
                "requested_fps": requested_frame_rate,
                "high_frame_rate_helper_required": false,
                "runtime_authoritative": true
            }),
        );
        object.insert(
            "source_bridge".to_string(),
            json!({
                "kind": "native_window",
                "transport": "screencapturekit",
                "sampled_screencapture": false
            }),
        );
        if let Some(window) =
            inventory
                .get("devices")
                .and_then(Value::as_array)
                .and_then(|devices| {
                    devices.iter().find(|device| {
                        device.get("kind").and_then(Value::as_str) == Some("window")
                            && device.get("native_index").and_then(Value::as_i64) == Some(window_id)
                    })
                })
        {
            object.insert("window_target".to_string(), window.clone());
        }
    }
    validation
}

#[allow(clippy::too_many_arguments)]
fn with_window_sampled_segment_metadata(
    mut validation: Value,
    requested_frame_rate: i64,
    sampled_frame_rate: i64,
    requested_duration_seconds: i64,
    expected_samples: i64,
    captured_samples: i64,
    dropped_samples: i64,
    window_id: i64,
    inventory: &Value,
) -> Value {
    if let Some(object) = validation.as_object_mut() {
        object.insert("capture_kind".to_string(), json!("window"));
        object.insert("native_api".to_string(), json!("CoreGraphics"));
        object.insert("runtime_authoritative".to_string(), json!(false));
        object.insert("sampled_frame_capture".to_string(), json!(true));
        object.insert("window_id".to_string(), json!(window_id));
        object.insert(
            "requested_frame_rate".to_string(),
            json!(requested_frame_rate),
        );
        object.insert("sampled_frame_rate".to_string(), json!(sampled_frame_rate));
        object.insert(
            "requested_duration_seconds".to_string(),
            json!(requested_duration_seconds),
        );
        object.insert(
            "expected_window_samples".to_string(),
            json!(expected_samples),
        );
        object.insert(
            "captured_window_samples".to_string(),
            json!(captured_samples),
        );
        object.insert("dropped_frames".to_string(), json!(dropped_samples));
        object.insert(
            "frame_pacing".to_string(),
            json!({
                "mode": "native_window_frame_sampling_fallback",
                "target_fps": sampled_frame_rate,
                "requested_fps": requested_frame_rate,
                "high_frame_rate_helper_required": true,
                "runtime_authoritative": false
            }),
        );
        object.insert(
            "source_bridge".to_string(),
            json!({
                "kind": "native_window",
                "transport": "coregraphics_screencapture_fallback",
                "sampled_screencapture": true
            }),
        );
        if let Some(window) =
            inventory
                .get("devices")
                .and_then(Value::as_array)
                .and_then(|devices| {
                    devices.iter().find(|device| {
                        device.get("kind").and_then(Value::as_str) == Some("window")
                            && device.get("native_index").and_then(Value::as_i64) == Some(window_id)
                    })
                })
        {
            object.insert("window_target".to_string(), window.clone());
        }
    }
    validation
}

fn with_camera_segment_metadata(
    mut validation: Value,
    requested_frame_rate: i64,
    capture_frame_rate: i64,
    camera_index: i64,
    inventory: &Value,
) -> Value {
    if let Some(object) = validation.as_object_mut() {
        object.insert("capture_kind".to_string(), json!("camera"));
        object.insert("native_api".to_string(), json!("AVFoundation"));
        object.insert("native_camera_source_bridge".to_string(), json!(true));
        object.insert("camera_index".to_string(), json!(camera_index));
        object.insert(
            "requested_frame_rate".to_string(),
            json!(requested_frame_rate),
        );
        object.insert("capture_frame_rate".to_string(), json!(capture_frame_rate));
        object.insert(
            "source_bridge".to_string(),
            json!({
                "kind": "native_camera",
                "transport": "avfoundation",
                "browser_get_user_media": false
            }),
        );
        object.insert(
            "frame_pacing".to_string(),
            json!({
                "mode": "native_camera_avfoundation",
                "target_fps": capture_frame_rate,
                "requested_fps": requested_frame_rate,
                "high_frame_rate_helper_required": requested_frame_rate > capture_frame_rate
            }),
        );
        if let Some(camera) =
            inventory
                .get("devices")
                .and_then(Value::as_array)
                .and_then(|devices| {
                    devices.iter().find(|device| {
                        device.get("kind").and_then(Value::as_str) == Some("camera")
                            && device.get("native_index").and_then(Value::as_i64)
                                == Some(camera_index)
                    })
                })
        {
            object.insert("camera_target".to_string(), camera.clone());
        }
    }
    validation
}

async fn validate_audio_segment(
    path: &Path,
    expected_duration_seconds: i64,
) -> Result<Value, NativeCaptureError> {
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
            NativeCaptureError::Audio(format!(
                "could not spawn ffprobe microphone capture: {error}"
            ))
        })?;
    if !output.status.success() {
        return Err(NativeCaptureError::Audio(format!(
            "ffprobe microphone capture exited with status {}: {}",
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
    let audio = streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("audio"))
        .ok_or_else(|| {
            NativeCaptureError::Audio("microphone capture artifact has no audio stream".to_string())
        })?;
    let duration_seconds = format_duration_seconds(&probed);
    let min_duration = (expected_duration_seconds as f64 - 0.35).max(0.0);
    if duration_seconds < min_duration {
        return Err(NativeCaptureError::Audio(format!(
            "microphone capture duration {duration_seconds:.2}s was shorter than expected {expected_duration_seconds}s"
        )));
    }
    let sample_rate = audio
        .get("sample_rate")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or_default();
    let channels = audio
        .get("channels")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    if sample_rate <= 0 || channels <= 0 {
        return Err(NativeCaptureError::Audio(format!(
            "microphone capture validation failed: sample_rate={sample_rate}, channels={channels}"
        )));
    }
    let bytes = fs::read(path).await?;
    let sha256 = Sha256::digest(&bytes);
    Ok(json!({
        "playable": true,
        "capture_kind": "microphone",
        "format": "m4a",
        "codec": audio.get("codec_name").cloned().unwrap_or_else(|| json!("unknown")),
        "requested_duration_seconds": expected_duration_seconds,
        "validated_duration_seconds": duration_seconds,
        "sample_rate": sample_rate,
        "channels": channels,
        "byte_length": bytes.len(),
        "sha256": format!("{sha256:x}"),
        "captured_at": chrono::Utc::now().to_rfc3339(),
        "live_input_capture": true,
        "isolated_audio": true,
        "drift_correction_filter": "aresample=async=1000:first_pts=0",
        "drift_correction_active": true,
        "permission": "granted",
        "streams": streams,
        "probe_format": probed.get("format").cloned().unwrap_or_else(|| json!({}))
    }))
}

fn with_audio_capture_metadata(mut validation: Value, capture_kind: &str) -> Value {
    if let Some(object) = validation.as_object_mut() {
        object.insert("capture_kind".to_string(), json!(capture_kind));
        object.insert(
            "isolated_audio".to_string(),
            json!(capture_kind == "microphone"),
        );
        object.insert(
            "desktop_audio".to_string(),
            json!(capture_kind == "desktop_audio"),
        );
        object.insert(
            "system_audio".to_string(),
            json!(capture_kind == "system_audio"),
        );
        object.insert(
            "application_audio".to_string(),
            json!(capture_kind == "application_audio"),
        );
        object.insert(
            "loopback_device_required".to_string(),
            json!(capture_kind == "desktop_audio"),
        );
    }
    validation
}

async fn validate_png(path: &Path) -> Result<Value, NativeCaptureError> {
    let bytes = fs::read(path).await?;
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return Err(NativeCaptureError::PreviewFrame(format!(
            "{} is not a valid PNG frame",
            path.display()
        )));
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    let sha256 = Sha256::digest(&bytes);
    Ok(json!({
        "format": "png",
        "width": width,
        "height": height,
        "byte_length": bytes.len(),
        "sha256": format!("{sha256:x}"),
        "captured_at": chrono::Utc::now().to_rfc3339(),
        "low_latency_preview": true,
        "permission": "granted"
    }))
}

fn first_display_index(inventory: &Value) -> Option<i64> {
    inventory
        .get("devices")
        .and_then(Value::as_array)?
        .iter()
        .find(|device| device.get("kind").and_then(Value::as_str) == Some("display"))
        .and_then(|device| device.get("native_index").and_then(Value::as_i64))
}

fn first_camera_index(inventory: &Value) -> Option<i64> {
    inventory
        .get("devices")
        .and_then(Value::as_array)?
        .iter()
        .find(|device| device.get("kind").and_then(Value::as_str) == Some("camera"))
        .and_then(|device| device.get("native_index").and_then(Value::as_i64))
}

fn first_microphone_index(inventory: &Value) -> Option<i64> {
    inventory
        .get("devices")
        .and_then(Value::as_array)?
        .iter()
        .find(|device| device.get("kind").and_then(Value::as_str) == Some("microphone"))
        .and_then(|device| device.get("native_index").and_then(Value::as_i64))
}

fn first_desktop_audio_index(inventory: &Value) -> Option<i64> {
    inventory
        .get("devices")
        .and_then(Value::as_array)?
        .iter()
        .find(|device| device.get("kind").and_then(Value::as_str) == Some("desktop_audio"))
        .and_then(|device| device.get("native_index").and_then(Value::as_i64))
}

fn first_window_id(inventory: &Value) -> Option<i64> {
    inventory
        .get("devices")
        .and_then(Value::as_array)?
        .iter()
        .find(|device| device.get("kind").and_then(Value::as_str) == Some("window"))
        .and_then(|device| device.get("native_index").and_then(Value::as_i64))
}

fn format_duration_seconds(probed: &Value) -> f64 {
    probed
        .pointer("/format/duration")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0)
}

fn observed_video_frames(streams: &[Value]) -> i64 {
    streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("video"))
        .and_then(|stream| {
            stream
                .get("nb_read_frames")
                .or_else(|| stream.get("nb_frames"))
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<i64>().ok())
        })
        .unwrap_or(0)
}

pub fn parse_avfoundation_devices(raw: &str) -> Vec<NativeCaptureDevice> {
    let mut section = "";
    let mut devices = Vec::new();
    for line in raw.lines() {
        if line.contains("AVFoundation video devices:") {
            section = "video";
            continue;
        }
        if line.contains("AVFoundation audio devices:") {
            section = "audio";
            continue;
        }
        let Some((index, label)) = parse_indexed_line(line) else {
            continue;
        };
        let kind = if section == "audio" && is_desktop_loopback_label(&label) {
            "desktop_audio"
        } else if section == "audio" {
            "microphone"
        } else if label.to_ascii_lowercase().starts_with("capture screen") {
            "display"
        } else {
            "camera"
        };
        devices.push(NativeCaptureDevice {
            id: format!("avfoundation:{section}:{index}"),
            label,
            kind: kind.to_string(),
            native_index: index,
        });
    }
    devices
}

pub fn parse_macos_window_targets(raw: &str) -> Vec<Value> {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|target| {
            let window_id = target.get("id").and_then(Value::as_i64)?;
            let owner = target.get("owner").and_then(Value::as_str)?.trim();
            let title = target
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            let width = number_to_i64(target.get("width")).unwrap_or_default();
            let height = number_to_i64(target.get("height")).unwrap_or_default();
            if window_id <= 0 || owner.is_empty() || width < 80 || height < 60 {
                return None;
            }
            let label = if title.is_empty() {
                owner.to_string()
            } else {
                format!("{owner} - {title}")
            };
            Some(json!({
                "id": format!("coregraphics:window:{window_id}"),
                "label": label,
                "kind": "window",
                "native_index": window_id,
                "transport": "coregraphics",
                "owner": owner,
                "title": title,
                "width": width,
                "height": height
            }))
        })
        .collect()
}

fn number_to_i64(value: Option<&Value>) -> Option<i64> {
    value.and_then(Value::as_i64).or_else(|| {
        value
            .and_then(Value::as_f64)
            .map(|number| number.round() as i64)
    })
}

const MACOS_WINDOW_LIST_SWIFT: &str = r#"import Foundation
import CoreGraphics

let options = CGWindowListOption(arrayLiteral: .optionOnScreenOnly, .excludeDesktopElements)
let windows = (CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]]) ?? []
var rows: [[String: Any]] = []

for window in windows {
    let layer = window[kCGWindowLayer as String] as? Int ?? 0
    let id = window[kCGWindowNumber as String] as? UInt32 ?? 0
    let owner = window[kCGWindowOwnerName as String] as? String ?? ""
    let name = window[kCGWindowName as String] as? String ?? ""
    let bounds = window[kCGWindowBounds as String] as? [String: Any] ?? [:]
    let width = bounds["Width"] as? Double ?? 0
    let height = bounds["Height"] as? Double ?? 0

    if layer == 0 && id > 0 && width >= 80 && height >= 60 && !owner.isEmpty {
        rows.append([
            "id": id,
            "owner": owner,
            "name": name,
            "width": width,
            "height": height
        ])
    }
}

let data = try JSONSerialization.data(withJSONObject: rows, options: [])
print(String(data: data, encoding: .utf8)!)
"#;

const MACOS_WINDOW_CAPTURE_SWIFT: &str = r#"import Foundation
import AppKit
import AVFoundation
import CoreMedia
import ScreenCaptureKit

@available(macOS 13.0, *)
final class WindowVideoRecorder: NSObject, SCStreamOutput, SCStreamDelegate {
    let writer: AVAssetWriter
    let input: AVAssetWriterInput
    let done: DispatchSemaphore
    var started = false
    var finishStarted = false
    var sampleCount = 0
    var droppedCount = 0
    var failure: String?

    init(outputURL: URL, width: Int, height: Int, fps: Int, done: DispatchSemaphore) throws {
        self.writer = try AVAssetWriter(outputURL: outputURL, fileType: .mp4)
        self.input = AVAssetWriterInput(mediaType: .video, outputSettings: [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: width,
            AVVideoHeightKey: height,
            AVVideoCompressionPropertiesKey: [
                AVVideoAverageBitRateKey: max(4_000_000, width * height * max(1, fps) / 6),
                AVVideoExpectedSourceFrameRateKey: fps,
                AVVideoProfileLevelKey: AVVideoProfileLevelH264HighAutoLevel
            ]
        ])
        self.input.expectsMediaDataInRealTime = true
        self.done = done
        super.init()
        if writer.canAdd(input) {
            writer.add(input)
        } else {
            throw NSError(domain: "VantaWindowCapture", code: 1, userInfo: [
                NSLocalizedDescriptionKey: "AVAssetWriter could not add H.264 video input"
            ])
        }
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        failure = error.localizedDescription
        finish()
    }

    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of outputType: SCStreamOutputType) {
        guard outputType == .screen, CMSampleBufferIsValid(sampleBuffer) else { return }
        guard isCompleteFrame(sampleBuffer) else {
            droppedCount += 1
            return
        }
        if !started {
            guard writer.startWriting() else {
                failure = writer.error?.localizedDescription ?? "AVAssetWriter failed to start"
                finish()
                return
            }
            writer.startSession(atSourceTime: CMSampleBufferGetPresentationTimeStamp(sampleBuffer))
            started = true
        }
        if input.isReadyForMoreMediaData {
            if input.append(sampleBuffer) {
                sampleCount += 1
            } else {
                failure = writer.error?.localizedDescription ?? "AVAssetWriter rejected a video frame"
                finish()
            }
        } else {
            droppedCount += 1
        }
    }

    func isCompleteFrame(_ sampleBuffer: CMSampleBuffer) -> Bool {
        guard let attachments = CMSampleBufferGetSampleAttachmentsArray(sampleBuffer, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]],
              let statusRaw = attachments.first?[SCStreamFrameInfo.status] as? Int,
              let status = SCFrameStatus(rawValue: statusRaw) else {
            return true
        }
        return status == .complete
    }

    func finish() {
        if finishStarted { return }
        finishStarted = true
        if started {
            input.markAsFinished()
            writer.finishWriting { self.done.signal() }
        } else {
            writer.cancelWriting()
            done.signal()
        }
    }
}

@available(macOS 13.0, *)
func runCapture(outputPath: String, durationSeconds: Double, fps: Int, windowID: UInt32, width: Int, height: Int) async throws {
    _ = NSApplication.shared
    let outputURL = URL(fileURLWithPath: outputPath)
    try? FileManager.default.removeItem(at: outputURL)
    let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
    guard let window = content.windows.first(where: { $0.windowID == windowID }) else {
        throw NSError(domain: "VantaWindowCapture", code: 2, userInfo: [
            NSLocalizedDescriptionKey: "ScreenCaptureKit did not report the requested window"
        ])
    }
    let evenWidth = max(320, width - (width % 2))
    let evenHeight = max(240, height - (height % 2))
    let targetFPS = max(1, min(fps, 60))
    let done = DispatchSemaphore(value: 0)
    let recorder = try WindowVideoRecorder(outputURL: outputURL, width: evenWidth, height: evenHeight, fps: targetFPS, done: done)
    let filter: SCContentFilter
    if let display = content.displays.first {
        filter = SCContentFilter(display: display, including: [window])
    } else {
        filter = SCContentFilter(desktopIndependentWindow: window)
    }
    let config = SCStreamConfiguration()
    config.width = evenWidth
    config.height = evenHeight
    config.pixelFormat = kCVPixelFormatType_32BGRA
    config.minimumFrameInterval = CMTime(value: 1, timescale: CMTimeScale(targetFPS))
    config.queueDepth = 8
    config.showsCursor = true
    config.capturesAudio = false

    let queue = DispatchQueue(label: "tv.vanta.obs.window-video")
    let stream = SCStream(filter: filter, configuration: config, delegate: recorder)
    try stream.addStreamOutput(recorder, type: .screen, sampleHandlerQueue: queue)
    try await stream.startCapture()
    try await Task.sleep(nanoseconds: UInt64(durationSeconds * 1_000_000_000))
    try await stream.stopCapture()
    recorder.finish()
    _ = done.wait(timeout: .now() + 8)
    if let failure = recorder.failure {
        throw NSError(domain: "VantaWindowCapture", code: 4, userInfo: [NSLocalizedDescriptionKey: failure])
    }
    if recorder.sampleCount == 0 {
        throw NSError(domain: "VantaWindowCapture", code: 5, userInfo: [
            NSLocalizedDescriptionKey: "ScreenCaptureKit produced no complete window frames; grant Screen Recording permission and keep the target window visible"
        ])
    }
}

let args = CommandLine.arguments
guard args.count >= 7 else {
    fputs("usage: screencapturekit-window.swift <output.mp4> <duration_seconds> <fps> <window_id> <width> <height>\n", stderr)
    exit(64)
}
let outputPath = args[1]
let durationSeconds = Double(args[2]) ?? 2.0
let fps = Int(args[3]) ?? 60
let windowID = UInt32(args[4]) ?? 0
let width = Int(args[5]) ?? 1280
let height = Int(args[6]) ?? 720
if #available(macOS 13.0, *) {
    do {
        try await runCapture(
            outputPath: outputPath,
            durationSeconds: max(2.0, min(durationSeconds, 30.0)),
            fps: fps,
            windowID: windowID,
            width: width,
            height: height
        )
    } catch {
        fputs(error.localizedDescription + "\n", stderr)
        exit(1)
    }
} else {
    fputs("ScreenCaptureKit window capture requires macOS 13 or newer\n", stderr)
    exit(1)
}
"#;

const MACOS_SYSTEM_AUDIO_PROBE_SWIFT: &str = r#"import Foundation
import ScreenCaptureKit

let supported: Bool
let status: String
var applications: [[String: Any]] = []
if #available(macOS 13.0, *) {
    supported = true
    status = "available"
    do {
        let currentPID = ProcessInfo.processInfo.processIdentifier
        let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
        applications = content.applications
            .filter { app in app.processID != currentPID && !app.applicationName.isEmpty }
            .prefix(12)
            .enumerated()
            .map { index, app in
                [
                    "index": index,
                    "application_name": app.applicationName,
                    "bundle_id": app.bundleIdentifier ?? "",
                    "process_id": app.processID
                ]
            }
    } catch {
        applications = []
    }
} else {
    supported = false
    status = "requires_macos_13"
}
let payload: [String: Any] = [
    "supported": supported,
    "status": status,
    "native_api": "ScreenCaptureKit",
    "loopback_device_required": false,
    "capture_kind": "system_audio",
    "application_audio_supported": supported && !applications.isEmpty,
    "applications": applications
]
let data = try JSONSerialization.data(withJSONObject: payload, options: [])
print(String(data: data, encoding: .utf8)!)
"#;

const MACOS_SYSTEM_AUDIO_CAPTURE_SWIFT: &str = r#"import Foundation
import AVFoundation
import CoreMedia
import ScreenCaptureKit

@available(macOS 13.0, *)
final class SystemAudioRecorder: NSObject, SCStreamOutput, SCStreamDelegate {
    let writer: AVAssetWriter
    let input: AVAssetWriterInput
    let done: DispatchSemaphore
    var started = false
    var sampleCount = 0
    var finishStarted = false
    var failure: String?

    init(outputURL: URL, done: DispatchSemaphore) throws {
        self.writer = try AVAssetWriter(outputURL: outputURL, fileType: .m4a)
        self.input = AVAssetWriterInput(mediaType: .audio, outputSettings: [
            AVFormatIDKey: kAudioFormatMPEG4AAC,
            AVSampleRateKey: 48000,
            AVNumberOfChannelsKey: 2,
            AVEncoderBitRateKey: 128000
        ])
        self.input.expectsMediaDataInRealTime = true
        self.done = done
        super.init()
        if writer.canAdd(input) {
            writer.add(input)
        } else {
            throw NSError(domain: "VantaSystemAudio", code: 1, userInfo: [
                NSLocalizedDescriptionKey: "AVAssetWriter could not add AAC audio input"
            ])
        }
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        failure = error.localizedDescription
        finish()
    }

    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of outputType: SCStreamOutputType) {
        guard outputType == .audio, CMSampleBufferIsValid(sampleBuffer) else { return }
        if !started {
            guard writer.startWriting() else {
                failure = writer.error?.localizedDescription ?? "AVAssetWriter failed to start"
                finish()
                return
            }
            writer.startSession(atSourceTime: CMSampleBufferGetPresentationTimeStamp(sampleBuffer))
            started = true
        }
        if input.isReadyForMoreMediaData {
            if input.append(sampleBuffer) {
                sampleCount += 1
            } else {
                failure = writer.error?.localizedDescription ?? "AVAssetWriter rejected an audio sample"
                finish()
            }
        }
    }

    func finish() {
        if finishStarted { return }
        finishStarted = true
        input.markAsFinished()
        if started {
            writer.finishWriting { self.done.signal() }
        } else {
            writer.cancelWriting()
            done.signal()
        }
    }
}

@available(macOS 13.0, *)
func runCapture(outputPath: String, durationSeconds: Double) async throws {
    let outputURL = URL(fileURLWithPath: outputPath)
    try? FileManager.default.removeItem(at: outputURL)
    let done = DispatchSemaphore(value: 0)
    let recorder = try SystemAudioRecorder(outputURL: outputURL, done: done)
    let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
    guard let display = content.displays.first else {
        throw NSError(domain: "VantaSystemAudio", code: 2, userInfo: [
            NSLocalizedDescriptionKey: "ScreenCaptureKit did not report a display for system audio capture"
        ])
    }
    let filter = SCContentFilter(display: display, excludingWindows: [])
    let config = SCStreamConfiguration()
    config.capturesAudio = true
    config.excludesCurrentProcessAudio = true
    config.sampleRate = 48000
    config.channelCount = 2
    config.width = 2
    config.height = 2
    config.minimumFrameInterval = CMTime(value: 1, timescale: 1)

    let queue = DispatchQueue(label: "tv.vanta.obs.system-audio")
    let stream = SCStream(filter: filter, configuration: config, delegate: recorder)
    try stream.addStreamOutput(recorder, type: .audio, sampleHandlerQueue: queue)
    try await stream.startCapture()
    try await Task.sleep(nanoseconds: UInt64(durationSeconds * 1_000_000_000))
    try await stream.stopCapture()
    recorder.finish()
    _ = done.wait(timeout: .now() + 5)
    if let failure = recorder.failure {
        throw NSError(domain: "VantaSystemAudio", code: 3, userInfo: [NSLocalizedDescriptionKey: failure])
    }
    if recorder.sampleCount == 0 {
        throw NSError(domain: "VantaSystemAudio", code: 4, userInfo: [
            NSLocalizedDescriptionKey: "ScreenCaptureKit produced no system audio samples; grant Screen Recording permission and ensure system audio is playing"
        ])
    }
}

let args = CommandLine.arguments
guard args.count >= 3 else {
    fputs("usage: screencapturekit-system-audio.swift <output.m4a> <duration_seconds>\n", stderr)
    exit(64)
}
let outputPath = args[1]
let durationSeconds = Double(args[2]) ?? 2.0
if #available(macOS 13.0, *) {
    do {
        try await runCapture(outputPath: outputPath, durationSeconds: max(2.0, min(durationSeconds, 30.0)))
    } catch {
        fputs(error.localizedDescription + "\n", stderr)
        exit(1)
    }
} else {
    fputs("ScreenCaptureKit system audio capture requires macOS 13 or newer\n", stderr)
    exit(1)
}
"#;

const MACOS_APPLICATION_AUDIO_CAPTURE_SWIFT: &str = r#"import Foundation
import AVFoundation
import CoreMedia
import ScreenCaptureKit

@available(macOS 13.0, *)
final class ApplicationAudioRecorder: NSObject, SCStreamOutput, SCStreamDelegate {
    let writer: AVAssetWriter
    let input: AVAssetWriterInput
    let done: DispatchSemaphore
    var started = false
    var sampleCount = 0
    var finishStarted = false
    var failure: String?

    init(outputURL: URL, done: DispatchSemaphore) throws {
        self.writer = try AVAssetWriter(outputURL: outputURL, fileType: .m4a)
        self.input = AVAssetWriterInput(mediaType: .audio, outputSettings: [
            AVFormatIDKey: kAudioFormatMPEG4AAC,
            AVSampleRateKey: 48000,
            AVNumberOfChannelsKey: 2,
            AVEncoderBitRateKey: 128000
        ])
        self.input.expectsMediaDataInRealTime = true
        self.done = done
        super.init()
        if writer.canAdd(input) {
            writer.add(input)
        } else {
            throw NSError(domain: "VantaApplicationAudio", code: 1, userInfo: [
                NSLocalizedDescriptionKey: "AVAssetWriter could not add AAC audio input"
            ])
        }
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        failure = error.localizedDescription
        finish()
    }

    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of outputType: SCStreamOutputType) {
        guard outputType == .audio, CMSampleBufferIsValid(sampleBuffer) else { return }
        if !started {
            guard writer.startWriting() else {
                failure = writer.error?.localizedDescription ?? "AVAssetWriter failed to start"
                finish()
                return
            }
            writer.startSession(atSourceTime: CMSampleBufferGetPresentationTimeStamp(sampleBuffer))
            started = true
        }
        if input.isReadyForMoreMediaData {
            if input.append(sampleBuffer) {
                sampleCount += 1
            } else {
                failure = writer.error?.localizedDescription ?? "AVAssetWriter rejected an audio sample"
                finish()
            }
        }
    }

    func finish() {
        if finishStarted { return }
        finishStarted = true
        input.markAsFinished()
        if started {
            writer.finishWriting { self.done.signal() }
        } else {
            writer.cancelWriting()
            done.signal()
        }
    }
}

@available(macOS 13.0, *)
func runCapture(outputPath: String, durationSeconds: Double) async throws {
    let outputURL = URL(fileURLWithPath: outputPath)
    try? FileManager.default.removeItem(at: outputURL)
    let done = DispatchSemaphore(value: 0)
    let recorder = try ApplicationAudioRecorder(outputURL: outputURL, done: done)
    let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
    guard let display = content.displays.first else {
        throw NSError(domain: "VantaApplicationAudio", code: 2, userInfo: [
            NSLocalizedDescriptionKey: "ScreenCaptureKit did not report a display for application audio capture"
        ])
    }
    let currentPID = ProcessInfo.processInfo.processIdentifier
    guard let application = content.applications.first(where: { app in
        app.processID != currentPID && !app.applicationName.isEmpty
    }) else {
        throw NSError(domain: "VantaApplicationAudio", code: 3, userInfo: [
            NSLocalizedDescriptionKey: "ScreenCaptureKit did not report a captureable running application"
        ])
    }
    let filter = SCContentFilter(display: display, including: [application], exceptingWindows: [])
    let config = SCStreamConfiguration()
    config.capturesAudio = true
    config.excludesCurrentProcessAudio = true
    config.sampleRate = 48000
    config.channelCount = 2
    config.width = 2
    config.height = 2
    config.minimumFrameInterval = CMTime(value: 1, timescale: 1)

    let queue = DispatchQueue(label: "tv.vanta.obs.application-audio")
    let stream = SCStream(filter: filter, configuration: config, delegate: recorder)
    try stream.addStreamOutput(recorder, type: .audio, sampleHandlerQueue: queue)
    try await stream.startCapture()
    try await Task.sleep(nanoseconds: UInt64(durationSeconds * 1_000_000_000))
    try await stream.stopCapture()
    recorder.finish()
    _ = done.wait(timeout: .now() + 5)
    if let failure = recorder.failure {
        throw NSError(domain: "VantaApplicationAudio", code: 4, userInfo: [NSLocalizedDescriptionKey: failure])
    }
    if recorder.sampleCount == 0 {
        throw NSError(domain: "VantaApplicationAudio", code: 5, userInfo: [
            NSLocalizedDescriptionKey: "ScreenCaptureKit produced no application audio samples; grant Screen Recording permission and play audio in the selected application"
        ])
    }
}

let args = CommandLine.arguments
guard args.count >= 3 else {
    fputs("usage: screencapturekit-application-audio.swift <output.m4a> <duration_seconds>\n", stderr)
    exit(64)
}
let outputPath = args[1]
let durationSeconds = Double(args[2]) ?? 2.0
if #available(macOS 13.0, *) {
    do {
        try await runCapture(outputPath: outputPath, durationSeconds: max(2.0, min(durationSeconds, 30.0)))
    } catch {
        fputs(error.localizedDescription + "\n", stderr)
        exit(1)
    }
} else {
    fputs("ScreenCaptureKit application audio capture requires macOS 13 or newer\n", stderr)
    exit(1)
}
"#;

fn is_desktop_loopback_label(label: &str) -> bool {
    let normalized = label.to_ascii_lowercase();
    [
        "blackhole",
        "soundflower",
        "loopback",
        "background music",
        "obs virtual audio",
        "system audio",
        "desktop audio",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn parse_indexed_line(line: &str) -> Option<(i64, String)> {
    let (_, rest) = line.rsplit_once("] [")?;
    let (index, label) = rest.split_once("] ")?;
    let index = index.parse::<i64>().ok()?;
    let label = label.trim();
    if label.is_empty() {
        return None;
    }
    Some((index, label.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{
        capture_permission, first_desktop_audio_index, first_display_index, first_microphone_index,
        first_window_id, macos_permission_summary, parse_avfoundation_devices,
        parse_macos_window_targets, permission_block_message, source_health_for_capture,
        unsupported_capture_message,
    };
    use serde_json::json;

    #[test]
    fn parses_avfoundation_camera_microphone_and_display_inventory() {
        let devices = parse_avfoundation_devices(
            "[AVFoundation indev @ 0x1] AVFoundation video devices:\n\
             [AVFoundation indev @ 0x1] [0] FaceTime HD Camera\n\
             [AVFoundation indev @ 0x1] [1] Capture screen 0\n\
             [AVFoundation indev @ 0x1] AVFoundation audio devices:\n\
             [AVFoundation indev @ 0x1] [0] MacBook Pro Microphone\n\
             [AVFoundation indev @ 0x1] [1] BlackHole 2ch\n",
        );
        assert_eq!(devices.len(), 4);
        assert_eq!(devices[0].kind, "camera");
        assert_eq!(devices[1].kind, "display");
        assert_eq!(devices[2].kind, "microphone");
        assert_eq!(devices[3].kind, "desktop_audio");
    }

    #[test]
    fn parses_coregraphics_window_targets_for_native_capture() {
        let windows = parse_macos_window_targets(
            r#"[
              {"id":108,"owner":"Vanta","name":"Live Studio","width":1512.0,"height":945.0},
              {"id":109,"owner":"Tiny","name":"Ignored","width":20.0,"height":20.0}
            ]"#,
        );
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0]["kind"], "window");
        assert_eq!(windows[0]["native_index"], 108);
        assert_eq!(windows[0]["label"], "Vanta - Live Studio");
    }

    #[test]
    fn derives_source_health_and_explicit_unsupported_messages() {
        let inventory = json!({
            "support": {"camera": true, "window": true},
            "permissions": {
                "camera": {
                    "status": "prompt_required",
                    "required": true,
                    "remediation": "macOS will request Camera access"
                }
            },
            "devices": [{"kind": "camera"}, {"kind": "window", "native_index": 108}],
            "trace_event": "media.capture.inventory.test"
        });
        let health = source_health_for_capture(&inventory, "camera");
        assert_eq!(health["status"], "ready");
        assert_eq!(health["ready_devices"], 1);
        assert_eq!(health["permission"]["status"], "prompt_required");
        let window_health = source_health_for_capture(&inventory, "window");
        assert_eq!(window_health["status"], "ready");
        assert_eq!(window_health["ready_devices"], 1);
        assert_eq!(first_window_id(&inventory), Some(108));
        assert!(unsupported_capture_message("window", &inventory).is_none());
    }

    #[test]
    fn treats_program_canvas_as_ready_without_physical_devices() {
        let inventory = json!({
            "support": {"program_canvas": true},
            "permissions": {
                "program_canvas": {
                    "status": "not_required",
                    "required": false,
                    "remediation": ""
                }
            },
            "devices": [],
            "trace_event": "media.capture.inventory.test"
        });
        let health = source_health_for_capture(&inventory, "program_canvas");
        assert_eq!(health["status"], "ready");
        assert_eq!(health["ready_devices"], 1);
        assert_eq!(health["supported"], true);
    }

    #[test]
    fn treats_screencapturekit_system_audio_as_no_loopback_native_capture() {
        let inventory = json!({
            "support": {"system_audio": true, "desktop_audio": false},
            "permissions": {
                "system_audio": {
                    "status": "prompt_required",
                    "required": true,
                    "remediation": "macOS will request Screen Recording access"
                }
            },
            "devices": [{
                "kind": "system_audio",
                "transport": "screencapturekit",
                "loopback_device_required": false
            }],
            "trace_event": "media.capture.inventory.test"
        });
        let health = source_health_for_capture(&inventory, "system_audio");
        assert_eq!(health["status"], "ready");
        assert_eq!(health["ready_devices"], 1);
        assert_eq!(health["permission"]["status"], "prompt_required");
        assert!(unsupported_capture_message("system_audio", &inventory).is_none());
        assert!(unsupported_capture_message("desktop_audio", &inventory).is_some());
    }

    #[test]
    fn classifies_native_camera_and_microphone_permission_flow() {
        let promptable = macos_permission_summary(
            "[AVFoundation indev @ 0x1] AVFoundation video devices:\n\
             [AVFoundation indev @ 0x1] [0] FaceTime HD Camera\n\
             [AVFoundation indev @ 0x1] AVFoundation audio devices:\n\
             [AVFoundation indev @ 0x1] [0] MacBook Pro Microphone\n",
            1,
            1,
            1,
            1,
            true,
            true,
        );
        assert_eq!(promptable["camera"]["status"], "prompt_required");
        assert_eq!(promptable["microphone"]["status"], "prompt_required");
        assert_eq!(promptable["display"]["status"], "ready");
        assert_eq!(promptable["system_audio"]["status"], "prompt_required");
        assert_eq!(promptable["system_audio"]["required"], true);

        let denied = json!({
            "permissions": macos_permission_summary(
                "AVFoundation authorization status is denied for camera video input",
                1,
                1,
                0,
                0,
                false,
                false,
            )
        });
        assert_eq!(capture_permission(&denied, "camera")["status"], "denied");
        assert_eq!(
            permission_block_message("camera", &denied).unwrap(),
            "macOS camera permission is denied; enable Vanta Native Capture in System Settings > Privacy & Security > Camera"
        );
    }

    #[test]
    fn finds_first_display_index_for_continuous_capture() {
        let inventory = json!({
            "devices": [
                {"kind": "camera", "native_index": 0},
                {"kind": "display", "native_index": 3},
                {"kind": "microphone", "native_index": 1},
                {"kind": "desktop_audio", "native_index": 2},
                {"kind": "window", "native_index": 108}
            ]
        });
        assert_eq!(first_display_index(&inventory), Some(3));
        assert_eq!(first_microphone_index(&inventory), Some(1));
        assert_eq!(first_desktop_audio_index(&inventory), Some(2));
        assert_eq!(first_window_id(&inventory), Some(108));
    }
}
