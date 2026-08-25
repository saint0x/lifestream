use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use base64::{Engine as _, engine::general_purpose};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{fs, process::Command};

use crate::native::{
    protocol::NativeHelperCommandInput,
    service::{NativeService, NativeServiceError},
};

use super::{
    capture::{
        NativeCaptureError, capture_application_audio_segment, capture_desktop_audio_segment,
        capture_display_segment, capture_microphone_segment, capture_preview_frame,
        capture_system_audio_segment, native_capture_inventory, permission_block_message,
        source_health_for_capture, unsupported_capture_message,
    },
    domain::{
        CaptureStartInput, EncodeStartInput, RuntimeProgramFrameInput, RuntimeSourceFrameInput,
        RuntimeSourcePlayoutInput, SourceAudioIngestInput,
    },
    ffmpeg::{FfmpegError, FfmpegMediaEngine, PackageRequest, RenderRequest},
    source::{SourceMediaError, ingest_source_audio},
    store::{MediaStore, MediaStoreError},
};

#[derive(Clone)]
pub struct MediaService {
    store: Arc<MediaStore>,
    native: Arc<NativeService>,
    ffmpeg: FfmpegMediaEngine,
}

impl MediaService {
    pub fn new(store: MediaStore, native: Arc<NativeService>) -> Self {
        Self {
            store: Arc::new(store),
            native,
            ffmpeg: FfmpegMediaEngine,
        }
    }

    pub async fn start_capture(
        &self,
        input: CaptureStartInput,
    ) -> Result<Value, MediaServiceError> {
        validate_capture(&input)?;
        let inventory = native_capture_inventory().await?;
        if let Some(message) = unsupported_capture_message(&input.capture_kind, &inventory) {
            return Err(MediaServiceError::Invalid {
                field: "capture_kind",
                message,
            });
        }
        if let Some(message) = permission_block_message(&input.capture_kind, &inventory) {
            return Err(MediaServiceError::Invalid {
                field: "capture_kind",
                message,
            });
        }
        let source_health = source_health_for_capture(&inventory, &input.capture_kind);
        let helper = self
            .native
            .start_session(crate::native::protocol::NativeHelperStartInput {
                helper_kind: "capture".to_string(),
                launch_mode: Some("managed".to_string()),
                binary_path: None,
                endpoint: None,
            })
            .await?;
        let helper_session_id = text(&helper, "id")?;
        let command_payload = json!({
            "source_id": input.source_id,
            "capture_kind": input.capture_kind,
            "width": input.width,
            "height": input.height,
            "frame_rate": input.frame_rate,
            "audio": input.audio.unwrap_or(false),
            "source_health": source_health,
            "inventory": inventory
        });
        let command = self
            .native
            .command(
                &helper_session_id,
                NativeHelperCommandInput {
                    command_kind: "prepare_capture".to_string(),
                    payload_json: Some(command_payload),
                },
            )
            .await?;
        Ok(self
            .store
            .create_capture_session(input, helper_session_id, command, source_health)
            .await?)
    }

    pub async fn stop_capture(&self, session_id: &str) -> Result<Value, MediaServiceError> {
        require_text(session_id, "session_id")?;
        Ok(self.store.stop_capture_session(session_id).await?)
    }

    pub async fn reconcile_capture(&self, session_id: &str) -> Result<Value, MediaServiceError> {
        require_text(session_id, "session_id")?;
        let session = self.store.capture_session(session_id).await?;
        if value_text(&session, "status") == "stopped" {
            return Err(MediaServiceError::Invalid {
                field: "session_id",
                message: "must reference a live or reconnecting capture session",
            });
        }
        let capture_kind = value_text(&session, "capture_kind");
        let inventory = native_capture_inventory().await?;
        let source_health = source_health_for_capture(&inventory, &capture_kind);
        let source_status = value_text(&source_health, "status");
        let next_status = if source_status == "ready" {
            "capturing"
        } else {
            "reconnecting"
        };
        let reconnect = json!({
            "status": if source_status == "ready" { "recovered" } else { "waiting_for_source" },
            "source_status": source_status,
            "capture_kind": capture_kind,
            "attempted_at": chrono::Utc::now().to_rfc3339(),
            "strategy": "same_helper_session_inventory_reconcile"
        });
        let helper_session_id = text(&session, "helper_session_id")?;
        let command_payload = json!({
            "capture_session_id": session_id,
            "source_id": value_text(&session, "source_id"),
            "capture_kind": capture_kind,
            "target_status": next_status,
            "source_health": source_health,
            "inventory": inventory,
            "reconnect": reconnect
        });
        let command = self
            .native
            .command(
                &helper_session_id,
                NativeHelperCommandInput {
                    command_kind: "reconcile_capture".to_string(),
                    payload_json: Some(command_payload),
                },
            )
            .await?;
        Ok(self
            .store
            .reconcile_capture_session(session_id, next_status, source_health, reconnect, command)
            .await?)
    }

