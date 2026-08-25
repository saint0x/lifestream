use chrono::Utc;
use serde_json::{Value, json};
use sqlx::{Column, Row};
use uuid::Uuid;

use super::ObsStoreError;

pub(super) fn object_row(row: &sqlx::sqlite::SqliteRow) -> Result<Value, ObsStoreError> {
    let mut object = serde_json::Map::new();
    for column in row.columns() {
        let name = column.name();
        if JSON_COLUMNS.contains(&name) {
            let raw: String = row.try_get(name)?;
            object.insert(
                name.to_string(),
                serde_json::from_str(&raw).unwrap_or_else(|_| json!({})),
            );
        } else if let Ok(value) = row.try_get::<String, _>(name) {
            object.insert(name.to_string(), json!(value));
        } else if let Ok(value) = row.try_get::<f64, _>(name) {
            object.insert(name.to_string(), json!(value));
        } else if let Ok(value) = row.try_get::<i64, _>(name) {
            object.insert(name.to_string(), json!(value));
        }
    }
    Ok(Value::Object(object))
}

pub(super) fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

pub(super) fn int(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap_or_default()
}

pub(super) fn num(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or_default()
}

pub(super) fn id() -> String {
    Uuid::new_v4().to_string()
}

pub(super) fn short_id() -> String {
    Uuid::new_v4().simple().to_string()[..10].to_string()
}

pub(super) fn now() -> String {
    Utc::now().to_rfc3339()
}

const JSON_COLUMNS: [&str; 83] = [
    "tags_json",
    "collaboration_settings_json",
    "permissions_json",
    "default_settings_json",
    "crop_json",
    "transform_json",
    "layout_json",
    "settings_json",
    "obs_mapping_json",
    "filters_json",
    "route_json",
    "guard_json",
    "shared_program_context_json",
    "routing_policy_json",
    "return_feed_json",
    "connection_health_json",
    "isolated_recording_json",
    "device_check_json",
    "moderator_control_json",
    "media_state_json",
    "telemetry_json",
    "packet_json",
    "frame_json",
    "decoded_frame_json",
    "route_frame_json",
    "sync_pair_json",
    "compositor_frame_json",
    "playout_json",
    "ice_candidates_json",
    "archive_manifest_json",
    "audio_track_json",
    "video_track_json",
    "sync_json",
    "track_manifest_json",
    "participants_json",
    "modes_supported_json",
    "interruption_policy_json",
    "preview_json",
    "requirements_json",
    "checks_json",
    "result_json",
    "blockers_json",
    "warnings_json",
    "output_paths_json",
    "metrics_json",
    "sponsor_proofs_json",
    "manifest_json",
    "pressure_json",
    "buffer_json",
    "upload_queue_json",
    "reconnect_policy_json",
    "negotiation_json",
    "health_json",
    "transport_json",
    "tracks_json",
    "runtime_target_json",
    "latest_transition_json",
    "playback_readiness_json",
    "details_json",
    "bundle_json",
    "metadata_json",
    "validation_json",
    "password_json",
    "last_snapshot_json",
    "payload_json",
    "report_json",
    "original_metadata_json",
    "scene_collection_json",
    "asset_manifest_json",
    "setup_instructions_json",
    "discovery_json",
    "safety_json",
    "checks_json",
    "reminder_json",
    "options_json",
    "flight_json",
    "claims_json",
    "performance_json",
    "renderer_json",
    "review_json",
    "artifact_json",
    "binding_json",
    "last_snapshot_json",
];
