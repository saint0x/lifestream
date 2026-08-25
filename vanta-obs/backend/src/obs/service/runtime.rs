use serde_json::Value;

use crate::obs::domain::{
    EmergencyDisconnectInput, LiveOpsOverrideInput, RuntimeErrorInput, RuntimeTelemetryInput,
};

use super::{ObsService, ObsServiceError, ObsServiceResult, require_text};

const RUNTIME_ERROR_SEVERITIES: &[&str] = &["warning", "error", "critical"];
const LIVE_OPS_ACTIONS: &[&str] = &["force_end", "safe_mode", "clear_incidents"];

impl ObsService {
    pub async fn runtime(&self, broadcast_id: &str) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        Ok(self.store.runtime(broadcast_id).await?)
    }

    pub async fn health(&self, broadcast_id: &str) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        Ok(self.store.health(broadcast_id).await?)
    }

    pub async fn post_show(&self, broadcast_id: &str) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        Ok(self.store.post_show(broadcast_id).await?)
    }

    pub async fn send_to_editor(&self, broadcast_id: &str) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        Ok(self.store.send_to_editor(broadcast_id).await?)
    }

    pub async fn emergency_disconnect(
        &self,
        broadcast_id: &str,
        input: EmergencyDisconnectInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        Ok(self.store.emergency_disconnect(broadcast_id, input).await?)
    }

    pub async fn ingest_runtime_error(
        &self,
        broadcast_id: &str,
        input: RuntimeErrorInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        require_text(&input.message, "message")?;
        if let Some(severity) = input.severity.as_deref()
            && !RUNTIME_ERROR_SEVERITIES.contains(&severity)
        {
            return Err(ObsServiceError::Invalid {
                field: "severity",
                message: "is not supported by Vanta OBS",
            });
        }
        Ok(self.store.ingest_runtime_error(broadcast_id, input).await?)
    }

    pub async fn ingest_runtime_telemetry(
        &self,
        broadcast_id: &str,
        input: RuntimeTelemetryInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        if input.bitrate_kbps < 0 {
            return Err(ObsServiceError::Invalid {
                field: "bitrate_kbps",
                message: "must be non-negative",
            });
        }
        if !input.upload_mbps.is_finite() || input.upload_mbps < 0.0 {
            return Err(ObsServiceError::Invalid {
                field: "upload_mbps",
                message: "must be non-negative",
            });
        }
        if input.ingest_latency_ms < 0 {
            return Err(ObsServiceError::Invalid {
                field: "ingest_latency_ms",
                message: "must be non-negative",
            });
        }
        if input.dropped_frames < 0 {
            return Err(ObsServiceError::Invalid {
                field: "dropped_frames",
                message: "must be non-negative",
            });
        }
        if !(0..=100).contains(&input.cpu_percent) {
            return Err(ObsServiceError::Invalid {
                field: "cpu_percent",
                message: "must be between 0 and 100",
            });
        }
        if input.reconnect_count.unwrap_or_default() < 0 {
            return Err(ObsServiceError::Invalid {
                field: "reconnect_count",
                message: "must be non-negative",
            });
        }
        Ok(self
            .store
            .ingest_runtime_telemetry(broadcast_id, input)
            .await?)
    }

    pub async fn live_ops_override(
        &self,
        broadcast_id: &str,
        input: LiveOpsOverrideInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        require_text(&input.action, "action")?;
        require_text(&input.reason, "reason")?;
        if !LIVE_OPS_ACTIONS.contains(&input.action.as_str()) {
            return Err(ObsServiceError::Invalid {
                field: "action",
                message: "is not supported by Vanta OBS",
            });
        }
        if let Some(target_scene_id) = input.target_scene_id.as_deref() {
            require_text(target_scene_id, "target_scene_id")?;
        }
        Ok(self.store.live_ops_override(broadcast_id, input).await?)
    }

    pub async fn create_support_bundle(&self, broadcast_id: &str) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        Ok(self.store.create_support_bundle(broadcast_id).await?)
    }
}