    pub async fn capture_preview_frame(
        &self,
        session_id: &str,
    ) -> Result<Value, MediaServiceError> {
        require_text(session_id, "session_id")?;
        let session = self.store.capture_session(session_id).await?;
        if value_text(&session, "status") != "capturing" {
            return Err(MediaServiceError::Invalid {
                field: "session_id",
                message: "must reference an active capture session",
            });
        }
        let capture_kind = value_text(&session, "capture_kind");
        if !matches!(
            capture_kind.as_str(),
            "camera" | "display" | "program_canvas" | "window"
        ) {
            return Err(MediaServiceError::Invalid {
                field: "session_id",
                message: "must reference a camera-backed, display-backed, or window-backed capture session",
            });
        }
        let result = capture_preview_frame(session_id, &capture_kind).await?;
        Ok(self
            .store
            .create_capture_frame(session_id, &result.artifact_path, result.validation_json)
            .await?)
    }

    pub async fn ingest_runtime_program_frame(
        &self,
        session_id: &str,
        input: RuntimeProgramFrameInput,
    ) -> Result<Value, MediaServiceError> {
        require_text(session_id, "session_id")?;
        require_one_of(
            &input.compositor_backend,
            "compositor_backend",
            &["webgl_gpu", "canvas_2d"],
        )?;
        require_range(input.frame_sequence, "frame_sequence", 1, i64::MAX)?;
        let session = self.store.capture_session(session_id).await?;
        if value_text(&session, "status") != "capturing" {
            return Err(MediaServiceError::Invalid {
                field: "session_id",
                message: "must reference an active capture session",
            });
        }
        if value_text(&session, "capture_kind") != "program_canvas" {
            return Err(MediaServiceError::Invalid {
                field: "session_id",
                message: "must reference a program canvas capture session",
            });
        }
        let png = decode_png_data_url(&input.image_data_url)?;
        let (width, height) = png_dimensions(&png).ok_or(MediaServiceError::Invalid {
            field: "image_data_url",
            message: "must contain a valid PNG image",
        })?;
        let expected_width = value_int(&session, "width") as u32;
        let expected_height = value_int(&session, "height") as u32;
        if width != expected_width || height != expected_height {
            return Err(MediaServiceError::Invalid {
                field: "image_data_url",
                message: "must match the active capture session dimensions",
            });
        }
        let base = media_dir()
            .await?
            .join("runtime-program-frames")
            .join(safe_path_segment(session_id));
        fs::create_dir_all(&base).await?;
        let artifact_path = base.join(format!("program-frame-{:08}.png", input.frame_sequence));
        fs::write(&artifact_path, &png).await?;
        let sha256 = Sha256::digest(&png);
        let validation = json!({
            "playable": true,
            "capture_kind": "program_canvas",
            "frame_kind": "runtime_program_canvas_png",
            "runtime_backed_program_output": true,
            "browser_preview_authoritative": false,
            "compositor_backend": input.compositor_backend,
            "frame_sequence": input.frame_sequence,
            "captured_at_ms": input.captured_at_ms.unwrap_or_default(),
            "width": width,
            "height": height,
            "byte_length": png.len(),
            "sha256": format!("{sha256:x}"),
            "transport_contract": {
                "source": "browser_canvas_capture",
                "format": "png",
                "program_clock_ready": true
            },
            "frame_pacing": {
                "mode": "runtime_program_clock",
                "target_frame_rate": value_int(&session, "frame_rate"),
                "dropped_frames": 0,
                "reported_for_source_kind": "program_canvas"
            }
        });
        Ok(self
            .store
            .create_capture_frame_with_kind(
                session_id,
                &artifact_path.to_string_lossy(),
                "runtime_program_canvas_png",
                validation,
            )
            .await?)
    }

    pub async fn ingest_runtime_source_frame(
        &self,
        session_id: &str,
        input: RuntimeSourceFrameInput,
    ) -> Result<Value, MediaServiceError> {
        require_text(session_id, "session_id")?;
        require_one_of(
            &input.compositor_backend,
            "compositor_backend",
            &["webgl_gpu", "canvas_2d", "runtime_headless_browser"],
        )?;
        require_one_of(
            &input.surface_kind,
            "surface_kind",
            &["browser_source", "remote_web_surface"],
        )?;
        require_range(input.frame_sequence, "frame_sequence", 1, i64::MAX)?;
        require_range(
            input.dropped_frames.unwrap_or(0),
            "dropped_frames",
            0,
            i64::MAX,
        )?;
        require_range(
            input.reconnect_count.unwrap_or(0),
            "reconnect_count",
            0,
            i64::MAX,
        )?;
        require_range(
            input.ingest_latency_ms.unwrap_or(0),
            "ingest_latency_ms",
            0,
            i64::MAX,
        )?;
        let session = self.store.capture_session(session_id).await?;
        if value_text(&session, "status") != "capturing" {
            return Err(MediaServiceError::Invalid {
                field: "session_id",
                message: "must reference an active capture session",
            });
        }
        if value_text(&session, "capture_kind") != "browser_surface" {
            return Err(MediaServiceError::Invalid {
                field: "session_id",
                message: "must reference a browser surface capture session",
            });
        }
        let source = self
            .store
            .obs_source(&value_text(&session, "source_id"))
            .await?;
        let source_kind = value_text(&source, "source_kind");
        if !matches!(
            source_kind.as_str(),
            "browser_capture" | "remote_contribution"
        ) {
            return Err(MediaServiceError::Invalid {
                field: "session_id",
                message: "must reference a browser or remote web surface source",
            });
        }
        let png = decode_png_data_url(&input.image_data_url)?;
        let (width, height) = png_dimensions(&png).ok_or(MediaServiceError::Invalid {
            field: "image_data_url",
            message: "must contain a valid PNG image",
        })?;
        let expected_width = value_int(&session, "width") as u32;
        let expected_height = value_int(&session, "height") as u32;
        if width != expected_width || height != expected_height {
            return Err(MediaServiceError::Invalid {
                field: "image_data_url",
                message: "must match the active capture session dimensions",
            });
        }
        let source_id = value_text(&session, "source_id");
        let base = media_dir()
            .await?
            .join("runtime-source-frames")
            .join(safe_path_segment(&source_id))
            .join(safe_path_segment(session_id));
        fs::create_dir_all(&base).await?;
        let artifact_path = base.join(format!("source-frame-{:08}.png", input.frame_sequence));
        fs::write(&artifact_path, &png).await?;
        let sha256 = Sha256::digest(&png);
        let dropped_frames = input.dropped_frames.unwrap_or(0);
        let reconnect_count = input.reconnect_count.unwrap_or(0);
        let ingest_latency_ms = input.ingest_latency_ms.unwrap_or(0);
        let health = browser_surface_health(dropped_frames, reconnect_count, ingest_latency_ms);
        let validation = json!({
            "playable": true,
            "capture_kind": "browser_surface",
            "frame_kind": "runtime_browser_surface_png",
            "runtime_backed_source_output": true,
            "browser_preview_authoritative": false,
            "sandboxed_iframe_pixels_read": false,
            "source_id": source_id,
            "source_kind": source_kind,
            "browser_url": source.get("browser_url").cloned().unwrap_or(Value::Null),
            "surface_kind": input.surface_kind,
            "compositor_backend": input.compositor_backend,
            "frame_sequence": input.frame_sequence,
            "captured_at_ms": input.captured_at_ms.unwrap_or_default(),
            "width": width,
            "height": height,
            "byte_length": png.len(),
            "sha256": format!("{sha256:x}"),
            "long_session": health,
            "transport_contract": {
                "source": "vanta_runtime_browser_surface",
                "format": "png",
                "program_clock_ready": true,
                "authority": "runtime_bridge"
            }
        });
        Ok(self
            .store
            .create_capture_frame_with_kind(
                session_id,
                &artifact_path.to_string_lossy(),
                "runtime_browser_surface_png",
                validation,
            )
            .await?)
    }

    pub async fn create_runtime_source_playout(
        &self,
        session_id: &str,
        input: RuntimeSourcePlayoutInput,
    ) -> Result<Value, MediaServiceError> {
        require_text(session_id, "session_id")?;
        let target_frame_rate = input.target_frame_rate.unwrap_or(30);
        let frame_count = input.frame_count.unwrap_or(8);
        require_range(target_frame_rate, "target_frame_rate", 1, 120)?;
        require_range(frame_count, "frame_count", 2, 240)?;
        let session = self.store.capture_session(session_id).await?;
        if value_text(&session, "status") != "capturing" {
            return Err(MediaServiceError::Invalid {
                field: "session_id",
                message: "must reference an active capture session",
            });
        }
        if value_text(&session, "capture_kind") != "browser_surface" {
            return Err(MediaServiceError::Invalid {
                field: "session_id",
                message: "must reference a browser surface capture session",
            });
        }
        let mut frames = self
            .store
            .capture_frames(session_id)
            .await?
            .into_iter()
            .filter(|frame| value_text(frame, "frame_kind") == "runtime_browser_surface_png")
            .take(frame_count as usize)
            .collect::<Vec<_>>();
        if frames.len() < 2 {
            return Err(MediaServiceError::Invalid {
                field: "frame_count",
                message: "requires at least two validated runtime source frames",
            });
        }
        frames.sort_by_key(|frame| {
            frame
                .get("validation_json")
                .and_then(|value| value.get("frame_sequence"))
                .and_then(Value::as_i64)
                .unwrap_or_default()
        });
        let playout_id = format!(
            "browser_surface_playout_{}",
            safe_path_segment(&uuid::Uuid::new_v4().simple().to_string())
        );
        let base = media_dir()
            .await?
            .join("runtime-source-playout")
            .join(safe_path_segment(session_id))
            .join(&playout_id);
        fs::create_dir_all(&base).await?;
        for (index, frame) in frames.iter().enumerate() {
            let source = PathBuf::from(value_text(frame, "artifact_path"));
            let target = base.join(format!("frame-{:08}.png", index + 1));
            fs::copy(&source, &target).await?;
        }
        let output_path = base.join("browser-surface-playout.mp4");
        let encode = encode_browser_surface_playout(&base, &output_path, target_frame_rate).await?;
        let validation =
            validate_browser_surface_playout(&output_path, frames.len() as i64, target_frame_rate)
                .await?;
        let first = frames
            .first()
            .and_then(|frame| frame.get("validation_json"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        let last = frames
            .last()
            .and_then(|frame| frame.get("validation_json"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        let cumulative_dropped_frames = frames
            .iter()
            .map(|frame| {
                frame
                    .get("validation_json")
                    .and_then(|value| value.get("long_session"))
                    .and_then(|value| value.get("dropped_frames"))
                    .and_then(Value::as_i64)
                    .unwrap_or_default()
            })
            .sum::<i64>();
        let max_ingest_latency_ms = frames
            .iter()
            .filter_map(|frame| {
                frame
                    .get("validation_json")
                    .and_then(|value| value.get("long_session"))
                    .and_then(|value| value.get("ingest_latency_ms"))
                    .and_then(Value::as_i64)
            })
            .max()
            .unwrap_or_default();
        let artifact_validation = json!({
            "playable": true,
            "capture_kind": "browser_surface",
            "artifact_kind": "runtime_browser_surface_playout_mp4",
            "sustained_runtime_loop": true,
            "runtime_backed_program_output": true,
            "browser_preview_authoritative": false,
            "source_id": value_text(&session, "source_id"),
            "source_kind": last.get("source_kind").cloned().unwrap_or(Value::Null),
            "surface_kind": last.get("surface_kind").cloned().unwrap_or(Value::Null),
            "first_frame_sequence": first.get("frame_sequence").cloned().unwrap_or_else(|| json!(0)),
            "last_frame_sequence": last.get("frame_sequence").cloned().unwrap_or_else(|| json!(0)),
            "frame_count": frames.len(),
            "target_frame_rate": target_frame_rate,
            "duration_seconds": frames.len() as f64 / target_frame_rate as f64,
            "cumulative_dropped_frames": cumulative_dropped_frames,
            "max_ingest_latency_ms": max_ingest_latency_ms,
            "encoder": encode,
            "validation": validation,
            "runtime_delivery": {
                "transport": "vanta_realtime_sfu",
                "program_surface": "browser_source_program_surface",
                "frame_source": "runtime_browser_surface_playout_chunk",
                "policy": "pace_source_frames_on_program_clock",
                "continuity_action": if cumulative_dropped_frames > 120 || max_ingest_latency_ms > 2500 {
                    "hold_last_good_source_frame_and_reduce_refresh_rate"
                } else {
                    "play_current_source_frames"
                }
            },
            "frame_pacing": {
                "mode": "runtime_program_clock",
                "target_frame_rate": target_frame_rate,
                "dropped_frames": cumulative_dropped_frames,
                "max_ingest_latency_ms": max_ingest_latency_ms,
                "reported_for_source_kind": last.get("source_kind").cloned().unwrap_or(Value::Null)
            }
        });
        Ok(self
            .store
            .create_capture_artifact(
                session_id,
                "runtime_browser_surface_playout_mp4",
                &output_path.to_string_lossy(),
                artifact_validation,
            )
            .await?)
    }

    pub async fn capture_segment(&self, session_id: &str) -> Result<Value, MediaServiceError> {
        require_text(session_id, "session_id")?;
        let session = self.store.capture_session(session_id).await?;
        if value_text(&session, "status") != "capturing" {
            return Err(MediaServiceError::Invalid {
                field: "session_id",
                message: "must reference an active capture session",
            });
        }
        let capture_kind = value_text(&session, "capture_kind");
        let (artifact_kind, result) = match capture_kind.as_str() {
            "camera" | "display" | "program_canvas" | "window" => (
                if capture_kind == "camera" {
                    "live_camera_mp4"
                } else if capture_kind == "window" {
                    "continuous_window_mp4"
                } else {
                    "continuous_display_mp4"
                },
                capture_display_segment(
                    session_id,
                    &capture_kind,
                    value_int(&session, "frame_rate"),
                    capture_duration_seconds(&session),
                )
                .await?,
            ),
            "microphone" => (
                "live_microphone_m4a",
                capture_microphone_segment(
                    session_id,
                    &capture_kind,
                    capture_duration_seconds(&session),
                )
                .await?,
            ),
            "desktop_audio" => (
                "live_desktop_audio_m4a",
                capture_desktop_audio_segment(
                    session_id,
                    &capture_kind,
                    capture_duration_seconds(&session),
                )
                .await?,
            ),
            "system_audio" => (
                "live_system_audio_m4a",
                capture_system_audio_segment(
                    session_id,
                    &capture_kind,
                    capture_duration_seconds(&session),
                )
                .await?,
            ),
            "application_audio" => (
                "live_application_audio_m4a",
                capture_application_audio_segment(
                    session_id,
                    &capture_kind,
                    capture_duration_seconds(&session),
                )
                .await?,
            ),
            _ => {
                return Err(MediaServiceError::Invalid {
                    field: "session_id",
                    message: "must reference a display-backed or audio capture session",
                });
            }
        };
        Ok(self
            .store
            .create_capture_artifact(
                session_id,
                artifact_kind,
                &result.artifact_path,
                result.validation_json,
            )
            .await?)
    }

    pub async fn capture_sessions(&self) -> Result<Vec<Value>, MediaServiceError> {
        Ok(self.store.capture_sessions().await?)
    }

    pub async fn capture_frames(&self, session_id: &str) -> Result<Vec<Value>, MediaServiceError> {
        require_text(session_id, "session_id")?;
        Ok(self.store.capture_frames(session_id).await?)
    }

    pub async fn capture_artifacts(
        &self,
        session_id: &str,
    ) -> Result<Vec<Value>, MediaServiceError> {
        require_text(session_id, "session_id")?;
        Ok(self.store.capture_artifacts(session_id).await?)
    }

    pub async fn ingest_source_audio(
        &self,
        input: SourceAudioIngestInput,
    ) -> Result<Value, MediaServiceError> {
        require_text(&input.source_id, "source_id")?;
        require_text(&input.input_path, "input_path")?;
        let result = ingest_source_audio(&input.source_id, &input.input_path).await?;
        Ok(self
            .store
            .create_source_artifact(
                &input.source_id,
                "media_source_audio_m4a",
                &result.artifact_path,
                result.validation_json,
            )
            .await?)
    }

    pub async fn source_artifacts(&self, source_id: &str) -> Result<Vec<Value>, MediaServiceError> {
        require_text(source_id, "source_id")?;
        Ok(self.store.source_artifacts(source_id).await?)
    }

    pub async fn capabilities(&self) -> Result<Value, MediaServiceError> {
        Ok(self.ffmpeg.capabilities().await?)
    }

    pub async fn capture_inventory(&self) -> Result<Value, MediaServiceError> {
        Ok(native_capture_inventory().await?)
    }

    pub async fn start_encode(&self, input: EncodeStartInput) -> Result<Value, MediaServiceError> {
        validate_encode(&input)?;
        let capture = self
            .store
            .capture_session(&input.capture_session_id)
            .await?;
        if value_text(&capture, "status") != "capturing" {
            return Err(MediaServiceError::Invalid {
                field: "capture_session_id",
                message: "must reference an active capture session",
            });
        }
        let helper = self
            .native
            .start_session(crate::native::protocol::NativeHelperStartInput {
                helper_kind: "encode".to_string(),
                launch_mode: Some("managed".to_string()),
                binary_path: None,
                endpoint: None,
            })
            .await?;
        let helper_session_id = text(&helper, "id")?;
        let command_payload = json!({
            "capture_session_id": input.capture_session_id,
            "codec": input.codec,
            "audio_codec": input.audio_codec,
            "container": input.container,
            "bitrate_kbps": input.bitrate_kbps,
            "keyframe_interval_seconds": input.keyframe_interval_seconds,
            "latency_profile": input.latency_profile
        });
        let command = self
            .native
            .command(
                &helper_session_id,
                NativeHelperCommandInput {
                    command_kind: "prepare_encode".to_string(),
                    payload_json: Some(command_payload),
                },
            )
            .await?;
        Ok(self
            .store
            .create_encode_job(input, helper_session_id, command)
            .await?)
    }

    pub async fn stop_encode(&self, job_id: &str) -> Result<Value, MediaServiceError> {
        require_text(job_id, "job_id")?;
        Ok(self.store.stop_encode_job(job_id).await?)
    }

    pub async fn render_encode(&self, job_id: &str) -> Result<Value, MediaServiceError> {
        require_text(job_id, "job_id")?;
        let job = self.store.encode_job(job_id).await?;
        let capture = self
            .store
            .capture_session(&value_text(&job, "capture_session_id"))
            .await?;
        let output = self
            .ffmpeg
            .render(RenderRequest {
                job_id: value_text(&job, "id"),
                codec: value_text(&job, "codec"),
                audio_codec: value_text(&job, "audio_codec"),
                container: value_text(&job, "container"),
                bitrate_kbps: value_int(&job, "bitrate_kbps"),
                keyframe_interval_seconds: value_int(&job, "keyframe_interval_seconds"),
                latency_profile: value_text(&job, "latency_profile"),
                width: value_int(&capture, "width"),
                height: value_int(&capture, "height"),
                frame_rate: value_int(&capture, "frame_rate"),
                duration_seconds: capture_duration_seconds(&capture),
            })
            .await?;
        Ok(self
            .store
            .mark_encode_rendered(job_id, &output.output_path, output.validation_json)
            .await?)
    }

    pub async fn package_encode(&self, job_id: &str) -> Result<Value, MediaServiceError> {
        require_text(job_id, "job_id")?;
        let job = self.store.encode_job(job_id).await?;
        if value_text(&job, "status") != "playable" {
            return Err(MediaServiceError::Invalid {
                field: "job_id",
                message: "must reference a playable encode job",
            });
        }
        let packaged = self
            .ffmpeg
            .package_hls(PackageRequest {
                job_id: value_text(&job, "id"),
                input_path: value_text(&job, "output_path"),
            })
            .await?;
        Ok(self
            .store
            .create_package(
                job_id,
                "hls_cmaf",
                &packaged.manifest_path,
                packaged.package_json,
            )
            .await?)
    }

    pub async fn packages(&self) -> Result<Vec<Value>, MediaServiceError> {
        Ok(self.store.packages().await?)
    }

    pub async fn encode_jobs(&self) -> Result<Vec<Value>, MediaServiceError> {
        Ok(self.store.encode_jobs().await?)
    }
}

#[derive(Debug, Error)]
pub enum MediaServiceError {
    #[error(transparent)]
    Store(#[from] MediaStoreError),
    #[error(transparent)]
    Native(#[from] NativeServiceError),
    #[error(transparent)]
    Ffmpeg(#[from] FfmpegError),
    #[error(transparent)]
    NativeCapture(#[from] NativeCaptureError),
    #[error(transparent)]
    SourceMedia(#[from] SourceMediaError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("invalid {field}: {message}")]
    Invalid {
        field: &'static str,
        message: &'static str,
    },
}

fn decode_png_data_url(value: &str) -> Result<Vec<u8>, MediaServiceError> {
    let Some(encoded) = value.strip_prefix("data:image/png;base64,") else {
        return Err(MediaServiceError::Invalid {
            field: "image_data_url",
            message: "must be a PNG data URL",
        });
    };
    if encoded.len() > 16 * 1024 * 1024 {
        return Err(MediaServiceError::Invalid {
            field: "image_data_url",
            message: "must be under the runtime frame size limit",
        });
    }
    let bytes = general_purpose::STANDARD
        .decode(encoded.as_bytes())
        .map_err(|_| MediaServiceError::Invalid {
            field: "image_data_url",
            message: "must contain valid base64",
        })?;
    if bytes.len() < 24 || &bytes[0..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return Err(MediaServiceError::Invalid {
            field: "image_data_url",
            message: "must contain a valid PNG image",
        });
    }
    Ok(bytes)
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[0..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((width, height))
}

fn browser_surface_health(
    dropped_frames: i64,
    reconnect_count: i64,
    ingest_latency_ms: i64,
) -> Value {
    let drop_status = if dropped_frames > 180 {
        "critical"
    } else if dropped_frames > 60 {
        "warning"
    } else {
        "nominal"
    };
    let reconnect_status = if reconnect_count > 0 {
        "recovering"
    } else {
        "stable"
    };
    let drift_status = if ingest_latency_ms > 2500 {
        "drift_warning"
    } else if ingest_latency_ms > 1200 {
        "watch"
    } else {
        "locked"
    };
    json!({
        "status": if drop_status == "critical" || drift_status == "drift_warning" {
            "degrading"
        } else if drop_status == "warning" || reconnect_status == "recovering" || drift_status == "watch" {
            "watch"
        } else {
            "stable"
        },
        "dropped_frames": dropped_frames,
        "drop_status": drop_status,
        "reconnect_count": reconnect_count,
        "reconnect_status": reconnect_status,
        "ingest_latency_ms": ingest_latency_ms,
        "drift_status": drift_status,
        "continuity_action": if drop_status == "critical" || drift_status == "drift_warning" {
            "hold_last_good_source_frame_and_reduce_refresh_rate"
        } else if drop_status == "warning" || reconnect_status == "recovering" || drift_status == "watch" {
            "keep_program_frame_clock_and_watch_source"
        } else {
            "none"
        }
    })
}

async fn encode_browser_surface_playout(
    frame_dir: &Path,
    output_path: &Path,
    target_frame_rate: i64,
) -> Result<Value, MediaServiceError> {
    let partial_path = output_path.with_extension("partial.mp4");
    let _ = fs::remove_file(output_path).await;
    let _ = fs::remove_file(&partial_path).await;
    let input_pattern = frame_dir.join("frame-%08d.png");
    let candidates = [("h264_videotoolbox", true), ("libx264", false)];
    let mut attempted = Vec::new();
    let mut last_error = String::new();

    for (encoder, hardware_accelerated) in candidates {
        attempted.push(encoder);
        let _ = fs::remove_file(&partial_path).await;
        let output = Command::new("ffmpeg")
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-y")
            .arg("-framerate")
            .arg(target_frame_rate.to_string())
            .arg("-start_number")
            .arg("1")
            .arg("-i")
            .arg(&input_pattern)
            .arg("-an")
            .arg("-c:v")
            .arg(encoder)
            .arg("-pix_fmt")
            .arg("yuv420p")
            .arg("-g")
            .arg(target_frame_rate.to_string())
            .arg("-movflags")
            .arg("+frag_keyframe+empty_moov+default_base_moof")
            .arg(&partial_path)
            .output()
            .await
            .map_err(|error| {
                MediaServiceError::Ffmpeg(FfmpegError::Render(format!(
                    "could not spawn browser surface ffmpeg playout: {error}"
                )))
            })?;
        if output.status.success() {
            fs::rename(&partial_path, output_path).await?;
            return Ok(json!({
                "selected": encoder,
                "attempted": attempted,
                "hardware_accelerated": hardware_accelerated,
                "latency_profile": "ultra_low",
                "container": "fragmented_mp4"
            }));
        }
        last_error = format!(
            "status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Err(MediaServiceError::Ffmpeg(FfmpegError::Render(format!(
        "browser surface playout encode failed after {:?}: {last_error}",
        attempted
    ))))
}

async fn validate_browser_surface_playout(
    output_path: &Path,
    expected_frames: i64,
    target_frame_rate: i64,
) -> Result<Value, MediaServiceError> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-count_frames")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_entries")
        .arg("stream=codec_name,width,height,nb_read_frames,avg_frame_rate,duration")
        .arg("-of")
        .arg("json")
        .arg(output_path)
        .output()
        .await
        .map_err(|error| {
            MediaServiceError::Ffmpeg(FfmpegError::Probe(format!(
                "could not spawn browser surface ffprobe: {error}"
            )))
        })?;
    if !output.status.success() {
        return Err(MediaServiceError::Ffmpeg(FfmpegError::Probe(format!(
            "browser surface ffprobe exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))));
    }
    let report: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| MediaServiceError::Ffmpeg(FfmpegError::Json(error)))?;
    let stream = report
        .get("streams")
        .and_then(Value::as_array)
        .and_then(|streams| streams.first())
        .cloned()
        .unwrap_or_else(|| json!({}));
    let width = stream
        .get("width")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let height = stream
        .get("height")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let frames = stream
        .get("nb_read_frames")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or_default();
    if width <= 0 || height <= 0 || frames < expected_frames {
        return Err(MediaServiceError::Ffmpeg(FfmpegError::Probe(format!(
            "browser surface playout validation failed: {width}x{height}, {frames}/{expected_frames} frames"
        ))));
    }
    Ok(json!({
        "playable": true,
        "codec": stream.get("codec_name").cloned().unwrap_or_else(|| json!("h264")),
        "width": width,
        "height": height,
        "frames": frames,
        "expected_frames": expected_frames,
        "target_frame_rate": target_frame_rate,
        "avg_frame_rate": stream.get("avg_frame_rate").cloned().unwrap_or_else(|| json!("")),
        "duration": stream.get("duration").cloned().unwrap_or_else(|| json!(""))
    }))
}

async fn media_dir() -> Result<PathBuf, MediaServiceError> {
    let base = std::env::var("VANTA_OBS_MEDIA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("vanta-obs-media"));
    fs::create_dir_all(&base).await?;
    Ok(base)
}

fn safe_path_segment(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn validate_capture(input: &CaptureStartInput) -> Result<(), MediaServiceError> {
    require_text(&input.source_id, "source_id")?;
    require_one_of(
        &input.capture_kind,
        "capture_kind",
        &[
            "camera",
            "microphone",
            "display",
            "window",
            "application_audio",
            "desktop_audio",
            "system_audio",
            "program_canvas",
            "browser_surface",
        ],
    )?;
    require_range(input.width, "width", 320, 7680)?;
    require_range(input.height, "height", 180, 4320)?;
    require_range(input.frame_rate, "frame_rate", 1, 120)?;
    require_range(
        input.duration_seconds.unwrap_or(2),
        "duration_seconds",
        2,
        21600,
    )?;
    Ok(())
}

fn validate_encode(input: &EncodeStartInput) -> Result<(), MediaServiceError> {
    require_text(&input.broadcast_id, "broadcast_id")?;
    require_text(&input.capture_session_id, "capture_session_id")?;
    require_one_of(&input.codec, "codec", &["h264", "h265", "av1"])?;
    require_one_of(&input.audio_codec, "audio_codec", &["aac", "opus"])?;
    require_one_of(&input.container, "container", &["fragmented_mp4", "mkv"])?;
    require_one_of(
        &input.latency_profile,
        "latency_profile",
        &["ultra_low", "low", "normal"],
    )?;
    require_range(input.bitrate_kbps, "bitrate_kbps", 500, 51000)?;
    require_range(
        input.keyframe_interval_seconds,
        "keyframe_interval_seconds",
        1,
        10,
    )?;
    Ok(())
}

fn require_text(value: &str, field: &'static str) -> Result<(), MediaServiceError> {
    if value.trim().is_empty() {
        return Err(MediaServiceError::Invalid {
            field,
            message: "must not be empty",
        });
    }
    Ok(())
}

fn require_one_of(
    value: &str,
    field: &'static str,
    accepted: &'static [&'static str],
) -> Result<(), MediaServiceError> {
    if !accepted.contains(&value) {
        return Err(MediaServiceError::Invalid {
            field,
            message: "is not supported by Vanta media",
        });
    }
    Ok(())
}

fn require_range(
    value: i64,
    field: &'static str,
    min: i64,
    max: i64,
) -> Result<(), MediaServiceError> {
    if value < min || value > max {
        return Err(MediaServiceError::Invalid {
            field,
            message: "is outside the supported production range",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_validation_enforces_source_kind_and_capture_bounds() {
        assert!(
            validate_capture(&CaptureStartInput {
                source_id: "source_camera".to_string(),
                capture_kind: "browser_surface".to_string(),
                width: 1280,
                height: 720,
                frame_rate: 60,
                audio: Some(false),
                duration_seconds: Some(21600),
            })
            .is_ok()
        );

        assert_invalid_field(
            validate_capture(&CaptureStartInput {
                source_id: "source_camera".to_string(),
                capture_kind: "decklink".to_string(),
                width: 1920,
                height: 1080,
                frame_rate: 60,
                audio: Some(true),
                duration_seconds: Some(2),
            }),
            "capture_kind",
        );
        assert_invalid_field(
            validate_capture(&CaptureStartInput {
                source_id: "source_camera".to_string(),
                capture_kind: "display".to_string(),
                width: 319,
                height: 1080,
                frame_rate: 60,
                audio: Some(true),
                duration_seconds: Some(2),
            }),
            "width",
        );
        assert_invalid_field(
            validate_capture(&CaptureStartInput {
                source_id: "source_camera".to_string(),
                capture_kind: "display".to_string(),
                width: 1920,
                height: 1080,
                frame_rate: 60,
                audio: Some(true),
                duration_seconds: Some(21601),
            }),
            "duration_seconds",
        );
    }

    #[test]
    fn encode_validation_enforces_profiles_and_bitrate_ranges() {
        assert!(
            validate_encode(&EncodeStartInput {
                broadcast_id: "broadcast_prime".to_string(),
                capture_session_id: "capture_session".to_string(),
                codec: "h264".to_string(),
                audio_codec: "aac".to_string(),
                container: "fragmented_mp4".to_string(),
                bitrate_kbps: 6000,
                keyframe_interval_seconds: 2,
                latency_profile: "low".to_string(),
            })
            .is_ok()
        );

        assert_invalid_field(
            validate_encode(&EncodeStartInput {
                broadcast_id: "broadcast_prime".to_string(),
                capture_session_id: "capture_session".to_string(),
                codec: "prores".to_string(),
                audio_codec: "aac".to_string(),
                container: "fragmented_mp4".to_string(),
                bitrate_kbps: 6000,
                keyframe_interval_seconds: 2,
                latency_profile: "low".to_string(),
            }),
            "codec",
        );
        assert_invalid_field(
            validate_encode(&EncodeStartInput {
                broadcast_id: "broadcast_prime".to_string(),
                capture_session_id: "capture_session".to_string(),
                codec: "h264".to_string(),
                audio_codec: "aac".to_string(),
                container: "fragmented_mp4".to_string(),
                bitrate_kbps: 499,
                keyframe_interval_seconds: 2,
                latency_profile: "low".to_string(),
            }),
            "bitrate_kbps",
        );
        assert_invalid_field(
            validate_encode(&EncodeStartInput {
                broadcast_id: "broadcast_prime".to_string(),
                capture_session_id: "capture_session".to_string(),
                codec: "h264".to_string(),
                audio_codec: "aac".to_string(),
                container: "fragmented_mp4".to_string(),
                bitrate_kbps: 6000,
                keyframe_interval_seconds: 11,
                latency_profile: "low".to_string(),
            }),
            "keyframe_interval_seconds",
        );
    }

    fn assert_invalid_field(result: Result<(), MediaServiceError>, expected: &'static str) {
        match result {
            Err(MediaServiceError::Invalid { field, .. }) => assert_eq!(field, expected),
            other => panic!("expected invalid field {expected}, got {other:?}"),
        }
    }
}

fn text(value: &Value, field: &'static str) -> Result<String, MediaServiceError> {
    let text = value_text(value, field);
    if text.is_empty() {
        return Err(MediaServiceError::Invalid {
            field,
            message: "was missing from helper response",
        });
    }
    Ok(text)
}

fn value_text(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn value_int(value: &Value, field: &str) -> i64 {
    value.get(field).and_then(Value::as_i64).unwrap_or_default()
}

fn capture_duration_seconds(capture: &Value) -> i64 {
    capture
        .pointer("/settings_json/duration_seconds")
        .and_then(Value::as_i64)
        .unwrap_or(2)
}
