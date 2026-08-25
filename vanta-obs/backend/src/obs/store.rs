use base64::{Engine as _, engine::general_purpose};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::collections::{HashSet, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::{fs, process::Command};

use super::domain::{
    ActionConfirmationInput, AudioChannelPatch, BlockedTermInput, BroadcastInput, BroadcastPatch,
    CueInput, EmergencyDisconnectInput, GuestDeviceCheckInput, GuestInviteInput,
    GuestIsolatedRecordingInput, GuestMediaTelemetryInput, GuestModerationInput, GuestPatchInput,
    GuestReturnFeedInput, GuestRoomRoutingInput, GuestRtpPacketInput, GuestWebrtcAnswerInput,
    GuestWebrtcIceInput, GuestWebrtcOfferInput, InstanceInput, InstancePatch, LiveOpsOverrideInput,
    ModerationQueueInput, ModerationResolveInput, ModeratorInput, PinnedMessageInput,
    PreflightInput, PreflightResult, RecordingInput, ReplayInput, RuntimeErrorInput,
    RuntimeTelemetryInput, SceneGroupInput, SceneGroupPatch, SceneInput, ScenePatch,
    SceneTemplateInput, SourceInput, SourcePatch, TransitionPreviewInput, bool_int,
};
use crate::native::package::fallback_plan;
use crate::obs::{
    audio::{merged_filters, merged_route},
    recording_media::{self, ParticipantArchiveInput},
    replay_media::{ReplayClip, ReplayMediaSource},
    source::enriched_settings,
    stream_output::{StreamPublishRequest, start_local_publish},
    transition::transition_plan,
};

mod audience;
pub mod bridge;
mod engagement;
pub mod export;
mod filter;
mod hotkey;
pub mod import;
mod ops;
mod preflight;
mod query;
mod row;
mod schema;
mod seed;
mod sponsor;

use row::{id, int, now, num, short_id, text};
use schema::SCHEMA;

#[derive(Debug, Error)]
pub enum ObsStoreError {
    #[error("not found")]
    NotFound,
    #[error("safety blocked action: {0}")]
    SafetyBlocked(String),
    #[error("invalid store input: {0}")]
    Invalid(String),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

fn optional_text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

fn participant_archive_inputs(participants: &[Value]) -> Vec<ParticipantArchiveInput> {
    participants
        .iter()
        .map(|participant| ParticipantArchiveInput {
            participant_id: text(participant, "id"),
            display_name: text(participant, "display_name"),
            role: text(participant, "role"),
            source_id: optional_text(participant, "source_id"),
            status: text(participant, "status"),
        })
        .filter(|participant| !participant.participant_id.is_empty())
        .collect()
}

fn guest_device_check_status(input: &GuestDeviceCheckInput) -> String {
    let hard_block = [
        &input.camera_status,
        &input.microphone_status,
        &input.browser_status,
    ]
    .iter()
    .any(|status| matches!(status.as_str(), "denied" | "unsupported" | "missing"));
    if hard_block {
        return "blocked".to_string();
    }
    if input.network_status == "blocked"
        || input.bitrate_kbps < 800
        || input.round_trip_ms > 500
        || input.packet_loss_percent > 8.0
    {
        return "blocked".to_string();
    }
    let warning = [
        &input.camera_status,
        &input.microphone_status,
        &input.browser_status,
    ]
    .iter()
    .any(|status| status.as_str() != "ready")
        || input.network_status != "ready"
        || input.bitrate_kbps < 1200
        || input.round_trip_ms > 250
        || input.packet_loss_percent > 3.0;
    if warning {
        "warning".to_string()
    } else {
        "ready".to_string()
    }
}

fn guest_connection_health_from_device_check(status: &str, check: &Value) -> Value {
    json!({
        "status": match status {
            "ready" => "good",
            "warning" => "warning",
            _ => "blocked",
        },
        "latency_ms": check.get("round_trip_ms").cloned().unwrap_or_else(|| json!(0)),
        "packet_loss_percent": check.get("packet_loss_percent").cloned().unwrap_or_else(|| json!(0.0)),
        "recommended_layer": if status == "ready" { "720p30" } else if status == "warning" { "360p30" } else { "hold_backstage" },
        "degrade_policy": "guest_first",
        "device_check_id": check.get("id").cloned().unwrap_or_else(|| json!(""))
    })
}

fn select_replay_video_segment(segments: &Value) -> Option<&Value> {
    let segments = segments.as_array()?;
    for preferred_feed in ["program", "clean_feed"] {
        if let Some(segment) = segments.iter().find(|segment| {
            segment.get("feed").and_then(Value::as_str) == Some(preferred_feed)
                && segment
                    .get("path")
                    .or_else(|| segment.get("asset_path"))
                    .or_else(|| segment.get("source_path"))
                    .and_then(Value::as_str)
                    .is_some_and(|path| !path.trim().is_empty())
        }) {
            return Some(segment);
        }
    }
    None
}

trait GuardInput {
    fn operator_id(&self) -> Option<&str>;
    fn operator_role(&self) -> Option<&str>;
    fn confirmation_text(&self) -> Option<&str>;
    fn acknowledged_risks(&self) -> Option<&[String]>;
}

impl GuardInput for ActionConfirmationInput {
    fn operator_id(&self) -> Option<&str> {
        self.operator_id.as_deref()
    }

    fn operator_role(&self) -> Option<&str> {
        self.operator_role.as_deref()
    }

    fn confirmation_text(&self) -> Option<&str> {
        self.confirmation_text.as_deref()
    }

    fn acknowledged_risks(&self) -> Option<&[String]> {
        self.acknowledged_risks.as_deref()
    }
}

impl GuardInput for RecordingInput {
    fn operator_id(&self) -> Option<&str> {
        self.operator_id.as_deref()
    }

    fn operator_role(&self) -> Option<&str> {
        self.operator_role.as_deref()
    }

    fn confirmation_text(&self) -> Option<&str> {
        self.confirmation_text.as_deref()
    }

    fn acknowledged_risks(&self) -> Option<&[String]> {
        self.acknowledged_risks.as_deref()
    }
}

impl GuardInput for EmergencyDisconnectInput {
    fn operator_id(&self) -> Option<&str> {
        self.operator_id.as_deref()
    }

    fn operator_role(&self) -> Option<&str> {
        self.operator_role.as_deref()
    }

    fn confirmation_text(&self) -> Option<&str> {
        self.confirmation_text.as_deref()
    }

    fn acknowledged_risks(&self) -> Option<&[String]> {
        self.acknowledged_risks.as_deref()
    }
}

impl GuardInput for LiveOpsOverrideInput {
    fn operator_id(&self) -> Option<&str> {
        self.operator_id.as_deref()
    }

    fn operator_role(&self) -> Option<&str> {
        self.operator_role.as_deref()
    }

    fn confirmation_text(&self) -> Option<&str> {
        self.confirmation_text.as_deref()
    }

    fn acknowledged_risks(&self) -> Option<&[String]> {
        self.acknowledged_risks.as_deref()
    }
}

fn stream_health_for(input: &RuntimeTelemetryInput, reconnect_count: i64) -> Value {
    let status = if input.upload_mbps < 4.0
        || input.ingest_latency_ms > 2500
        || input.dropped_frames > 180
        || input.cpu_percent > 92
        || reconnect_count > 3
    {
        "red"
    } else if input.upload_mbps < 8.0
        || input.ingest_latency_ms > 1200
        || input.dropped_frames > 60
        || input.cpu_percent > 85
        || reconnect_count > 0
    {
        "yellow"
    } else {
        "green"
    };
    let adaptation = if status == "red" {
        json!({
            "state": "fallback",
            "target_bitrate_kbps": 2500,
            "target_resolution": "720p",
            "target_fps": 30,
            "reason": "protect_continuity"
        })
    } else if status == "yellow" {
        json!({
            "state": "constrained",
            "target_bitrate_kbps": 4500,
            "target_resolution": "900p",
            "target_fps": 30,
            "reason": "stabilize_upload"
        })
    } else {
        json!({
            "state": "stable",
            "target_bitrate_kbps": 6200,
            "target_resolution": "1080p",
            "target_fps": 30,
            "reason": "quality_headroom"
        })
    };
    json!({
        "status": status,
        "bandwidth_estimate_mbps": input.upload_mbps,
        "dynamic_bitrate": adaptation["state"],
        "adaptation": adaptation,
        "thresholds": {
            "green_upload_mbps": 8.0,
            "yellow_upload_mbps": 4.0,
            "green_latency_ms": 1200,
            "yellow_latency_ms": 2500,
            "green_dropped_frames": 60,
            "yellow_dropped_frames": 180,
            "green_cpu_percent": 85,
            "yellow_cpu_percent": 92
        },
        "reconnect": {
            "count": reconnect_count,
            "status": if reconnect_count > 0 { "recovering" } else { "armed" }
        },
        "viewer_playback_ready": status != "red",
        "details": input.details_json.clone().unwrap_or_else(|| json!({}))
    })
}

fn merge_output_health(existing: Option<&Value>, mut health: Value) -> Value {
    let existing_health = existing
        .and_then(|row| row.get("health_json"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    if let Some(local_publish) = existing_health.get("local_publish").cloned()
        && health.get("local_publish").is_none()
    {
        health["local_publish"] = local_publish;
    }
    if let Some(reconnect_attempts) = existing_health.get("reconnect_attempts").cloned()
        && health.get("reconnect_attempts").is_none()
    {
        health["reconnect_attempts"] = reconnect_attempts;
    }
    health
}

fn reconnect_attempt_plan(
    reconnect_count: i64,
    output_status: &str,
    ingest_status: &str,
    policy: &Value,
) -> Value {
    let max_retries = policy
        .get("max_retries")
        .and_then(Value::as_i64)
        .unwrap_or(12);
    let initial_backoff_ms = policy
        .get("initial_backoff_ms")
        .and_then(Value::as_i64)
        .unwrap_or(500);
    let max_backoff_ms = policy
        .get("max_backoff_ms")
        .and_then(Value::as_i64)
        .unwrap_or(8000);
    let exponent = reconnect_count.saturating_sub(1).min(8) as u32;
    let backoff_ms = if reconnect_count > 0 {
        (initial_backoff_ms.saturating_mul(2_i64.saturating_pow(exponent))).min(max_backoff_ms)
    } else {
        0
    };
    json!({
        "status": if reconnect_count == 0 { "recovered" } else if reconnect_count >= max_retries { "failed_over" } else { "retrying" },
        "count": reconnect_count,
        "max_retries": max_retries,
        "next_backoff_ms": backoff_ms,
        "failover_target": policy.get("failover").and_then(Value::as_str).unwrap_or("backup_ingest_url"),
        "output_status": output_status,
        "ingest_status": ingest_status,
        "continuity_priority": "preserve_program_before_quality"
    })
}

fn runtime_long_session_health(
    previous_sample_count: i64,
    previous_dropped_frames: i64,
    previous_max_reconnect_count: i64,
    previous_max_latency_ms: i64,
    previous_latency_ms: Option<i64>,
    input: &RuntimeTelemetryInput,
    reconnect_count: i64,
) -> Value {
    let sample_count = previous_sample_count + 1;
    let cumulative_dropped_frames = previous_dropped_frames + input.dropped_frames;
    let max_reconnect_count = previous_max_reconnect_count.max(reconnect_count);
    let max_latency_ms = previous_max_latency_ms.max(input.ingest_latency_ms);
    let latency_delta_ms = previous_latency_ms
        .map(|previous| input.ingest_latency_ms - previous)
        .unwrap_or(0);
    let drift_status = if latency_delta_ms.abs() > 600 || max_latency_ms > 2500 {
        "drift_warning"
    } else if latency_delta_ms.abs() > 200 || max_latency_ms > 1200 {
        "watch"
    } else {
        "locked"
    };
    let drop_status = if cumulative_dropped_frames > 360 || input.dropped_frames > 180 {
        "critical"
    } else if cumulative_dropped_frames > 120 || input.dropped_frames > 60 {
        "warning"
    } else {
        "nominal"
    };
    let reconnect_status = if reconnect_count > 0 {
        "reconnecting"
    } else if max_reconnect_count > 0 {
        "recovered"
    } else {
        "stable"
    };
    json!({
        "sample_count": sample_count,
        "cumulative_dropped_frames": cumulative_dropped_frames,
        "current_dropped_frames": input.dropped_frames,
        "drop_status": drop_status,
        "max_reconnect_count": max_reconnect_count,
        "current_reconnect_count": reconnect_count,
        "reconnect_status": reconnect_status,
        "max_ingest_latency_ms": max_latency_ms,
        "current_ingest_latency_ms": input.ingest_latency_ms,
        "latency_delta_ms": latency_delta_ms,
        "drift_status": drift_status,
        "continuity_action": if reconnect_count > 0 || drop_status == "critical" || drift_status == "drift_warning" {
            "protect_audio_hold_last_good_frame_reduce_video_layer"
        } else if drop_status == "warning" || drift_status == "watch" {
            "watch_and_prepare_layer_downshift"
        } else {
            "maintain_quality"
        },
        "protect_host_program": true,
        "protect_audio_continuity": true
    })
}

#[derive(Clone)]
pub struct ObsStore {
    pub(super) pool: SqlitePool,
}

impl ObsStore {
    pub fn next_id(&self) -> String {
        id()
    }

    pub fn pool(&self) -> SqlitePool {
        self.pool.clone()
    }

    pub async fn connect(pool: SqlitePool) -> Result<Self, ObsStoreError> {
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    async fn migrate(&self) -> Result<(), ObsStoreError> {
        for statement in SCHEMA.split(";").map(str::trim).filter(|s| !s.is_empty()) {
            sqlx::query(statement).execute(&self.pool).await?;
        }
        self.add_column_if_missing(
            "obs_replay_clip_drafts",
            "buffer_json",
            "TEXT NOT NULL DEFAULT '{}'",
        )
        .await?;
        self.add_column_if_missing(
            "obs_guest_participants",
            "device_check_json",
            "TEXT NOT NULL DEFAULT '{}'",
        )
        .await?;
        self.add_column_if_missing(
            "obs_guest_participants",
            "moderator_control_json",
            "TEXT NOT NULL DEFAULT '{}'",
        )
        .await?;
        self.add_column_if_missing(
            "obs_guest_participants",
            "media_state_json",
            "TEXT NOT NULL DEFAULT '{}'",
        )
        .await?;
        Ok(())
    }

    async fn add_column_if_missing(
        &self,
        table: &str,
        column: &str,
        definition: &str,
    ) -> Result<(), ObsStoreError> {
        let pragma = format!("PRAGMA table_info({table})");
        let rows = sqlx::query(&pragma).fetch_all(&self.pool).await?;
        let exists = rows.iter().any(|row| {
            sqlx::Row::try_get::<String, _>(row, "name")
                .map(|name| name == column)
                .unwrap_or(false)
        });
        if !exists {
            let statement = format!("ALTER TABLE {table} ADD COLUMN {column} {definition}");
            sqlx::query(&statement).execute(&self.pool).await?;
        }
        Ok(())
    }

    pub async fn dashboard(&self) -> Result<Value, ObsStoreError> {
        let collection = self.active_collection().await?;
        let broadcast = self.active_broadcast().await?;
        let broadcast_id = text(&broadcast, "id");
        let collection_id = text(&collection, "id");
        Ok(json!({
            "broadcast": broadcast,
            "collection": collection,
            "scenes": self.scenes(&collection_id).await?,
            "scene_templates": self.scene_templates().await?,
            "sources": self.sources().await?,
            "instances": self.instances(&collection_id).await?,
            "audio": self.audio_channels(&broadcast_id).await?,
            "guests": self.guest_room(&broadcast_id).await?,
            "hotkeys": self.hotkeys().await?,
            "cues": self.cues(&broadcast_id).await?,
            "runtime": self.runtime(&broadcast_id).await?,
            "health": self.health(&broadcast_id).await?,
            "preflight": self.latest_preflight(&broadcast_id).await?,
            "replays": self.replays(&broadcast_id).await?,
            "events": self.events(&broadcast_id).await?,
            "safety": self.safety_state(&broadcast_id).await?,
            "moderation": self.moderation_state(&broadcast_id).await?,
            "audience": self.audience_state(&broadcast_id).await?,
            "engagement": self.engagement_state(&broadcast_id).await?,
            "sponsor": self.sponsor_state(&broadcast_id).await?,
            "post_show": self.post_show(&broadcast_id).await?
        }))
    }

    pub async fn collections(&self) -> Result<Vec<Value>, ObsStoreError> {
        self.list(
            "SELECT * FROM obs_scene_collections ORDER BY updated_at DESC",
            &[],
        )
        .await
    }

    pub async fn collection_bundle(&self, collection_id: &str) -> Result<Value, ObsStoreError> {
        Ok(json!({
            "collection": self.row("SELECT * FROM obs_scene_collections WHERE id = ?", &[collection_id]).await?,
            "scenes": self.scenes(collection_id).await?,
            "sources": self.sources().await?,
            "instances": self.instances(collection_id).await?
        }))
    }

    pub async fn create_scene(&self, input: SceneInput) -> Result<Value, ObsStoreError> {
        let scene_id = id();
        let now = now();
        let order_index: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(order_index), 0) + 1 FROM obs_scenes WHERE collection_id = ?",
        )
        .bind(&input.collection_id)
        .fetch_one(&self.pool)
        .await?;
        sqlx::query(
            "INSERT INTO obs_scenes
            (id, collection_id, creator_id, name, order_index, transition_kind, transition_duration_ms, hotkey, locked, validation_state, created_at, updated_at)
            VALUES (?, ?, 'creator_vanta_originals', ?, ?, ?, ?, NULL, 0, 'needs_sources', ?, ?)",
        )
        .bind(&scene_id)
        .bind(input.collection_id)
        .bind(input.name)
        .bind(order_index)
        .bind(input.transition_kind.unwrap_or_else(|| "fade".to_string()))
        .bind(input.transition_duration_ms.unwrap_or(300))
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.row("SELECT * FROM obs_scenes WHERE id = ?", &[&scene_id])
            .await
    }

    pub async fn patch_scene(
        &self,
        scene_id: &str,
        input: ScenePatch,
    ) -> Result<Value, ObsStoreError> {
        let current = self
            .row("SELECT * FROM obs_scenes WHERE id = ?", &[scene_id])
            .await?;
        sqlx::query("UPDATE obs_scenes SET name = ?, locked = ?, validation_state = ?, transition_kind = ?, transition_duration_ms = ?, updated_at = ? WHERE id = ?")
            .bind(input.name.unwrap_or_else(|| text(&current, "name")))
            .bind(bool_int(input.locked.unwrap_or_else(|| int(&current, "locked") != 0)))
            .bind(input.validation_state.unwrap_or_else(|| text(&current, "validation_state")))
            .bind(input.transition_kind.unwrap_or_else(|| text(&current, "transition_kind")))
            .bind(input.transition_duration_ms.unwrap_or_else(|| int(&current, "transition_duration_ms")))
            .bind(now())
            .bind(scene_id)
            .execute(&self.pool)
            .await?;
        self.row("SELECT * FROM obs_scenes WHERE id = ?", &[scene_id])
            .await
    }

    pub async fn delete_scene(&self, scene_id: &str) -> Result<Value, ObsStoreError> {
        let scene = self
            .row("SELECT * FROM obs_scenes WHERE id = ?", &[scene_id])
            .await?;
        if int(&scene, "locked") != 0 {
            return Err(ObsStoreError::Invalid(
                "locked scenes cannot be deleted".to_string(),
            ));
        }
        let collection_id = text(&scene, "collection_id");
        let active_scene_id: Option<String> =
            sqlx::query_scalar("SELECT active_scene_id FROM obs_scene_collections WHERE id = ?")
                .bind(&collection_id)
                .fetch_optional(&self.pool)
                .await?;
        if active_scene_id.as_deref() == Some(scene_id) {
            return Err(ObsStoreError::Invalid(
                "active collection scene cannot be deleted".to_string(),
            ));
        }
        let runtime_broadcast_id: Option<String> = sqlx::query_scalar(
            "SELECT broadcast_id FROM obs_runtime_bindings
             WHERE scene_collection_id = ?
             AND (active_scene_id = ? OR program_scene_id = ? OR preview_scene_id = ?)
             LIMIT 1",
        )
        .bind(&collection_id)
        .bind(scene_id)
        .bind(scene_id)
        .bind(scene_id)
        .fetch_optional(&self.pool)
        .await?;
        if runtime_broadcast_id.is_some() {
            return Err(ObsStoreError::Invalid(
                "runtime active, preview, and program scenes cannot be deleted".to_string(),
            ));
        }
        let scene_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM obs_scenes WHERE collection_id = ?")
                .bind(&collection_id)
                .fetch_one(&self.pool)
                .await?;
        if scene_count <= 1 {
            return Err(ObsStoreError::Invalid(
                "a scene collection must keep at least one scene".to_string(),
            ));
        }
        let broadcast_id: Option<String> = sqlx::query_scalar(
            "SELECT broadcast_id FROM obs_runtime_bindings WHERE scene_collection_id = ? LIMIT 1",
        )
        .bind(&collection_id)
        .fetch_optional(&self.pool)
        .await?;
        sqlx::query("DELETE FROM obs_source_instances WHERE scene_id = ?")
            .bind(scene_id)
            .execute(&self.pool)
            .await?;
        sqlx::query(
            "UPDATE obs_guest_participants SET scene_id = NULL, updated_at = ? WHERE scene_id = ?",
        )
        .bind(now())
        .bind(scene_id)
        .execute(&self.pool)
        .await?;
        sqlx::query("UPDATE obs_live_cues SET scene_id = NULL, updated_at = ? WHERE scene_id = ?")
            .bind(now())
            .bind(scene_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM obs_hotkeys WHERE target_id = ?")
            .bind(scene_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM obs_scenes WHERE id = ?")
            .bind(scene_id)
            .execute(&self.pool)
            .await?;
        self.normalize_scene_order(&collection_id).await?;
        sqlx::query("UPDATE obs_scene_collections SET updated_at = ? WHERE id = ?")
            .bind(now())
            .bind(&collection_id)
            .execute(&self.pool)
            .await?;
        self.add_event(
            broadcast_id.as_deref(),
            "scene_delete",
            &format!("{} deleted from scene collection", text(&scene, "name")),
        )
        .await?;
        self.dashboard().await
    }

    pub async fn reorder_scenes(
        &self,
        collection_id: &str,
        scene_ids: Vec<String>,
    ) -> Result<Value, ObsStoreError> {
        self.row(
            "SELECT * FROM obs_scene_collections WHERE id = ?",
            &[collection_id],
        )
        .await?;
        let existing = self.scenes(collection_id).await?;
        let existing_ids: Vec<String> = existing.iter().map(|scene| text(scene, "id")).collect();
        if existing_ids.len() != scene_ids.len() {
            return Err(ObsStoreError::Invalid(
                "reorder must include every scene exactly once".to_string(),
            ));
        }
        let requested: HashSet<&str> = scene_ids.iter().map(String::as_str).collect();
        if requested.len() != scene_ids.len() {
            return Err(ObsStoreError::Invalid(
                "reorder scene ids must be unique".to_string(),
            ));
        }
        let known: HashSet<&str> = existing_ids.iter().map(String::as_str).collect();
        if requested != known {
            return Err(ObsStoreError::Invalid(
                "reorder scene ids must match the collection".to_string(),
            ));
        }
        let now = now();
        for (index, scene_id) in scene_ids.iter().enumerate() {
            sqlx::query("UPDATE obs_scenes SET order_index = ?, updated_at = ? WHERE id = ?")
                .bind((index as i64) + 1)
                .bind(&now)
                .bind(scene_id)
                .execute(&self.pool)
                .await?;
        }
        sqlx::query("UPDATE obs_scene_collections SET updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(collection_id)
            .execute(&self.pool)
            .await?;
        let broadcast_id: Option<String> = sqlx::query_scalar(
            "SELECT broadcast_id FROM obs_runtime_bindings WHERE scene_collection_id = ? LIMIT 1",
        )
        .bind(collection_id)
        .fetch_optional(&self.pool)
        .await?;
        self.add_event(
            broadcast_id.as_deref(),
            "scene_reorder",
            "Scene rail order updated",
        )
        .await?;
        self.dashboard().await
    }

    pub async fn create_scene_from_template(
        &self,
        template_id: &str,
        input: SceneTemplateInput,
    ) -> Result<Value, ObsStoreError> {
        let template = self
            .row(
                "SELECT * FROM obs_scene_templates WHERE id = ?",
                &[template_id],
            )
            .await?;
        self.row(
            "SELECT * FROM obs_scene_collections WHERE id = ?",
            &[&input.collection_id],
        )
        .await?;
        let layout = template["layout_json"].as_array().ok_or_else(|| {
            ObsStoreError::Invalid("template layout must be an array".to_string())
        })?;
        let required_sources = template["requirements_json"]["source_kinds"]
            .as_array()
            .ok_or_else(|| {
                ObsStoreError::Invalid("template source requirements must be an array".to_string())
            })?;
        for source_kind in required_sources {
            let Some(source_kind) = source_kind.as_str() else {
                return Err(ObsStoreError::Invalid(
                    "template source requirements must be strings".to_string(),
                ));
            };
            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM obs_sources WHERE source_kind = ? AND health_state != 'blocked'",
            )
            .bind(source_kind)
            .fetch_one(&self.pool)
            .await?;
            if count <= 0 {
                return Err(ObsStoreError::Invalid(format!(
                    "template requires available {source_kind} source"
                )));
            }
        }
        let scene = self
            .create_scene(SceneInput {
                collection_id: input.collection_id.clone(),
                name: input.name.unwrap_or_else(|| text(&template, "label")),
                transition_kind: Some(text(&template, "transition_kind")),
                transition_duration_ms: Some(int(&template, "transition_duration_ms")),
            })
            .await?;
        let scene_id = text(&scene, "id");
        for (index, item) in layout.iter().enumerate() {
            let source_kind = item
                .get("source_kind")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    ObsStoreError::Invalid("template item missing source_kind".to_string())
                })?;
            let source_id: String = sqlx::query_scalar(
                "SELECT id FROM obs_sources WHERE source_kind = ? AND health_state != 'blocked' ORDER BY updated_at DESC, display_name ASC LIMIT 1",
            )
            .bind(source_kind)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| {
                ObsStoreError::Invalid(format!("template requires available {source_kind} source"))
            })?;
            self.create_instance_raw(
                &scene_id,
                &source_id,
                item.get("order_index")
                    .and_then(Value::as_i64)
                    .unwrap_or((index as i64) + 1),
                item.get("x").and_then(Value::as_f64).unwrap_or(0.0),
                item.get("y").and_then(Value::as_f64).unwrap_or(0.0),
                item.get("width").and_then(Value::as_f64).unwrap_or(1920.0),
                item.get("height").and_then(Value::as_f64).unwrap_or(1080.0),
                item.get("opacity").and_then(Value::as_f64).unwrap_or(1.0),
            )
            .await?;
        }
        sqlx::query(
            "UPDATE obs_scenes SET validation_state = 'ready', updated_at = ? WHERE id = ?",
        )
        .bind(now())
        .bind(&scene_id)
        .execute(&self.pool)
        .await?;
        let broadcast_id: Option<String> = sqlx::query_scalar(
            "SELECT broadcast_id FROM obs_runtime_bindings WHERE scene_collection_id = ? LIMIT 1",
        )
        .bind(&input.collection_id)
        .fetch_optional(&self.pool)
        .await?;
        self.add_event(
            broadcast_id.as_deref(),
            "scene_template",
            &format!("{} template created", text(&template, "label")),
        )
        .await?;
        self.dashboard().await
    }

    async fn normalize_scene_order(&self, collection_id: &str) -> Result<(), ObsStoreError> {
        let scenes = self.scenes(collection_id).await?;
        let now = now();
        for (index, scene) in scenes.iter().enumerate() {
            sqlx::query("UPDATE obs_scenes SET order_index = ?, updated_at = ? WHERE id = ?")
                .bind((index as i64) + 1)
                .bind(&now)
                .bind(text(scene, "id"))
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    pub async fn duplicate_scene(&self, scene_id: &str) -> Result<Value, ObsStoreError> {
        let scene = self
            .row("SELECT * FROM obs_scenes WHERE id = ?", &[scene_id])
            .await?;
        let copy = self
            .create_scene(SceneInput {
                collection_id: text(&scene, "collection_id"),
                name: format!("{} Copy", text(&scene, "name")),
                transition_kind: Some(text(&scene, "transition_kind")),
                transition_duration_ms: Some(int(&scene, "transition_duration_ms")),
            })
            .await?;
        let copy_id = text(&copy, "id");
        for item in self.scene_instances(scene_id).await? {
            self.create_instance(
                &copy_id,
                InstanceInput {
                    source_id: text(&item, "source_id"),
                    order_index: int(&item, "order_index"),
                    x: num(&item, "x"),
                    y: num(&item, "y"),
                    width: num(&item, "width"),
                    height: num(&item, "height"),
                },
            )
            .await?;
        }
        self.collection_bundle(&text(&scene, "collection_id")).await
    }

    pub async fn send_to_program(&self, scene_id: &str) -> Result<Value, ObsStoreError> {
        let scene = self
            .row("SELECT * FROM obs_scenes WHERE id = ?", &[scene_id])
            .await?;
        let collection_id = text(&scene, "collection_id");
        let runtime = self
            .row(
                "SELECT * FROM obs_runtime_bindings WHERE scene_collection_id = ?",
                &[&collection_id],
            )
            .await?;
        let broadcast_id = text(&runtime, "broadcast_id");
        let from_scene_id = text(&runtime, "program_scene_id");
        let transition_kind = text(&scene, "transition_kind");
        let duration_ms = int(&scene, "transition_duration_ms");
        let now = now();
        sqlx::query("UPDATE obs_scene_transition_runs SET status = 'interrupted' WHERE collection_id = ? AND status = 'running'")
            .bind(&collection_id)
            .execute(&self.pool)
            .await?;
        sqlx::query(
            "INSERT INTO obs_scene_transition_runs
            (id, creator_id, broadcast_id, collection_id, from_scene_id, to_scene_id, transition_kind, duration_ms, status, interruption_policy_json, preview_json, started_at, completed_at, created_at)
            VALUES (?, 'creator_vanta_originals', ?, ?, ?, ?, ?, ?, 'completed', ?, ?, ?, ?, ?)",
        )
        .bind(format!("transition_{}", short_id()))
        .bind(&broadcast_id)
        .bind(&collection_id)
        .bind(if from_scene_id.is_empty() { None } else { Some(from_scene_id.as_str()) })
        .bind(scene_id)
        .bind(&transition_kind)
        .bind(duration_ms)
        .bind(json!({"mode":"replace_running","previous_running_status":"interrupted","operator_action":"send_to_program"}).to_string())
        .bind(transition_plan(
            &transition_kind,
            duration_ms,
            if from_scene_id.is_empty() {
                None
            } else {
                Some(from_scene_id.as_str())
            },
            scene_id,
            false,
        )
        .to_string())
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "UPDATE obs_scene_collections SET active_scene_id = ?, updated_at = ? WHERE id = ?",
        )
        .bind(scene_id)
        .bind(&now)
        .bind(&collection_id)
        .execute(&self.pool)
        .await?;
        sqlx::query("UPDATE obs_runtime_bindings SET program_scene_id = ?, active_scene_id = ?, runtime_state = 'program_updated', last_heartbeat_at = ?, updated_at = ? WHERE scene_collection_id = ?")
            .bind(scene_id)
            .bind(scene_id)
            .bind(&now)
            .bind(&now)
            .bind(&collection_id)
            .execute(&self.pool)
            .await?;
        self.add_event(
            Some(&broadcast_id),
            "scene_program",
            &format!(
                "{} sent to program with {} over {}ms",
                text(&scene, "name"),
                transition_kind,
                duration_ms
            ),
        )
        .await?;
        self.dashboard().await
    }

    pub async fn transition_preview(
        &self,
        scene_id: &str,
        input: TransitionPreviewInput,
    ) -> Result<Value, ObsStoreError> {
        let scene = self
            .row("SELECT * FROM obs_scenes WHERE id = ?", &[scene_id])
            .await?;
        let collection_id = text(&scene, "collection_id");
        let runtime = self
            .row(
                "SELECT * FROM obs_runtime_bindings WHERE scene_collection_id = ?",
                &[&collection_id],
            )
            .await?;
        let from_scene_id = input
            .from_scene_id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| text(&runtime, "program_scene_id"));
        if !from_scene_id.is_empty() {
            let from_scene = self
                .row("SELECT * FROM obs_scenes WHERE id = ?", &[&from_scene_id])
                .await?;
            let from_collection_id = text(&from_scene, "collection_id");
            if from_collection_id != collection_id {
                return Err(ObsStoreError::Invalid(
                    "transition preview scenes must be in the same collection".to_string(),
                ));
            }
        }
        let transition_kind = text(&scene, "transition_kind");
        let duration_ms = int(&scene, "transition_duration_ms");
        let from_scene_id = if from_scene_id.is_empty() {
            None
        } else {
            Some(from_scene_id)
        };
        let plan = transition_plan(
            &transition_kind,
            duration_ms,
            from_scene_id.as_deref(),
            scene_id,
            true,
        );
        Ok(json!({
            "id": format!("transition_preview_{}", scene_id),
            "collection_id": collection_id,
            "from_scene_id": from_scene_id,
            "to_scene_id": scene_id,
            "transition": plan
        }))
    }

    pub async fn create_source(&self, input: SourceInput) -> Result<Value, ObsStoreError> {
        let source_id = id();
        let now = now();
        let settings = enriched_settings(
            &input.source_kind,
            input.device_id.as_deref(),
            input.browser_url.as_deref(),
            input.media_asset_id.as_deref(),
            "pending",
            "unknown",
            input.settings_json.unwrap_or_else(|| json!({})),
        );
        sqlx::query(
            "INSERT INTO obs_sources
            (id, creator_id, source_kind, display_name, device_id, media_asset_id, browser_url, default_settings_json, permission_state, health_state, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, ?, ?, ?, ?, ?, 'pending', 'unknown', ?, ?)",
        )
        .bind(&source_id)
        .bind(input.source_kind)
        .bind(input.display_name)
        .bind(input.device_id)
        .bind(input.media_asset_id)
        .bind(input.browser_url)
        .bind(settings.to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.row("SELECT * FROM obs_sources WHERE id = ?", &[&source_id])
            .await
    }

    pub async fn create_scene_group(
        &self,
        target_scene_id: &str,
        input: SceneGroupInput,
    ) -> Result<Value, ObsStoreError> {
        let target_scene = self
            .row("SELECT * FROM obs_scenes WHERE id = ?", &[target_scene_id])
            .await?;
        let child_scene = self
            .row(
                "SELECT * FROM obs_scenes WHERE id = ?",
                &[&input.child_scene_id],
            )
            .await?;
        let collection_id = text(&target_scene, "collection_id");
        if collection_id != text(&child_scene, "collection_id") {
            return Err(ObsStoreError::Invalid(
                "scene groups must reference scenes in the same collection".to_string(),
            ));
        }
        self.ensure_scene_group_can_reference(target_scene_id, &input.child_scene_id)
            .await?;
        let source_id = id();
        let now = now();
        let settings = enriched_settings(
            "scene_group",
            None,
            None,
            None,
            "granted",
            "good",
            json!({
                "scene_id": input.child_scene_id,
                "collection_id": collection_id,
                "group_kind": "nested_scene",
                "renderer": "nested_scene_graph"
            }),
        );
        sqlx::query(
            "INSERT INTO obs_sources
            (id, creator_id, source_kind, display_name, device_id, media_asset_id, browser_url, default_settings_json, permission_state, health_state, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', 'scene_group', ?, NULL, NULL, NULL, ?, 'granted', 'good', ?, ?)",
        )
        .bind(&source_id)
        .bind(input.label)
        .bind(settings.to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        let order_index: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(order_index), 0) + 1 FROM obs_source_instances WHERE scene_id = ?",
        )
        .bind(target_scene_id)
        .fetch_one(&self.pool)
        .await?;
        self.create_instance_raw(
            target_scene_id,
            &source_id,
            order_index,
            input.x.unwrap_or(120.0),
            input.y.unwrap_or(120.0),
            input.width.unwrap_or(760.0),
            input.height.unwrap_or(428.0),
            input.opacity.unwrap_or(1.0),
        )
        .await?;
        sqlx::query(
            "UPDATE obs_scenes SET validation_state = 'ready', updated_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(target_scene_id)
        .execute(&self.pool)
        .await?;
        let broadcast_id: Option<String> = sqlx::query_scalar(
            "SELECT broadcast_id FROM obs_runtime_bindings WHERE scene_collection_id = ? LIMIT 1",
        )
        .bind(&collection_id)
        .fetch_optional(&self.pool)
        .await?;
        self.add_event(
            broadcast_id.as_deref(),
            "scene_group",
            &format!(
                "{} nested into {}",
                text(&child_scene, "name"),
                text(&target_scene, "name")
            ),
        )
        .await?;
        self.dashboard().await
    }

    pub async fn patch_scene_group(
        &self,
        source_id: &str,
        input: SceneGroupPatch,
    ) -> Result<Value, ObsStoreError> {
        let source = self.source(source_id).await?;
        if text(&source, "source_kind") != "scene_group" {
            return Err(ObsStoreError::Invalid(
                "source is not a scene group".to_string(),
            ));
        }
        let target_scenes = self
            .list(
                "SELECT DISTINCT s.* FROM obs_scenes s JOIN obs_source_instances i ON i.scene_id = s.id WHERE i.source_id = ? ORDER BY s.order_index ASC",
                &[source_id],
            )
            .await?;
        let mut settings = source
            .get("default_settings_json")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let next_child_id = input
            .child_scene_id
            .unwrap_or_else(|| text(&settings, "scene_id"));
        let child_scene = self
            .row("SELECT * FROM obs_scenes WHERE id = ?", &[&next_child_id])
            .await?;
        for target_scene in &target_scenes {
            if text(target_scene, "collection_id") != text(&child_scene, "collection_id") {
                return Err(ObsStoreError::Invalid(
                    "scene groups must reference scenes in the same collection".to_string(),
                ));
            }
            self.ensure_scene_group_can_reference(&text(target_scene, "id"), &next_child_id)
                .await?;
        }
        if let Some(object) = settings.as_object_mut() {
            object.insert("scene_id".to_string(), json!(next_child_id));
            object.insert(
                "collection_id".to_string(),
                json!(text(&child_scene, "collection_id")),
            );
            object.insert("group_kind".to_string(), json!("nested_scene"));
            object.insert("renderer".to_string(), json!("nested_scene_graph"));
        }
        let display_name = input.label.unwrap_or_else(|| text(&source, "display_name"));
        let enriched =
            enriched_settings("scene_group", None, None, None, "granted", "good", settings);
        sqlx::query("UPDATE obs_sources SET display_name = ?, default_settings_json = ?, updated_at = ? WHERE id = ?")
            .bind(display_name)
            .bind(enriched.to_string())
            .bind(now())
            .bind(source_id)
            .execute(&self.pool)
            .await?;
        let broadcast_id: Option<String> = sqlx::query_scalar(
            "SELECT rb.broadcast_id FROM obs_runtime_bindings rb JOIN obs_scenes s ON s.collection_id = rb.scene_collection_id JOIN obs_source_instances i ON i.scene_id = s.id WHERE i.source_id = ? LIMIT 1",
        )
        .bind(source_id)
        .fetch_optional(&self.pool)
        .await?;
        self.add_event(
            broadcast_id.as_deref(),
            "scene_group_update",
            "Scene group reference updated",
        )
        .await?;
        self.dashboard().await
    }

    pub async fn patch_source(
        &self,
        source_id: &str,
        input: SourcePatch,
    ) -> Result<Value, ObsStoreError> {
        let current = self
            .row("SELECT * FROM obs_sources WHERE id = ?", &[source_id])
            .await?;
        let display_name = input
            .display_name
            .unwrap_or_else(|| text(&current, "display_name"));
        let permission_state = input
            .permission_state
            .unwrap_or_else(|| text(&current, "permission_state"));
        let health_state = input
            .health_state
            .unwrap_or_else(|| text(&current, "health_state"));
        let settings = enriched_settings(
            &text(&current, "source_kind"),
            optional_text(&current, "device_id").as_deref(),
            optional_text(&current, "browser_url").as_deref(),
            optional_text(&current, "media_asset_id").as_deref(),
            &permission_state,
            &health_state,
            input
                .settings_json
                .unwrap_or_else(|| current["default_settings_json"].clone()),
        );
        sqlx::query("UPDATE obs_sources SET display_name = ?, permission_state = ?, health_state = ?, default_settings_json = ?, updated_at = ? WHERE id = ?")
            .bind(display_name)
            .bind(permission_state)
            .bind(health_state)
            .bind(settings.to_string())
            .bind(now())
            .bind(source_id)
            .execute(&self.pool)
            .await?;
        self.row("SELECT * FROM obs_sources WHERE id = ?", &[source_id])
            .await
    }

    pub async fn source(&self, source_id: &str) -> Result<Value, ObsStoreError> {
        self.row("SELECT * FROM obs_sources WHERE id = ?", &[source_id])
            .await
    }

    async fn ensure_scene_group_can_reference(
        &self,
        target_scene_id: &str,
        child_scene_id: &str,
    ) -> Result<(), ObsStoreError> {
        if target_scene_id == child_scene_id {
            return Err(ObsStoreError::Invalid(
                "scene groups cannot reference their own scene".to_string(),
            ));
        }
        let mut stack = vec![child_scene_id.to_string()];
        let mut visited = HashSet::new();
        while let Some(scene_id) = stack.pop() {
            if scene_id == target_scene_id {
                return Err(ObsStoreError::Invalid(
                    "scene group reference would create a cycle".to_string(),
                ));
            }
            if !visited.insert(scene_id.clone()) {
                continue;
            }
            let children = self
                .list(
                    "SELECT src.default_settings_json FROM obs_source_instances i JOIN obs_sources src ON src.id = i.source_id WHERE i.scene_id = ? AND src.source_kind = 'scene_group'",
                    &[&scene_id],
                )
                .await?;
            for child in children {
                let nested_id = text(&child["default_settings_json"], "scene_id");
                if !nested_id.is_empty() {
                    stack.push(nested_id);
                }
            }
        }
        Ok(())
    }

    async fn create_guest_source(
        &self,
        participant_id: &str,
        display_name: &str,
    ) -> Result<String, ObsStoreError> {
        let source_id = format!("source_{participant_id}");
        let now = now();
        sqlx::query(
            "INSERT OR IGNORE INTO obs_sources
            (id, creator_id, source_kind, display_name, device_id, media_asset_id, browser_url, default_settings_json, permission_state, health_state, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', 'guest_feed', ?, ?, NULL, NULL, ?, 'granted', 'good', ?, ?)",
        )
        .bind(&source_id)
        .bind(display_name)
        .bind(format!("guest:{participant_id}"))
        .bind(json!({"return_audio":"mix_minus","source":"guest_room"}).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(source_id)
    }

    pub async fn patch_audio_channel(
        &self,
        channel_id: &str,
        input: AudioChannelPatch,
    ) -> Result<Value, ObsStoreError> {
        let current = self
            .row(
                "SELECT * FROM obs_audio_channels WHERE id = ?",
                &[channel_id],
            )
            .await?;
        let filters = merged_filters(current["filters_json"].clone(), input.filters_json);
        let route = merged_route(current["route_json"].clone(), input.route_json);
        sqlx::query("UPDATE obs_audio_channels SET muted = ?, solo = ?, gain_db = ?, monitor_enabled = ?, program_enabled = ?, delay_ms = ?, filters_json = ?, route_json = ?, updated_at = ? WHERE id = ?")
            .bind(bool_int(input.muted.unwrap_or_else(|| int(&current, "muted") != 0)))
            .bind(bool_int(input.solo.unwrap_or_else(|| int(&current, "solo") != 0)))
            .bind(input.gain_db.unwrap_or_else(|| num(&current, "gain_db")))
            .bind(bool_int(input.monitor_enabled.unwrap_or_else(|| int(&current, "monitor_enabled") != 0)))
            .bind(bool_int(input.program_enabled.unwrap_or_else(|| int(&current, "program_enabled") != 0)))
            .bind(input.delay_ms.unwrap_or_else(|| int(&current, "delay_ms")))
            .bind(filters.to_string())
            .bind(route.to_string())
            .bind(now())
            .bind(channel_id)
            .execute(&self.pool)
            .await?;
        self.row(
            "SELECT * FROM obs_audio_channels WHERE id = ?",
            &[channel_id],
        )
        .await
    }

    pub async fn invite_guest(
        &self,
        broadcast_id: &str,
        input: GuestInviteInput,
    ) -> Result<Value, ObsStoreError> {
        let room = self.guest_room(broadcast_id).await?;
        let room_id = text(&room, "id");
        let participant = format!("guest_{}", short_id());
        let now = now();
        let role = input.role.unwrap_or_else(|| "guest".to_string());
        sqlx::query(
            "INSERT INTO obs_guest_participants
            (id, room_id, broadcast_id, display_name, role, source_id, status, muted, solo, safety_disabled, invite_url, scene_id, return_feed_json, connection_health_json, isolated_recording_json, device_check_json, moderator_control_json, media_state_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, NULL, 'invited', 0, 0, 0, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&participant)
        .bind(&room_id)
        .bind(broadcast_id)
        .bind(input.display_name)
        .bind(role)
        .bind(format!("https://studio.vanta.local/guest/{broadcast_id}/{participant}"))
        .bind(json!({"video":"program_return","audio":"mix_minus","shared_game_feed":"low_latency"}).to_string())
        .bind(json!({"status":"invited","latency_ms":0,"packet_loss_percent":0,"recommended_layer":"pending"}).to_string())
        .bind(json!({"status":"pending","audio":true,"video":true,"storage":"local_then_archive"}).to_string())
        .bind(json!({"status":"pending","camera":"pending","microphone":"pending","network":"pending","browser":"pending"}).to_string())
        .bind(json!({"status":"clear","last_action":"none","moderator_id":null}).to_string())
        .bind(json!({"speaking":false,"active_speaker":false,"audio_level_db":-80.0,"video_active":true,"score":0.0}).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.add_event(
            Some(broadcast_id),
            "guest_invite",
            "Guest invite link created",
        )
        .await?;
        self.guest_room(broadcast_id).await
    }

    pub async fn promote_guest(
        &self,
        participant_id: &str,
        scene_id: &str,
    ) -> Result<Value, ObsStoreError> {
        let participant = self
            .row(
                "SELECT * FROM obs_guest_participants WHERE id = ?",
                &[participant_id],
            )
            .await?;
        let broadcast_id = text(&participant, "broadcast_id");
        let source_id = if text(&participant, "source_id").is_empty() {
            self.create_guest_source(participant_id, &text(&participant, "display_name"))
                .await?
        } else {
            text(&participant, "source_id")
        };
        sqlx::query("UPDATE obs_guest_participants SET status = 'live', source_id = ?, scene_id = ?, return_feed_json = ?, connection_health_json = ?, isolated_recording_json = ?, updated_at = ? WHERE id = ?")
            .bind(&source_id)
            .bind(scene_id)
            .bind(json!({"video":"program_return","audio":"mix_minus","shared_game_feed":"low_latency","active":true}).to_string())
            .bind(json!({"status":"good","latency_ms":96,"packet_loss_percent":0.3,"recommended_layer":"720p30","degrade_policy":"guest_first"}).to_string())
            .bind(json!({"status":"recording","audio":true,"video":true,"storage":"local_then_archive"}).to_string())
            .bind(now())
            .bind(participant_id)
            .execute(&self.pool)
            .await?;
        let existing_instance: Option<String> = sqlx::query_scalar(
            "SELECT id FROM obs_source_instances WHERE scene_id = ? AND source_id = ? LIMIT 1",
        )
        .bind(scene_id)
        .bind(&source_id)
        .fetch_optional(&self.pool)
        .await?;
        if existing_instance.is_none() {
            let order_index: i64 = sqlx::query_scalar(
                "SELECT COALESCE(MAX(order_index), 0) + 1 FROM obs_source_instances WHERE scene_id = ?",
            )
            .bind(scene_id)
            .fetch_one(&self.pool)
            .await?;
            self.create_instance_raw(
                scene_id,
                &source_id,
                order_index,
                1320.0,
                96.0,
                500.0,
                282.0,
                1.0,
            )
            .await?;
        }
        self.add_event(
            Some(&broadcast_id),
            "guest_promote",
            "Guest promoted from backstage to program scene",
        )
        .await?;
        self.guest_room(&broadcast_id).await
    }

    pub async fn patch_guest(
        &self,
        participant_id: &str,
        input: GuestPatchInput,
    ) -> Result<Value, ObsStoreError> {
        let participant = self
            .row(
                "SELECT * FROM obs_guest_participants WHERE id = ?",
                &[participant_id],
            )
            .await?;
        let broadcast_id = text(&participant, "broadcast_id");
        let status = if input.safety_disabled == Some(true) {
            "disabled".to_string()
        } else {
            text(&participant, "status")
        };
        sqlx::query("UPDATE obs_guest_participants SET muted = ?, solo = ?, safety_disabled = ?, status = ?, updated_at = ? WHERE id = ?")
            .bind(bool_int(input.muted.unwrap_or_else(|| int(&participant, "muted") != 0)))
            .bind(bool_int(input.solo.unwrap_or_else(|| int(&participant, "solo") != 0)))
            .bind(bool_int(input.safety_disabled.unwrap_or_else(|| int(&participant, "safety_disabled") != 0)))
            .bind(status)
            .bind(now())
            .bind(participant_id)
            .execute(&self.pool)
            .await?;
        self.add_event(
            Some(&broadcast_id),
            "guest_update",
            "Guest controls updated",
        )
        .await?;
        self.guest_room(&broadcast_id).await
    }

    pub async fn run_guest_device_check(
        &self,
        participant_id: &str,
        input: GuestDeviceCheckInput,
    ) -> Result<Value, ObsStoreError> {
        let participant = self
            .row(
                "SELECT * FROM obs_guest_participants WHERE id = ?",
                &[participant_id],
            )
            .await?;
        let broadcast_id = text(&participant, "broadcast_id");
        let created_at = now();
        let check_id = format!("guest_check_{}", short_id());
        let status = guest_device_check_status(&input);
        let checks = json!({
            "camera": input.camera_status,
            "microphone": input.microphone_status,
            "network": input.network_status,
            "browser": input.browser_status,
            "bitrate_kbps": input.bitrate_kbps,
            "round_trip_ms": input.round_trip_ms,
            "packet_loss_percent": input.packet_loss_percent,
            "details": input.checks_json.unwrap_or_else(|| json!({})),
            "thresholds": {
                "minimum_bitrate_kbps": 1200,
                "maximum_round_trip_ms": 250,
                "maximum_packet_loss_percent": 3.0
            }
        });
        let latest = json!({
            "id": check_id,
            "status": status,
            "camera": checks["camera"],
            "microphone": checks["microphone"],
            "network": checks["network"],
            "browser": checks["browser"],
            "bitrate_kbps": checks["bitrate_kbps"],
            "round_trip_ms": checks["round_trip_ms"],
            "packet_loss_percent": checks["packet_loss_percent"],
            "checked_at": created_at,
            "checks": checks
        });
        sqlx::query(
            "INSERT INTO obs_guest_device_checks
            (id, participant_id, broadcast_id, status, camera_status, microphone_status,
             network_status, browser_status, bitrate_kbps, round_trip_ms, packet_loss_percent,
             checks_json, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&check_id)
        .bind(participant_id)
        .bind(&broadcast_id)
        .bind(&status)
        .bind(text(&checks, "camera"))
        .bind(text(&checks, "microphone"))
        .bind(text(&checks, "network"))
        .bind(text(&checks, "browser"))
        .bind(input.bitrate_kbps)
        .bind(input.round_trip_ms)
        .bind(input.packet_loss_percent)
        .bind(checks.to_string())
        .bind(&created_at)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "UPDATE obs_guest_participants SET device_check_json = ?, connection_health_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(latest.to_string())
        .bind(guest_connection_health_from_device_check(&status, &latest).to_string())
        .bind(&created_at)
        .bind(participant_id)
        .execute(&self.pool)
        .await?;
        self.add_event(
            Some(&broadcast_id),
            "guest_device_check",
            "Guest device check completed",
        )
        .await?;
        self.guest_room(&broadcast_id).await
    }

    pub async fn moderate_guest(
        &self,
        participant_id: &str,
        input: GuestModerationInput,
    ) -> Result<Value, ObsStoreError> {
        let participant = self
            .row(
                "SELECT * FROM obs_guest_participants WHERE id = ?",
                &[participant_id],
            )
            .await?;
        let broadcast_id = text(&participant, "broadcast_id");
        let created_at = now();
        let action_id = format!("guest_mod_{}", short_id());
        let target_scene_id = input.target_scene_id.clone().unwrap_or_default();
        let result = json!({
            "id": action_id,
            "status": "applied",
            "action": input.action,
            "moderator_id": input.moderator_id,
            "reason": input.reason,
            "target_scene_id": if target_scene_id.is_empty() { Value::Null } else { json!(target_scene_id) },
            "applied_at": created_at
        });
        let action = text(&result, "action");
        let moderator_id = text(&result, "moderator_id");
        let reason = text(&result, "reason");
        let moderation_result = result.to_string();
        sqlx::query(
            "INSERT INTO obs_guest_moderation_actions
            (id, participant_id, broadcast_id, moderator_id, action, reason, target_scene_id,
             status, result_json, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, 'applied', ?, ?)",
        )
        .bind(&action_id)
        .bind(participant_id)
        .bind(&broadcast_id)
        .bind(&moderator_id)
        .bind(&action)
        .bind(&reason)
        .bind(if target_scene_id.is_empty() {
            Option::<String>::None
        } else {
            Some(target_scene_id.clone())
        })
        .bind(&moderation_result)
        .bind(&created_at)
        .execute(&self.pool)
        .await?;

        match action.as_str() {
            "hold_backstage" => {
                sqlx::query("UPDATE obs_guest_participants SET status = 'held', safety_disabled = 1, scene_id = NULL, solo = 0, moderator_control_json = ?, updated_at = ? WHERE id = ?")
                    .bind(&moderation_result)
                    .bind(&created_at)
                    .bind(participant_id)
                    .execute(&self.pool)
                    .await?;
            }
            "release_backstage" => {
                sqlx::query("UPDATE obs_guest_participants SET status = 'backstage', safety_disabled = 0, moderator_control_json = ?, updated_at = ? WHERE id = ?")
                    .bind(&moderation_result)
                    .bind(&created_at)
                    .bind(participant_id)
                    .execute(&self.pool)
                    .await?;
            }
            "approve_live" => {
                sqlx::query("UPDATE obs_guest_participants SET moderator_control_json = ?, updated_at = ? WHERE id = ?")
                    .bind(&moderation_result)
                    .bind(&created_at)
                    .bind(participant_id)
                    .execute(&self.pool)
                    .await?;
                if !target_scene_id.is_empty() {
                    self.promote_guest(participant_id, &target_scene_id).await?;
                }
            }
            _ => {
                return Err(ObsStoreError::Invalid(
                    "unsupported guest moderation action".to_string(),
                ));
            }
        }
        self.add_event(
            Some(&broadcast_id),
            "guest_moderation",
            "Guest moderation control applied",
        )
        .await?;
        self.guest_room(&broadcast_id).await
    }

    pub async fn report_guest_media_telemetry(
        &self,
        participant_id: &str,
        input: GuestMediaTelemetryInput,
    ) -> Result<Value, ObsStoreError> {
        let participant = self
            .row(
                "SELECT * FROM obs_guest_participants WHERE id = ?",
                &[participant_id],
            )
            .await?;
        let broadcast_id = text(&participant, "broadcast_id");
        let created_at = now();
        let telemetry_id = format!("guest_media_{}", short_id());
        let score = guest_active_speaker_score(&participant, &input);
        let previous_guest_stats = sqlx::query(
            "SELECT COUNT(*) AS sample_count,
                    COALESCE(SUM(dropped_frames), 0) AS dropped_frames,
                    COALESCE(MAX(round_trip_ms), 0) AS max_round_trip_ms,
                    COALESCE(MAX(jitter_ms), 0) AS max_jitter_ms,
                    COALESCE(MAX(packet_loss_percent), 0.0) AS max_packet_loss_percent
             FROM obs_guest_media_telemetry WHERE participant_id = ?",
        )
        .bind(participant_id)
        .fetch_one(&self.pool)
        .await?;
        let long_session = guest_long_session_health(
            previous_guest_stats.get::<i64, _>("sample_count"),
            previous_guest_stats.get::<i64, _>("dropped_frames"),
            previous_guest_stats.get::<i64, _>("max_round_trip_ms"),
            previous_guest_stats.get::<i64, _>("max_jitter_ms"),
            previous_guest_stats.get::<f64, _>("max_packet_loss_percent"),
            &input,
        );
        let telemetry = json!({
            "id": telemetry_id,
            "participant_id": participant_id,
            "audio_level_db": input.audio_level_db,
            "speaking": input.speaking,
            "video_active": input.video_active,
            "round_trip_ms": input.round_trip_ms,
            "packet_loss_percent": input.packet_loss_percent,
            "jitter_ms": input.jitter_ms.unwrap_or_default(),
            "dropped_frames": input.dropped_frames.unwrap_or_default(),
            "active_speaker_score": score,
            "long_session": long_session,
            "media": input.media_json.unwrap_or_else(|| json!({})),
            "sampled_at": created_at
        });
        sqlx::query(
            "INSERT INTO obs_guest_media_telemetry
            (id, participant_id, broadcast_id, audio_level_db, speaking, video_active,
             round_trip_ms, packet_loss_percent, jitter_ms, dropped_frames, active_speaker_score,
             telemetry_json, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&telemetry_id)
        .bind(participant_id)
        .bind(&broadcast_id)
        .bind(input.audio_level_db)
        .bind(bool_int(input.speaking))
        .bind(bool_int(input.video_active))
        .bind(input.round_trip_ms)
        .bind(input.packet_loss_percent)
        .bind(input.jitter_ms.unwrap_or_default())
        .bind(input.dropped_frames.unwrap_or_default())
        .bind(score)
        .bind(telemetry.to_string())
        .bind(&created_at)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "UPDATE obs_guest_participants SET media_state_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(guest_media_state(&participant, &telemetry).to_string())
        .bind(&created_at)
        .bind(participant_id)
        .execute(&self.pool)
        .await?;
        self.refresh_guest_active_speaker(&broadcast_id, &created_at)
            .await?;
        self.add_event(
            Some(&broadcast_id),
            "guest_active_speaker",
            "Guest media telemetry updated active speaker state",
        )
        .await?;
        self.guest_room(&broadcast_id).await
    }

    pub async fn create_guest_webrtc_offer(
        &self,
        participant_id: &str,
        input: GuestWebrtcOfferInput,
    ) -> Result<Value, ObsStoreError> {
        let participant = self
            .row(
                "SELECT * FROM obs_guest_participants WHERE id = ?",
                &[participant_id],
            )
            .await?;
        if text(&participant, "status") == "removed" {
            return Err(ObsStoreError::Invalid(
                "removed guests cannot negotiate media sessions".to_string(),
            ));
        }
        if !input.audio && !input.video {
            return Err(ObsStoreError::Invalid(
                "guest WebRTC sessions require audio or video".to_string(),
            ));
        }
        let session_id = format!("guest_webrtc_{}", short_id());
        let broadcast_id = text(&participant, "broadcast_id");
        let created_at = now();
        let preferred_video_layer = input
            .preferred_video_layer
            .filter(|layer| !layer.trim().is_empty())
            .unwrap_or_else(|| guest_recommended_layer(&participant).to_string());
        let tracks = input.tracks_json.unwrap_or_else(|| {
            json!({
                "audio": input.audio,
                "video": input.video,
                "participant_source_id": optional_text(&participant, "source_id"),
                "expected_audio_codec": "opus",
                "expected_video_codec": "h264"
            })
        });
        let transport = json!({
            "transport": "webrtc",
            "controller": "vanta_realtime_sfu",
            "signaling": "backend_persisted_offer_runtime_answer",
            "session_role": input.session_role,
            "direction": input.direction,
            "ice_servers": [
                { "urls": ["stun:stun.l.google.com:19302"] }
            ],
            "sfu_required": true
        });
        let health = json!({
            "status": "awaiting_sfu_answer",
            "audio_ready": input.audio,
            "video_ready": input.video,
            "ice_candidate_count": 0,
            "created_at": created_at,
            "last_error": Value::Null
        });
        sqlx::query(
            "INSERT INTO obs_guest_webrtc_sessions
            (id, participant_id, broadcast_id, session_role, direction, status, audio_enabled, video_enabled,
             preferred_video_layer, selected_video_layer, offer_sdp, answer_sdp, ice_candidates_json,
             tracks_json, transport_json, health_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, 'awaiting_sfu_answer', ?, ?, ?, '', ?, '', ?, ?, ?, ?, ?, ?)",
        )
        .bind(&session_id)
        .bind(participant_id)
        .bind(&broadcast_id)
        .bind(&input.session_role)
        .bind(&input.direction)
        .bind(bool_int(input.audio))
        .bind(bool_int(input.video))
        .bind(&preferred_video_layer)
        .bind(&input.offer_sdp)
        .bind(json!([]).to_string())
        .bind(tracks.to_string())
        .bind(transport.to_string())
        .bind(health.to_string())
        .bind(&created_at)
        .bind(&created_at)
        .execute(&self.pool)
        .await?;
        self.update_guest_webrtc_state(participant_id, &session_id, &health)
            .await?;
        self.add_event(
            Some(&broadcast_id),
            "guest_webrtc_offer",
            "Guest WebRTC offer persisted for runtime SFU answer",
        )
        .await?;
        self.guest_room(&broadcast_id).await
    }

    pub async fn apply_guest_webrtc_answer(
        &self,
        session_id: &str,
        input: GuestWebrtcAnswerInput,
    ) -> Result<Value, ObsStoreError> {
        let session = self
            .row(
                "SELECT * FROM obs_guest_webrtc_sessions WHERE id = ?",
                &[session_id],
            )
            .await?;
        let participant_id = text(&session, "participant_id");
        let selected_video_layer = input
            .selected_video_layer
            .filter(|layer| !layer.trim().is_empty())
            .unwrap_or_else(|| text(&session, "preferred_video_layer"));
        let updated_at = now();
        let mut health = session
            .get("health_json")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if !health.is_object() {
            health = json!({});
        }
        if let Some(object) = health.as_object_mut() {
            object.insert("status".to_string(), json!("connected"));
            object.insert("answered_at".to_string(), json!(updated_at));
            object.insert(
                "selected_video_layer".to_string(),
                json!(selected_video_layer),
            );
            object.insert(
                "runtime_media".to_string(),
                input.media_json.unwrap_or_else(|| json!({})),
            );
        }
        sqlx::query(
            "UPDATE obs_guest_webrtc_sessions SET status = 'connected', selected_video_layer = ?, answer_sdp = ?, health_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&selected_video_layer)
        .bind(&input.answer_sdp)
        .bind(health.to_string())
        .bind(&updated_at)
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        self.update_guest_webrtc_state(&participant_id, session_id, &health)
            .await?;
        let broadcast_id = text(&session, "broadcast_id");
        self.add_event(
            Some(&broadcast_id),
            "guest_webrtc_answer",
            "Guest WebRTC runtime answer applied",
        )
        .await?;
        self.guest_room(&broadcast_id).await
    }

    pub async fn add_guest_webrtc_ice_candidate(
        &self,
        session_id: &str,
        input: GuestWebrtcIceInput,
    ) -> Result<Value, ObsStoreError> {
        let session = self
            .row(
                "SELECT * FROM obs_guest_webrtc_sessions WHERE id = ?",
                &[session_id],
            )
            .await?;
        let updated_at = now();
        let mut candidates = session
            .get("ice_candidates_json")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        candidates.push(json!({
            "candidate": input.candidate,
            "sdp_mid": input.sdp_mid,
            "sdp_mline_index": input.sdp_mline_index,
            "payload": input.candidate_json.unwrap_or_else(|| json!({})),
            "received_at": updated_at
        }));
        let mut health = session
            .get("health_json")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if !health.is_object() {
            health = json!({});
        }
        if let Some(object) = health.as_object_mut() {
            object.insert("ice_candidate_count".to_string(), json!(candidates.len()));
            object.insert("ice_state".to_string(), json!("gathering"));
            object.insert("last_ice_at".to_string(), json!(updated_at));
        }
        sqlx::query(
            "UPDATE obs_guest_webrtc_sessions SET ice_candidates_json = ?, health_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(json!(candidates).to_string())
        .bind(health.to_string())
        .bind(&updated_at)
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        self.update_guest_webrtc_state(&text(&session, "participant_id"), session_id, &health)
            .await?;
        self.add_event(
            Some(&text(&session, "broadcast_id")),
            "guest_webrtc_ice",
            "Guest WebRTC ICE candidate persisted",
        )
        .await?;
        self.guest_room(&text(&session, "broadcast_id")).await
    }

    pub async fn reconcile_guest_media_relays(
        &self,
        broadcast_id: &str,
    ) -> Result<Value, ObsStoreError> {
        let runtime = self.runtime(broadcast_id).await?;
        let target = self.runtime_target(broadcast_id).await?;
        let target_id = if let Some(target) = target {
            text(&target, "id")
        } else {
            let target_id = format!("target_guest_relay_{}", short_id());
            let now = now();
            sqlx::query(
                "INSERT INTO vanta_live_runtime_targets
                (id, broadcast_id, target_kind, status, protocol, endpoint_url, latency_profile, negotiation_json, created_at, updated_at)
                VALUES (?, ?, 'guest_relay', 'ready', 'webrtc', ?, 'ultra_low', ?, ?, ?)",
            )
            .bind(&target_id)
            .bind(broadcast_id)
            .bind(format!("webrtc://runtime.vanta.local/guest-relays/{broadcast_id}"))
            .bind(json!({"source":"guest_media_relay_worker","requires_connected_guest_webrtc":true}).to_string())
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;
            target_id
        };
        let sessions = self
            .list(
                "SELECT * FROM obs_guest_webrtc_sessions WHERE broadcast_id = ? AND status = 'connected' ORDER BY created_at ASC",
                &[broadcast_id],
            )
            .await?;
        let mut relays = Vec::new();
        for session in sessions {
            let session_id = text(&session, "id");
            if let Some(existing) = self
                .row_optional(
                    "SELECT * FROM obs_guest_media_relays WHERE session_id = ? ORDER BY created_at DESC LIMIT 1",
                    &[&session_id],
                )
                .await?
            {
                relays.push(existing);
                continue;
            }
            let participant = self
                .row(
                    "SELECT * FROM obs_guest_participants WHERE id = ?",
                    &[&text(&session, "participant_id")],
                )
                .await?;
            let now = now();
            let relay_id = format!("guest_relay_{}", short_id());
            let output_id = format!("guest_relay_output_{}", short_id());
            let source_id = optional_text(&participant, "source_id");
            let return_feed = self
                .row_optional(
                    "SELECT * FROM obs_guest_return_feed_sessions WHERE participant_id = ? ORDER BY created_at DESC LIMIT 1",
                    &[&text(&participant, "id")],
                )
                .await?;
            let return_feed_session_id = return_feed.as_ref().map(|row| text(row, "id"));
            let route = json!({
                "relay_id": relay_id,
                "webrtc_session_id": session_id,
                "participant_id": text(&participant, "id"),
                "transport": "webrtc_sfu",
                "program_composition": {
                    "source_id": source_id,
                    "track_selector": "remote_guest_av",
                    "sync_policy": "program_clock",
                    "audio_bus": "mix_minus_and_program",
                    "video_layer": text(&session, "selected_video_layer")
                },
                "return_feed": {
                    "session_id": return_feed_session_id,
                    "mix_minus": true,
                    "program_return": true
                },
                "archive": {
                    "isolated_audio": true,
                    "isolated_video": int(&session, "video_enabled") != 0,
                    "manifest_source": "obs_guest_media_relays"
                }
            });
            let archive_manifest = json!({
                "status": "armed",
                "relay_id": relay_id,
                "participant_id": text(&participant, "id"),
                "webrtc_session_id": session_id,
                "tracks": session.get("tracks_json").cloned().unwrap_or_else(|| json!({})),
                "sync_policy": "program_clock",
                "archive_outputs": ["participant_archive", "isolated_recording", "program_reference"]
            });
            let health = json!({
                "status": "relaying",
                "latency_target_ms": 110,
                "video_layer": text(&session, "selected_video_layer"),
                "source_id": source_id,
                "return_feed_session_id": return_feed_session_id,
                "runtime_output_id": output_id,
                "started_at": now
            });
            sqlx::query(
                "INSERT INTO vanta_live_runtime_outputs
                (id, broadcast_id, ingest_session_id, output_kind, status, target_id, health_json, started_at, ended_at, created_at, updated_at)
                VALUES (?, ?, ?, 'guest_media_relay', 'publishing', ?, ?, ?, NULL, ?, ?)",
            )
            .bind(&output_id)
            .bind(broadcast_id)
            .bind(text(&runtime, "live_ingest_session_id"))
            .bind(&target_id)
            .bind(health.to_string())
            .bind(&now)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;
            sqlx::query(
                "INSERT INTO obs_guest_media_relays
                (id, session_id, participant_id, broadcast_id, status, relay_kind, program_source_id,
                 return_feed_session_id, runtime_output_id, archive_manifest_json, route_json, health_json, created_at, updated_at)
                VALUES (?, ?, ?, ?, 'relaying', 'webrtc_sfu', ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&relay_id)
            .bind(&session_id)
            .bind(text(&participant, "id"))
            .bind(broadcast_id)
            .bind(source_id.clone())
            .bind(return_feed_session_id.clone())
            .bind(&output_id)
            .bind(archive_manifest.to_string())
            .bind(route.to_string())
            .bind(health.to_string())
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;
            if let Some(source_id) = source_id.as_deref() {
                self.attach_relay_to_guest_source(source_id, &relay_id, &route, &health)
                    .await?;
            }
            if return_feed_session_id.is_some() {
                self.attach_relay_to_guest_return_feed(
                    &text(&participant, "id"),
                    &relay_id,
                    &route,
                )
                .await?;
            }
            self.update_guest_relay_state(&text(&participant, "id"), &relay_id, &health)
                .await?;
            relays.push(
                self.row(
                    "SELECT * FROM obs_guest_media_relays WHERE id = ?",
                    &[&relay_id],
                )
                .await?,
            );
        }
        self.add_event(
            Some(broadcast_id),
            "guest_media_relay_reconcile",
            "Guest WebRTC media relays reconciled into runtime outputs",
        )
        .await?;
        Ok(json!({
            "broadcast_id": broadcast_id,
            "status": "ready",
            "relays_json": relays,
            "guest_room": self.guest_room(broadcast_id).await?,
            "runtime": self.runtime(broadcast_id).await?
        }))
    }

    pub async fn ingest_guest_relay_rtp_packet(
        &self,
        relay_id: &str,
        input: GuestRtpPacketInput,
    ) -> Result<Value, ObsStoreError> {
        let relay = self
            .row(
                "SELECT * FROM obs_guest_media_relays WHERE id = ?",
                &[relay_id],
            )
            .await?;
        if text(&relay, "status") != "relaying" {
            return Err(ObsStoreError::Invalid(
                "RTP packets require an active relaying guest media route".to_string(),
            ));
        }
        let bytes = general_purpose::STANDARD
            .decode(input.packet_base64.as_bytes())
            .map_err(|_| {
                ObsStoreError::Invalid("packet_base64 must decode to RTP bytes".to_string())
            })?;
        let packet = parse_guest_rtp_packet(&bytes)?;
        let previous_worker = relay.pointer("/health_json/media_worker").cloned();
        let previous_sequence = previous_worker
            .as_ref()
            .and_then(|worker| worker.get("last_sequence_number"))
            .and_then(Value::as_i64)
            .or_else(|| {
                relay
                    .pointer("/health_json/last_sequence_number")
                    .and_then(Value::as_i64)
            })
            .map(|value| value as u16);
        let packet_order = rtp_packet_order(previous_sequence, packet.sequence_number);
        let dropped_since_last = if matches!(packet_order, "in_order" | "gap") {
            previous_sequence
                .map(|sequence| rtp_sequence_gap(sequence, packet.sequence_number))
                .unwrap_or(0)
        } else {
            0
        };
        let now = now();
        let packet_id = format!("guest_rtp_{}", short_id());
        let received_at_ms = input.received_at_ms.unwrap_or_default();
        let clock_rate = rtp_clock_rate(&input.payload_kind, input.metadata_json.as_ref());
        let codec = rtp_payload_codec(&input.payload_kind, input.metadata_json.as_ref());
        let media_worker = media_worker_state(
            previous_worker.as_ref(),
            &input.payload_kind,
            &codec,
            &packet,
            received_at_ms,
            clock_rate,
            packet_order,
            dropped_since_last,
        );
        let playout_at_ms = received_at_ms
            + media_worker
                .get("target_buffer_ms")
                .and_then(Value::as_i64)
                .unwrap_or(80);
        let frame_id = if packet.marker {
            Some(format!("guest_frame_{}", short_id()))
        } else {
            None
        };
        let packet_json = json!({
            "id": packet_id,
            "relay_id": relay_id,
            "payload_kind": input.payload_kind,
            "sequence_number": packet.sequence_number,
            "rtp_timestamp": packet.timestamp,
            "ssrc": packet.ssrc,
            "marker": packet.marker,
            "payload_type": packet.payload_type,
            "payload_bytes": packet.payload_bytes,
            "payload_base64": general_purpose::STANDARD.encode(&packet.payload),
            "codec": codec,
            "byte_length": bytes.len(),
            "dropped_since_last": dropped_since_last,
            "packet_order": packet_order,
            "received_at_ms": received_at_ms,
            "clock_rate": clock_rate,
            "playout_at_ms": playout_at_ms,
            "frame_id": frame_id.clone(),
            "metadata": input.metadata_json.unwrap_or_else(|| json!({}))
        });
        sqlx::query(
            "INSERT INTO obs_guest_rtp_packets
            (id, relay_id, session_id, participant_id, broadcast_id, payload_kind,
             sequence_number, rtp_timestamp, ssrc, marker, payload_type, byte_length, packet_json, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&packet_id)
        .bind(relay_id)
        .bind(text(&relay, "session_id"))
        .bind(text(&relay, "participant_id"))
        .bind(text(&relay, "broadcast_id"))
        .bind(&input.payload_kind)
        .bind(packet.sequence_number as i64)
        .bind(packet.timestamp as i64)
        .bind(packet.ssrc as i64)
        .bind(bool_int(packet.marker))
        .bind(packet.payload_type as i64)
        .bind(bytes.len() as i64)
        .bind(packet_json.to_string())
        .bind(&now)
        .execute(&self.pool)
        .await?;
        let frame_json = if let Some(frame_id) = frame_id.as_deref() {
            let frame = self
                .create_guest_media_worker_frame(
                    &relay,
                    relay_id,
                    frame_id,
                    &input.payload_kind,
                    &packet,
                    playout_at_ms,
                    &media_worker,
                    &now,
                )
                .await?;
            Some(frame)
        } else {
            None
        };
        let mut health = relay
            .get("health_json")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if !health.is_object() {
            health = json!({});
        }
        if let Some(object) = health.as_object_mut() {
            let packets_total = object
                .get("rtp_packet_count")
                .and_then(Value::as_i64)
                .unwrap_or_default()
                + 1;
            let dropped_total = object
                .get("dropped_packets")
                .and_then(Value::as_i64)
                .unwrap_or_default()
                + dropped_since_last;
            object.insert("status".to_string(), json!("relaying"));
            object.insert("rtp_packet_count".to_string(), json!(packets_total));
            object.insert("dropped_packets".to_string(), json!(dropped_total));
            object.insert("media_worker".to_string(), media_worker.clone());
            if let Some(frame_json) = frame_json.as_ref() {
                object.insert("last_depacketized_frame".to_string(), frame_json.clone());
                if let Some(decoded_frame) = frame_json.get("decoded_frame") {
                    object.insert(
                        "last_decoded_media_frame".to_string(),
                        decoded_frame.clone(),
                    );
                    if let Some(sync_pair) = decoded_frame
                        .get("sync_pairs")
                        .and_then(Value::as_array)
                        .and_then(|pairs| pairs.first())
                    {
                        object.insert("last_media_sync_pair".to_string(), sync_pair.clone());
                        if let Some(compositor_frame) = sync_pair.get("compositor_frame") {
                            object.insert(
                                "last_compositor_frame".to_string(),
                                compositor_frame.clone(),
                            );
                            if let Some(playout_frame) = compositor_frame.get("playout") {
                                object.insert(
                                    "last_compositor_playout_frame".to_string(),
                                    playout_frame.clone(),
                                );
                            }
                        }
                    }
                }
            }
            object.insert("last_payload_kind".to_string(), json!(input.payload_kind));
            if packet_order != "out_of_order" {
                object.insert(
                    "last_sequence_number".to_string(),
                    json!(packet.sequence_number),
                );
            }
            object.insert("last_rtp_timestamp".to_string(), json!(packet.timestamp));
            object.insert("last_ssrc".to_string(), json!(packet.ssrc));
            object.insert("last_packet_at".to_string(), json!(now));
            object.insert(
                "frame_clock".to_string(),
                json!({
                    "ssrc": packet.ssrc,
                    "rtp_timestamp": packet.timestamp,
                    "sequence_number": packet.sequence_number,
                    "marker": packet.marker,
                    "dropped_since_last": dropped_since_last,
                    "packet_order": packet_order,
                    "playout_at_ms": playout_at_ms,
                    "program_sync": "program_clock"
                }),
            );
        }
        sqlx::query(
            "UPDATE obs_guest_media_relays SET health_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(health.to_string())
        .bind(&now)
        .bind(relay_id)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "UPDATE vanta_live_runtime_outputs SET health_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(health.to_string())
        .bind(&now)
        .bind(text(&relay, "runtime_output_id"))
        .execute(&self.pool)
        .await?;
        self.sync_vanta_authoritative_runtime(
            &text(&relay, "broadcast_id"),
            "guest_rtp_packet",
            "relay_packet_ingested",
            json!({
                "relay_id": relay_id,
                "payload_kind": input.payload_kind.clone(),
                "sequence_number": packet.sequence_number,
                "rtp_timestamp": packet.timestamp,
                "ssrc": packet.ssrc,
                "dropped_since_last": dropped_since_last,
                "packet_order": packet_order,
                "playout_at_ms": playout_at_ms,
                "frame": frame_json,
                "relay_health": health.clone()
            }),
        )
        .await?;
        if let Some(source_id) = optional_text(&relay, "program_source_id") {
            sqlx::query(
                "UPDATE obs_source_instances SET settings_json = ?, updated_at = ? WHERE source_id = ?",
            )
            .bind(json!({"relay_id": relay_id, "relay_health": health, "last_rtp_packet": packet_json}).to_string())
            .bind(&now)
            .bind(&source_id)
            .execute(&self.pool)
            .await?;
        }
        self.update_guest_relay_state(&text(&relay, "participant_id"), relay_id, &health)
            .await?;
        self.add_event(
            Some(&text(&relay, "broadcast_id")),
            "guest_rtp_packet",
            "Guest RTP packet relayed into program clock",
        )
        .await?;
        Ok(json!({
            "status": "accepted",
            "packet": packet_json,
            "frame": frame_json.unwrap_or(Value::Null),
            "relay": self.row("SELECT * FROM obs_guest_media_relays WHERE id = ?", &[relay_id]).await?,
            "guest_room": self.guest_room(&text(&relay, "broadcast_id")).await?,
            "runtime": self.runtime(&text(&relay, "broadcast_id")).await?
        }))
    }

    #[allow(clippy::too_many_arguments)]
    async fn create_guest_media_worker_frame(
        &self,
        relay: &Value,
        relay_id: &str,
        frame_id: &str,
        payload_kind: &str,
        packet: &GuestRtpPacket,
        playout_at_ms: i64,
        media_worker: &Value,
        created_at: &str,
    ) -> Result<Value, ObsStoreError> {
        let ssrc = packet.ssrc.to_string();
        let timestamp = packet.timestamp.to_string();
        let packets = self
            .list(
                "SELECT * FROM obs_guest_rtp_packets
                 WHERE relay_id = ? AND payload_kind = ? AND ssrc = ? AND rtp_timestamp = ?
                 ORDER BY sequence_number ASC",
                &[relay_id, payload_kind, &ssrc, &timestamp],
            )
            .await?;
        let packet_count = packets.len() as i64;
        let byte_length = packets
            .iter()
            .map(|packet| int(packet, "byte_length"))
            .sum::<i64>();
        let start_sequence = packets
            .first()
            .map(|packet| int(packet, "sequence_number"))
            .unwrap_or(packet.sequence_number as i64);
        let end_sequence = packets
            .last()
            .map(|packet| int(packet, "sequence_number"))
            .unwrap_or(packet.sequence_number as i64);
        let access_unit = rtp_access_unit_json(payload_kind, media_worker, &packets);
        let decoded_frame = self
            .decode_guest_media_worker_frame(
                relay,
                frame_id,
                payload_kind,
                &access_unit,
                media_worker,
                playout_at_ms,
                created_at,
            )
            .await?;
        let frame_json = json!({
            "id": frame_id,
            "relay_id": relay_id,
            "payload_kind": payload_kind,
            "status": "ready_for_playout",
            "depacketizer": {
                "mode": "rtp_marker_delimited_access_unit",
                "codec": media_worker.get("codec").cloned().unwrap_or_else(|| json!("unknown")),
                "packet_count": packet_count,
                "packet_ids": packets.iter().map(|packet| text(packet, "id")).collect::<Vec<_>>()
            },
            "rtp_timestamp": packet.timestamp,
            "ssrc": packet.ssrc,
            "start_sequence_number": start_sequence,
            "end_sequence_number": end_sequence,
            "byte_length": byte_length,
            "access_unit": access_unit,
            "decoded_frame": decoded_frame,
            "playout": {
                "playout_at_ms": playout_at_ms,
                "target_buffer_ms": media_worker.get("target_buffer_ms").cloned().unwrap_or_else(|| json!(80)),
                "jitter_ms": media_worker.get("jitter_ms").cloned().unwrap_or_else(|| json!(0)),
                "program_sync": "program_clock"
            },
            "routes": {
                "program_composition": relay.pointer("/route_json/program_composition").cloned().unwrap_or_else(|| json!({})),
                "return_feed": relay.pointer("/route_json/return_feed").cloned().unwrap_or_else(|| json!({})),
                "archive": relay.pointer("/route_json/archive").cloned().unwrap_or_else(|| json!({}))
            }
        });
        sqlx::query(
            "INSERT INTO obs_guest_media_worker_frames
            (id, relay_id, session_id, participant_id, broadcast_id, payload_kind, status,
             start_sequence_number, end_sequence_number, rtp_timestamp, ssrc, packet_count,
             byte_length, playout_at_ms, frame_json, created_at)
            VALUES (?, ?, ?, ?, ?, ?, 'ready_for_playout', ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(frame_id)
        .bind(relay_id)
        .bind(text(relay, "session_id"))
        .bind(text(relay, "participant_id"))
        .bind(text(relay, "broadcast_id"))
        .bind(payload_kind)
        .bind(start_sequence)
        .bind(end_sequence)
        .bind(packet.timestamp as i64)
        .bind(packet.ssrc as i64)
        .bind(packet_count)
        .bind(byte_length)
        .bind(playout_at_ms)
        .bind(frame_json.to_string())
        .bind(created_at)
        .execute(&self.pool)
        .await?;
        Ok(frame_json)
    }

    async fn decode_guest_media_worker_frame(
        &self,
        relay: &Value,
        frame_id: &str,
        payload_kind: &str,
        access_unit: &Value,
        media_worker: &Value,
        playout_at_ms: i64,
        created_at: &str,
    ) -> Result<Value, ObsStoreError> {
        let relay_id = text(relay, "id");
        let codec = access_unit
            .get("codec")
            .and_then(Value::as_str)
            .unwrap_or_else(|| {
                media_worker
                    .get("codec")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            });
        let decoded_id = format!("guest_decoded_{}", short_id());
        let access_unit_ready = access_unit
            .get("ready_for_decode")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut decoded = if payload_kind == "video"
            && codec.eq_ignore_ascii_case("h264")
            && access_unit_ready
        {
            decode_h264_access_unit_artifact(&relay_id, frame_id, access_unit).await?
        } else if payload_kind == "audio" && codec.eq_ignore_ascii_case("opus") && access_unit_ready
        {
            decode_opus_access_unit_artifact(&relay_id, frame_id, access_unit).await?
        } else {
            json!({
                "status": "waiting_for_decoder",
                "decodeable": false,
                "reason": if access_unit_ready { "unsupported_payload_decoder" } else { "access_unit_not_ready" },
                "codec": codec,
                "payload_kind": payload_kind
            })
        };
        if let Some(object) = decoded.as_object_mut() {
            object.insert("id".to_string(), json!(decoded_id));
            object.insert("media_worker_frame_id".to_string(), json!(frame_id));
            object.insert("relay_id".to_string(), json!(relay_id));
            object.insert("payload_kind".to_string(), json!(payload_kind));
            object.insert("codec".to_string(), json!(codec));
            object.insert(
                "program_sync".to_string(),
                json!({
                    "clock": "program_clock",
                    "playout_at_ms": playout_at_ms,
                    "sync_policy": "rtp_timestamp_to_program_clock"
                }),
            );
            object.insert(
                "routes".to_string(),
                json!({
                    "program_composition": relay.pointer("/route_json/program_composition").cloned().unwrap_or_else(|| json!({})),
                    "return_feed": relay.pointer("/route_json/return_feed").cloned().unwrap_or_else(|| json!({})),
                    "archive": relay.pointer("/route_json/archive").cloned().unwrap_or_else(|| json!({}))
                }),
            );
        }
        let route_frames = guest_decoded_route_frames(
            relay,
            frame_id,
            &decoded_id,
            &decoded,
            payload_kind,
            codec,
            playout_at_ms,
        );
        if let Some(object) = decoded.as_object_mut() {
            object.insert("route_frames".to_string(), json!(route_frames));
        }
        let status = decoded
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let artifact_path = decoded
            .get("artifact_path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        sqlx::query(
            "INSERT INTO obs_guest_decoded_media_frames
            (id, media_worker_frame_id, relay_id, session_id, participant_id, broadcast_id,
             payload_kind, codec, status, artifact_path, decoded_frame_json, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&decoded_id)
        .bind(frame_id)
        .bind(&relay_id)
        .bind(text(relay, "session_id"))
        .bind(text(relay, "participant_id"))
        .bind(text(relay, "broadcast_id"))
        .bind(payload_kind)
        .bind(codec)
        .bind(status)
        .bind(artifact_path)
        .bind(decoded.to_string())
        .bind(created_at)
        .execute(&self.pool)
        .await?;
        for route_frame in &route_frames {
            sqlx::query(
                "INSERT INTO obs_guest_media_route_frames
                (id, decoded_media_frame_id, media_worker_frame_id, relay_id, participant_id,
                 broadcast_id, route_kind, status, route_frame_json, created_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(text(route_frame, "id"))
            .bind(&decoded_id)
            .bind(frame_id)
            .bind(&relay_id)
            .bind(text(relay, "participant_id"))
            .bind(text(relay, "broadcast_id"))
            .bind(text(route_frame, "route_kind"))
            .bind(text(route_frame, "status"))
            .bind(route_frame.to_string())
            .bind(created_at)
            .execute(&self.pool)
            .await?;
        }
        let sync_pairs = self
            .create_guest_media_sync_pairs(relay, &route_frames, created_at)
            .await?;
        if let Some(object) = decoded.as_object_mut() {
            object.insert("sync_pairs".to_string(), json!(sync_pairs));
        }
        sqlx::query(
            "UPDATE obs_guest_decoded_media_frames SET decoded_frame_json = ? WHERE id = ?",
        )
        .bind(decoded.to_string())
        .bind(&decoded_id)
        .execute(&self.pool)
        .await?;
        Ok(decoded)
    }

    async fn create_guest_media_sync_pairs(
        &self,
        relay: &Value,
        route_frames: &[Value],
        created_at: &str,
    ) -> Result<Vec<Value>, ObsStoreError> {
        let relay_id = text(relay, "id");
        let mut sync_pairs = Vec::new();
        for route_frame in route_frames {
            if text(route_frame, "status") != "ready" {
                continue;
            }
            let payload_kind = text(route_frame, "payload_kind");
            if !matches!(payload_kind.as_str(), "audio" | "video") {
                continue;
            }
            let route_kind = text(route_frame, "route_kind");
            let candidates = self
                .list(
                    "SELECT * FROM obs_guest_media_route_frames
                     WHERE relay_id = ? AND route_kind = ? AND status = 'ready'
                     ORDER BY created_at DESC LIMIT 20",
                    &[&relay_id, &route_kind],
                )
                .await?;
            let Some(opposite) = candidates.into_iter().find(|candidate| {
                candidate
                    .get("route_frame_json")
                    .and_then(|route| route.get("payload_kind"))
                    .and_then(Value::as_str)
                    .is_some_and(|kind| kind != payload_kind)
            }) else {
                continue;
            };
            let opposite_route = opposite
                .get("route_frame_json")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let mut pair = guest_media_sync_pair(relay, route_frame, &opposite_route, created_at);
            let compositor_frame = if text(&pair, "route_kind") == "program_composition" {
                let mut compositor_frame = create_guest_program_compositor_frame(
                    relay,
                    &pair,
                    route_frame,
                    &opposite_route,
                    created_at,
                )
                .await?;
                if text(&compositor_frame, "status") == "ready" {
                    let playout_frame = self
                        .create_guest_compositor_playout_frame(
                            relay,
                            &pair,
                            &compositor_frame,
                            created_at,
                        )
                        .await?;
                    if let Some(object) = compositor_frame.as_object_mut() {
                        object.insert("playout".to_string(), playout_frame);
                    }
                }
                Some(compositor_frame)
            } else {
                None
            };
            if let Some(compositor_frame) = compositor_frame.as_ref() {
                if let Some(object) = pair.as_object_mut() {
                    object.insert("compositor_frame".to_string(), compositor_frame.clone());
                }
            }
            sqlx::query(
                "INSERT INTO obs_guest_media_sync_pairs
                (id, relay_id, participant_id, broadcast_id, route_kind, audio_route_frame_id,
                 video_route_frame_id, sync_status, drift_ms, sync_pair_json, created_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(text(&pair, "id"))
            .bind(&relay_id)
            .bind(text(relay, "participant_id"))
            .bind(text(relay, "broadcast_id"))
            .bind(text(&pair, "route_kind"))
            .bind(text(&pair, "audio_route_frame_id"))
            .bind(text(&pair, "video_route_frame_id"))
            .bind(text(&pair, "sync_status"))
            .bind(int(&pair, "drift_ms"))
            .bind(pair.to_string())
            .bind(created_at)
            .execute(&self.pool)
            .await?;
            if let Some(compositor_frame) = compositor_frame.as_ref() {
                sqlx::query(
                    "INSERT INTO obs_guest_compositor_frames
                    (id, relay_id, participant_id, broadcast_id, route_kind, sync_pair_id,
                     audio_route_frame_id, video_route_frame_id, status, artifact_path,
                     compositor_frame_json, created_at)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(text(compositor_frame, "id"))
                .bind(&relay_id)
                .bind(text(relay, "participant_id"))
                .bind(text(relay, "broadcast_id"))
                .bind(text(compositor_frame, "route_kind"))
                .bind(text(&pair, "id"))
                .bind(text(&pair, "audio_route_frame_id"))
                .bind(text(&pair, "video_route_frame_id"))
                .bind(text(compositor_frame, "status"))
                .bind(text(compositor_frame, "artifact_path"))
                .bind(compositor_frame.to_string())
                .bind(created_at)
                .execute(&self.pool)
                .await?;
            }
            sync_pairs.push(pair);
        }
        Ok(sync_pairs)
    }

    async fn create_guest_compositor_playout_frame(
        &self,
        relay: &Value,
        sync_pair: &Value,
        compositor_frame: &Value,
        created_at: &str,
    ) -> Result<Value, ObsStoreError> {
        let relay_id = text(relay, "id");
        let previous = self
            .row(
                "SELECT * FROM obs_guest_compositor_playout_frames
                 WHERE relay_id = ? AND route_kind = 'program_composition'
                 ORDER BY program_frame_sequence DESC LIMIT 1",
                &[&relay_id],
            )
            .await
            .ok();
        let previous_json = previous
            .as_ref()
            .and_then(|row| row.get("playout_json"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        let previous_sequence = previous
            .as_ref()
            .map(|row| int(row, "program_frame_sequence"))
            .unwrap_or(0);
        let previous_playout_at_ms = previous_json
            .get("actual_playout_at_ms")
            .and_then(Value::as_i64);
        let actual_playout_at_ms = int(sync_pair, "video_playout_at_ms").max(0);
        let frame_interval_ms = 33;
        let expected_playout_at_ms = previous_playout_at_ms
            .map(|playout_at_ms| playout_at_ms + frame_interval_ms)
            .unwrap_or(actual_playout_at_ms);
        let gap_ms = previous_playout_at_ms
            .map(|playout_at_ms| actual_playout_at_ms.saturating_sub(playout_at_ms))
            .unwrap_or(0);
        let dropped_frames = if gap_ms > frame_interval_ms + 16 {
            ((gap_ms + frame_interval_ms / 2) / frame_interval_ms - 1).max(0)
        } else {
            0
        };
        let lateness_ms = actual_playout_at_ms - expected_playout_at_ms;
        let cumulative_dropped_frames = previous_json
            .get("cumulative_dropped_frames")
            .and_then(Value::as_i64)
            .unwrap_or_default()
            + dropped_frames;
        let playout_status = if dropped_frames > 0 {
            "dropped_frames"
        } else if lateness_ms.abs() > 16 {
            "jitter_warning"
        } else {
            "paced"
        };
        let pressure_level =
            if cumulative_dropped_frames >= 2 || dropped_frames > 0 || lateness_ms.abs() > 50 {
                "high"
            } else if lateness_ms.abs() > 16 {
                "medium"
            } else {
                "nominal"
            };
        let degradation_action = match pressure_level {
            "high" => "request_lower_sfu_layer_and_hold_last_good_frame",
            "medium" => "hold_last_good_frame_until_program_clock_recovers",
            _ => "play_current_frame",
        };
        let runtime_playout_artifact =
            create_runtime_gpu_playout_artifact(relay, compositor_frame, previous_sequence + 1)
                .await?;
        let runtime_software_fallback = runtime_playout_artifact
            .get("encoder")
            .and_then(|encoder| encoder.get("hardware_accelerated"))
            .and_then(Value::as_bool)
            .is_none_or(|accelerated| !accelerated);
        let live_feed_session = self
            .upsert_runtime_live_feed_session(
                relay,
                previous_sequence + 1,
                dropped_frames,
                cumulative_dropped_frames,
                lateness_ms,
                pressure_level,
                degradation_action,
                &runtime_playout_artifact,
                runtime_software_fallback,
                created_at,
            )
            .await?;
        let playout = json!({
            "id": format!("guest_compositor_playout_{}", short_id()),
            "relay_id": relay_id,
            "participant_id": text(relay, "participant_id"),
            "broadcast_id": text(relay, "broadcast_id"),
            "route_kind": "program_composition",
            "compositor_frame_id": text(compositor_frame, "id"),
            "sync_pair_id": text(sync_pair, "id"),
            "program_frame_sequence": previous_sequence + 1,
            "playout_status": playout_status,
            "nominal_frame_interval_ms": frame_interval_ms,
            "expected_playout_at_ms": expected_playout_at_ms,
            "actual_playout_at_ms": actual_playout_at_ms,
            "lateness_ms": lateness_ms,
            "gap_ms": gap_ms,
            "dropped_frames": dropped_frames,
            "cumulative_dropped_frames": cumulative_dropped_frames,
            "pressure": {
                "level": pressure_level,
                "degradation_action": degradation_action,
                "protect_host_program": true,
                "protect_audio_continuity": true,
                "target_runtime_latency_ms": 120,
                "max_consecutive_dropped_frames_before_layer_downshift": 2
            },
            "runtime_delivery": {
                "transport": "vanta_realtime_sfu",
                "program_surface": "guest_program_composition",
                "action": degradation_action,
                "frame_source": "runtime_program_playout_chunk",
                "next_engine": "runtime_gpu_playout",
                "playout_artifact": runtime_playout_artifact,
                "live_feed_session": live_feed_session
            },
            "frame_pacing": {
                "clock": "program_clock",
                "policy": "pace_from_video_playout_hold_last_good_frame",
                "software_fallback": runtime_software_fallback
            },
            "created_at": created_at
        });
        sqlx::query(
            "INSERT INTO obs_guest_compositor_playout_frames
            (id, relay_id, participant_id, broadcast_id, route_kind, compositor_frame_id,
             program_frame_sequence, playout_status, dropped_frames, playout_json, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(text(&playout, "id"))
        .bind(text(&playout, "relay_id"))
        .bind(text(&playout, "participant_id"))
        .bind(text(&playout, "broadcast_id"))
        .bind(text(&playout, "route_kind"))
        .bind(text(&playout, "compositor_frame_id"))
        .bind(int(&playout, "program_frame_sequence"))
        .bind(text(&playout, "playout_status"))
        .bind(int(&playout, "dropped_frames"))
        .bind(playout.to_string())
        .bind(created_at)
        .execute(&self.pool)
        .await?;
        Ok(playout)
    }

    async fn upsert_runtime_live_feed_session(
        &self,
        relay: &Value,
        program_frame_sequence: i64,
        dropped_frames: i64,
        cumulative_dropped_frames: i64,
        lateness_ms: i64,
        pressure_level: &str,
        degradation_action: &str,
        runtime_playout_artifact: &Value,
        runtime_software_fallback: bool,
        updated_at: &str,
    ) -> Result<Value, ObsStoreError> {
        let relay_id = text(relay, "id");
        let previous = self
            .row(
                "SELECT * FROM obs_runtime_live_feed_sessions
                 WHERE relay_id = ? AND transport = 'vanta_realtime_sfu'
                 ORDER BY updated_at DESC LIMIT 1",
                &[&relay_id],
            )
            .await
            .ok();
        let previous_json = previous
            .as_ref()
            .and_then(|row| row.get("delivery_json"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        let previous_chunks = previous
            .as_ref()
            .map(|row| int(row, "delivered_chunks"))
            .unwrap_or_default();
        let previous_average_lateness = previous
            .as_ref()
            .and_then(|row| row.get("average_lateness_ms"))
            .and_then(Value::as_f64)
            .unwrap_or_default();
        let previous_max_lateness = previous
            .as_ref()
            .map(|row| int(row, "max_lateness_ms"))
            .unwrap_or_default();
        let artifact_ready = runtime_playout_artifact
            .get("status")
            .and_then(Value::as_str)
            == Some("ready");
        let delivered_chunks = previous_chunks + i64::from(artifact_ready);
        let average_lateness_ms = if delivered_chunks > 0 {
            ((previous_average_lateness * previous_chunks as f64) + lateness_ms.abs() as f64)
                / delivered_chunks as f64
        } else {
            0.0
        };
        let max_lateness_ms = previous_max_lateness.max(lateness_ms.abs());
        let first_program_frame_sequence = previous
            .as_ref()
            .map(|row| int(row, "first_program_frame_sequence"))
            .filter(|sequence| *sequence > 0)
            .unwrap_or(program_frame_sequence);
        let status = if !artifact_ready {
            "stalled"
        } else if pressure_level == "high" || dropped_frames > 0 {
            "degraded"
        } else {
            "live"
        };
        let session_id = previous
            .as_ref()
            .map(|row| text(row, "id"))
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| format!("runtime_live_feed_{}", short_id()));
        let created_at = previous
            .as_ref()
            .map(|row| text(row, "created_at"))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| updated_at.to_string());
        let continuity = json!({
            "mode": "sustained_runtime_gpu_live_feed",
            "transport": "vanta_realtime_sfu",
            "program_surface": "guest_program_composition",
            "first_program_frame_sequence": first_program_frame_sequence,
            "last_program_frame_sequence": program_frame_sequence,
            "delivered_chunks": delivered_chunks,
            "chunk_duration_ms": runtime_playout_artifact
                .pointer("/transport_contract/chunk_duration_ms")
                .cloned()
                .unwrap_or_else(|| json!(1000)),
            "fragmented_mp4": runtime_playout_artifact
                .pointer("/transport_contract/fragmented_mp4")
                .cloned()
                .unwrap_or_else(|| json!(false)),
            "last_artifact_path": runtime_playout_artifact
                .get("artifact_path")
                .cloned()
                .unwrap_or_else(|| json!("")),
            "last_artifact_status": runtime_playout_artifact
                .get("status")
                .cloned()
                .unwrap_or_else(|| json!("unknown")),
            "software_fallback": runtime_software_fallback,
            "program_clock_paced": true,
            "delivery_target_latency_ms": 120,
            "continuity_action": degradation_action,
            "previous": previous_json.get("continuity").cloned().unwrap_or_else(|| json!({}))
        });
        let delivery_json = json!({
            "id": session_id,
            "status": status,
            "relay_id": relay_id,
            "participant_id": text(relay, "participant_id"),
            "broadcast_id": text(relay, "broadcast_id"),
            "pressure": {
                "level": pressure_level,
                "cumulative_dropped_frames": cumulative_dropped_frames,
                "latest_dropped_frames": dropped_frames,
                "average_lateness_ms": average_lateness_ms,
                "max_lateness_ms": max_lateness_ms,
                "action": degradation_action,
                "protect_host_program": true,
                "protect_audio_continuity": true
            },
            "continuity": continuity,
            "updated_at": updated_at
        });
        if previous.is_some() {
            sqlx::query(
                "UPDATE obs_runtime_live_feed_sessions
                 SET status = ?, last_program_frame_sequence = ?, delivered_chunks = ?,
                     cumulative_dropped_frames = ?, average_lateness_ms = ?, max_lateness_ms = ?,
                     pressure_level = ?, delivery_json = ?, updated_at = ?
                 WHERE id = ?",
            )
            .bind(status)
            .bind(program_frame_sequence)
            .bind(delivered_chunks)
            .bind(cumulative_dropped_frames)
            .bind(average_lateness_ms)
            .bind(max_lateness_ms)
            .bind(pressure_level)
            .bind(delivery_json.to_string())
            .bind(updated_at)
            .bind(&session_id)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                "INSERT INTO obs_runtime_live_feed_sessions
                (id, relay_id, participant_id, broadcast_id, transport, program_surface, status,
                 first_program_frame_sequence, last_program_frame_sequence, delivered_chunks,
                 cumulative_dropped_frames, average_lateness_ms, max_lateness_ms, pressure_level,
                 delivery_json, created_at, updated_at)
                VALUES (?, ?, ?, ?, 'vanta_realtime_sfu', 'guest_program_composition', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&session_id)
            .bind(&relay_id)
            .bind(text(relay, "participant_id"))
            .bind(text(relay, "broadcast_id"))
            .bind(status)
            .bind(first_program_frame_sequence)
            .bind(program_frame_sequence)
            .bind(delivered_chunks)
            .bind(cumulative_dropped_frames)
            .bind(average_lateness_ms)
            .bind(max_lateness_ms)
            .bind(pressure_level)
            .bind(delivery_json.to_string())
            .bind(&created_at)
            .bind(updated_at)
            .execute(&self.pool)
            .await?;
        }
        Ok(delivery_json)
    }

    async fn attach_relay_to_guest_source(
        &self,
        source_id: &str,
        relay_id: &str,
        route: &Value,
        health: &Value,
    ) -> Result<(), ObsStoreError> {
        let source = self
            .row("SELECT * FROM obs_sources WHERE id = ?", &[source_id])
            .await?;
        let mut settings = source
            .get("default_settings_json")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if !settings.is_object() {
            settings = json!({});
        }
        if let Some(object) = settings.as_object_mut() {
            object.insert(
                "media_url".to_string(),
                json!(format!("vanta://guest-relay/{relay_id}/program")),
            );
            object.insert("relay_id".to_string(), json!(relay_id));
            object.insert("relay_route".to_string(), route.clone());
        }
        sqlx::query(
            "UPDATE obs_sources SET default_settings_json = ?, health_state = 'ready', updated_at = ? WHERE id = ?",
        )
        .bind(settings.to_string())
        .bind(now())
        .bind(source_id)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "UPDATE obs_source_instances SET settings_json = ?, updated_at = ? WHERE source_id = ?",
        )
        .bind(json!({"relay_id": relay_id, "relay_health": health}).to_string())
        .bind(now())
        .bind(source_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn attach_relay_to_guest_return_feed(
        &self,
        participant_id: &str,
        relay_id: &str,
        route: &Value,
    ) -> Result<(), ObsStoreError> {
        let participant = self
            .row(
                "SELECT * FROM obs_guest_participants WHERE id = ?",
                &[participant_id],
            )
            .await?;
        let mut feed = participant
            .get("return_feed_json")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if !feed.is_object() {
            feed = json!({});
        }
        if let Some(object) = feed.as_object_mut() {
            object.insert("relay_id".to_string(), json!(relay_id));
            object.insert("relay_route".to_string(), route.clone());
            object.insert("media_ingress".to_string(), json!("webrtc_sfu"));
        }
        sqlx::query(
            "UPDATE obs_guest_participants SET return_feed_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(feed.to_string())
        .bind(now())
        .bind(participant_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_guest_relay_state(
        &self,
        participant_id: &str,
        relay_id: &str,
        health: &Value,
    ) -> Result<(), ObsStoreError> {
        let participant = self
            .row(
                "SELECT * FROM obs_guest_participants WHERE id = ?",
                &[participant_id],
            )
            .await?;
        let mut media_state = participant
            .get("media_state_json")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if !media_state.is_object() {
            media_state = json!({});
        }
        if let Some(object) = media_state.as_object_mut() {
            object.insert(
                "media_relay".to_string(),
                json!({
                    "relay_id": relay_id,
                    "status": health.get("status").and_then(Value::as_str).unwrap_or("relaying"),
                    "runtime_output_id": health.get("runtime_output_id").and_then(Value::as_str).unwrap_or_default(),
                    "transport": "webrtc_sfu"
                }),
            );
            object.insert("video_active".to_string(), json!(true));
        }
        sqlx::query(
            "UPDATE obs_guest_participants SET media_state_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(media_state.to_string())
        .bind(now())
        .bind(participant_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_guest_webrtc_state(
        &self,
        participant_id: &str,
        session_id: &str,
        health: &Value,
    ) -> Result<(), ObsStoreError> {
        let participant = self
            .row(
                "SELECT * FROM obs_guest_participants WHERE id = ?",
                &[participant_id],
            )
            .await?;
        let mut media_state = participant
            .get("media_state_json")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if !media_state.is_object() {
            media_state = json!({});
        }
        if let Some(object) = media_state.as_object_mut() {
            object.insert(
                "webrtc_session".to_string(),
                json!({
                    "session_id": session_id,
                    "status": health.get("status").and_then(Value::as_str).unwrap_or("awaiting_sfu_answer"),
                    "ice_candidate_count": health.get("ice_candidate_count").and_then(Value::as_i64).unwrap_or_default(),
                    "selected_video_layer": health.get("selected_video_layer").and_then(Value::as_str).unwrap_or_default(),
                    "transport": "webrtc"
                }),
            );
            object.insert(
                "video_active".to_string(),
                json!(health.get("status").and_then(Value::as_str) == Some("connected")),
            );
        }
        sqlx::query(
            "UPDATE obs_guest_participants SET media_state_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(media_state.to_string())
        .bind(now())
        .bind(participant_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn configure_guest_room_routing(
        &self,
        broadcast_id: &str,
        input: GuestRoomRoutingInput,
    ) -> Result<Value, ObsStoreError> {
        let room = self.guest_room(broadcast_id).await?;
        let room_id = text(&room, "id");
        let mode = input.room_mode;
        let max_participants = input
            .max_participants
            .unwrap_or_else(|| guest_room_default_capacity(&mode));
        let latency_target_ms = input.latency_target_ms.unwrap_or(140).max(60);
        let mirrored_channels = input.mirrored_channels.unwrap_or(false);
        let shared_feed_source_id = input.shared_feed_source_id.unwrap_or_default();
        let shared_source = if shared_feed_source_id.is_empty() {
            None
        } else {
            let source = self
                .row(
                    "SELECT * FROM obs_sources WHERE id = ?",
                    &[&shared_feed_source_id],
                )
                .await?;
            let kind = text(&source, "source_kind");
            if ![
                "screen_capture",
                "display_capture",
                "browser_capture",
                "media_file",
            ]
            .contains(&kind.as_str())
            {
                return Err(ObsStoreError::Invalid(
                    "shared feed source must be screen, display, browser, or media backed"
                        .to_string(),
                ));
            }
            Some(source)
        };
        if mode == "shared_game" && shared_source.is_none() {
            return Err(ObsStoreError::Invalid(
                "shared_game mode requires shared_feed_source_id".to_string(),
            ));
        }
        let now = now();
        let active_speaker = room
            .get("shared_program_context_json")
            .and_then(|value| value.get("active_speaker"))
            .cloned()
            .unwrap_or(Value::Null);
        let participants = room
            .get("participants_json")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let media_transport = guest_room_media_transport_plan(
            &mode,
            max_participants,
            latency_target_ms,
            &shared_feed_source_id,
            shared_source.as_ref(),
            mirrored_channels,
            active_speaker.clone(),
            &participants,
        );
        let shared_context = json!({
            "program_feed": "canonical",
            "room_mode": mode,
            "shared_game_feed": mode == "shared_game",
            "shared_feed_source_id": if shared_feed_source_id.is_empty() { Value::Null } else { json!(shared_feed_source_id) },
            "shared_feed_source_kind": shared_source.as_ref().map_or(Value::Null, |source| json!(text(source, "source_kind"))),
            "latency_target_ms": latency_target_ms,
            "active_speaker": active_speaker,
            "layout_policy": guest_room_layout_policy(&mode, max_participants),
            "media_transport": media_transport,
            "participant_tiles": true,
            "producer_controlled_layouts": true
        });
        let routing_policy = json!({
            "transport": "selective_forwarding",
            "room_mode": mode,
            "max_participants": max_participants,
            "target_latency_ms": latency_target_ms,
            "bandwidth_policy": "preserve_host_program",
            "degrade_guest_first": true,
            "mix_minus": true,
            "mirrored_channels": mirrored_channels,
            "return_video": if mode == "shared_game" { "program_and_shared_feed" } else { "program_return" },
            "return_audio": "mix_minus",
            "simulcast_layers": guest_room_simulcast_layers(max_participants),
            "media_plan": shared_context["media_transport"].clone(),
            "weak_guest_policy": "reduce_guest_layer_before_host_program"
        });
        sqlx::query(
            "UPDATE obs_guest_rooms SET room_mode = ?, max_participants = ?, shared_program_context_json = ?, routing_policy_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&mode)
        .bind(max_participants)
        .bind(shared_context.to_string())
        .bind(routing_policy.to_string())
        .bind(&now)
        .bind(&room_id)
        .execute(&self.pool)
        .await?;
        let return_feed = json!({
            "video": if mode == "shared_game" { "program_and_shared_feed" } else { "program_return" },
            "audio": "mix_minus",
            "shared_game_feed": if mode == "shared_game" { "low_latency" } else { "off" },
            "shared_feed_source_id": if shared_feed_source_id.is_empty() { Value::Null } else { json!(shared_feed_source_id) },
            "mirrored_channel": mirrored_channels,
            "latency_target_ms": latency_target_ms,
            "routing": "selective_forwarding",
            "transport_plan": guest_return_feed_transport_plan(&mode, latency_target_ms, &shared_feed_source_id, None)
        });
        sqlx::query(
            "UPDATE obs_guest_participants SET return_feed_json = ?, updated_at = ? WHERE broadcast_id = ? AND status != 'removed'",
        )
        .bind(return_feed.to_string())
        .bind(&now)
        .bind(broadcast_id)
        .execute(&self.pool)
        .await?;
        self.add_event(
            Some(broadcast_id),
            "guest_room_routing",
            "Guest room collaboration routing updated",
        )
        .await?;
        self.guest_room(broadcast_id).await
    }

    pub async fn negotiate_guest_return_feed(
        &self,
        participant_id: &str,
        input: GuestReturnFeedInput,
    ) -> Result<Value, ObsStoreError> {
        let participant = self
            .row(
                "SELECT * FROM obs_guest_participants WHERE id = ?",
                &[participant_id],
            )
            .await?;
        if text(&participant, "status") == "removed" {
            return Err(ObsStoreError::Invalid(
                "removed guests cannot receive return feeds".to_string(),
            ));
        }
        let broadcast_id = text(&participant, "broadcast_id");
        let session_id = format!("guest_return_{}", short_id());
        let now = now();
        let transport = input
            .transport
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "vanta_realtime_sfu".to_string());
        let latency = input.target_latency_ms.unwrap_or(140).max(60);
        let audio_bitrate = input.audio_bitrate_kbps.unwrap_or(96).max(24);
        let video_bitrate = input.video_bitrate_kbps.unwrap_or(1800).max(250);
        let shared_feed_source_id = input.shared_feed_source_id.unwrap_or_default();
        if !shared_feed_source_id.is_empty() {
            self.row(
                "SELECT * FROM obs_sources WHERE id = ?",
                &[&shared_feed_source_id],
            )
            .await?;
        }
        let room = self.guest_room(&broadcast_id).await?;
        let room_mode = text(&room, "room_mode");
        let audio_track = json!({
            "mode": input.audio_mode,
            "track_id": format!("{session_id}_audio"),
            "codec": "opus",
            "bitrate_kbps": audio_bitrate,
            "sample_rate_hz": 48000,
            "channels": 2,
            "mix_minus": input.audio_mode == "mix_minus",
            "url": format!("vanta://return-feed/{session_id}/audio")
        });
        let video_track = json!({
            "mode": input.video_mode,
            "track_id": format!("{session_id}_video"),
            "codec": "h264",
            "bitrate_kbps": video_bitrate,
            "max_resolution": if input.video_mode == "shared_game" { "1080p60" } else { "720p30" },
            "shared_feed_source_id": if shared_feed_source_id.is_empty() { Value::Null } else { json!(shared_feed_source_id) },
            "url": format!("vanta://return-feed/{session_id}/video")
        });
        let sync = json!({
            "status": "locked",
            "target_latency_ms": latency,
            "audio_video_offset_ms": 0,
            "jitter_buffer_ms": latency.min(180) / 2,
            "priority": "audio_continuity"
        });
        sqlx::query(
            "INSERT INTO obs_guest_return_feed_sessions
            (id, participant_id, broadcast_id, audio_mode, video_mode, transport, target_latency_ms,
             status, audio_track_json, video_track_json, sync_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, 'ready', ?, ?, ?, ?, ?)",
        )
        .bind(&session_id)
        .bind(participant_id)
        .bind(&broadcast_id)
        .bind(text(&audio_track, "mode"))
        .bind(text(&video_track, "mode"))
        .bind(&transport)
        .bind(latency)
        .bind(audio_track.to_string())
        .bind(video_track.to_string())
        .bind(sync.to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        let return_feed = json!({
            "session_id": session_id,
            "status": "ready",
            "transport": transport,
            "audio": text(&audio_track, "mode"),
            "video": text(&video_track, "mode"),
            "audio_track": audio_track,
            "video_track": video_track,
            "sync": sync,
            "shared_game_feed": if text(&video_track, "mode") == "shared_game" { "low_latency" } else { "off" },
            "shared_feed_source_id": if shared_feed_source_id.is_empty() { Value::Null } else { json!(shared_feed_source_id) },
            "latency_target_ms": latency,
            "routing": "selective_forwarding",
            "transport_plan": guest_return_feed_transport_plan(&room_mode, latency, &shared_feed_source_id, Some(&participant))
        });
        sqlx::query(
            "UPDATE obs_guest_participants SET return_feed_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(return_feed.to_string())
        .bind(&now)
        .bind(participant_id)
        .execute(&self.pool)
        .await?;
        self.add_event(
            Some(&broadcast_id),
            "guest_return_feed",
            "Low-latency guest return feed negotiated",
        )
        .await?;
        self.guest_room(&broadcast_id).await
    }

    pub async fn start_guest_isolated_recording(
        &self,
        participant_id: &str,
        input: GuestIsolatedRecordingInput,
    ) -> Result<Value, ObsStoreError> {
        let participant = self
            .row(
                "SELECT * FROM obs_guest_participants WHERE id = ?",
                &[participant_id],
            )
            .await?;
        if text(&participant, "status") == "removed" {
            return Err(ObsStoreError::Invalid(
                "removed guests cannot be isolated-recorded".to_string(),
            ));
        }
        let existing = self
            .row_optional(
                "SELECT * FROM obs_guest_isolated_recordings WHERE participant_id = ? AND status = 'recording' ORDER BY created_at DESC LIMIT 1",
                &[participant_id],
            )
            .await?;
        if existing.is_some() {
            return Err(ObsStoreError::Invalid(
                "an isolated guest recording is already active".to_string(),
            ));
        }
        let recording_id = format!("guest_iso_{}", short_id());
        let broadcast_id = text(&participant, "broadcast_id");
        let started_at = now();
        let include_video = input.include_video.unwrap_or(true);
        let include_audio = input.include_audio.unwrap_or(true);
        let recording_mode = input
            .recording_mode
            .filter(|mode| !mode.trim().is_empty())
            .unwrap_or_else(|| "audio_video".to_string());
        let track_manifest = json!({
            "status": "recording",
            "participant_id": participant_id,
            "source_id": optional_text(&participant, "source_id"),
            "recording_mode": recording_mode,
            "started_at": started_at,
            "tracks": {
                "audio": include_audio,
                "video": include_video,
                "audio_codec": "aac",
                "video_codec": "h264"
            },
            "storage": "local_then_archive",
            "worker": "vanta_obs_isolated_guest_recorder"
        });
        sqlx::query(
            "INSERT INTO obs_guest_isolated_recordings
            (id, participant_id, broadcast_id, source_id, status, recording_mode, started_at, ended_at,
             track_manifest_json, artifact_json, validation_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, 'recording', ?, ?, NULL, ?, ?, ?, ?, ?)",
        )
        .bind(&recording_id)
        .bind(participant_id)
        .bind(&broadcast_id)
        .bind(optional_text(&participant, "source_id"))
        .bind(&recording_mode)
        .bind(&started_at)
        .bind(track_manifest.to_string())
        .bind(json!({"status":"pending"}).to_string())
        .bind(json!({"status":"pending"}).to_string())
        .bind(&started_at)
        .bind(&started_at)
        .execute(&self.pool)
        .await?;
        let isolated = json!({
            "status": "recording",
            "session_id": recording_id,
            "recording_mode": recording_mode,
            "started_at": started_at,
            "tracks": track_manifest["tracks"],
            "storage": "local_then_archive"
        });
        sqlx::query(
            "UPDATE obs_guest_participants SET isolated_recording_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(isolated.to_string())
        .bind(&started_at)
        .bind(participant_id)
        .execute(&self.pool)
        .await?;
        self.add_event(
            Some(&broadcast_id),
            "guest_isolated_recording_start",
            "Isolated guest recording started",
        )
        .await?;
        self.guest_room(&broadcast_id).await
    }

    pub async fn stop_guest_isolated_recording(
        &self,
        participant_id: &str,
    ) -> Result<Value, ObsStoreError> {
        let participant = self
            .row(
                "SELECT * FROM obs_guest_participants WHERE id = ?",
                &[participant_id],
            )
            .await?;
        let recording = self
            .row(
                "SELECT * FROM obs_guest_isolated_recordings WHERE participant_id = ? AND status = 'recording' ORDER BY created_at DESC LIMIT 1",
                &[participant_id],
            )
            .await?;
        let ended_at = now();
        let track_manifest = recording["track_manifest_json"].clone();
        let tracks = track_manifest
            .get("tracks")
            .cloned()
            .unwrap_or_else(|| json!({ "audio": true, "video": true }));
        let include_audio = tracks.get("audio").and_then(Value::as_bool).unwrap_or(true);
        let include_video = tracks.get("video").and_then(Value::as_bool).unwrap_or(true);
        let artifact = recording_media::render_isolated_guest_recording(
            &text(&recording, "broadcast_id"),
            &text(&recording, "id"),
            participant_id,
            &text(&participant, "display_name"),
            &text(&recording, "started_at"),
            &ended_at,
            include_video,
            include_audio,
        )
        .await?;
        let validation = artifact["validation"].clone();
        sqlx::query(
            "UPDATE obs_guest_isolated_recordings SET status = 'ready', ended_at = ?, artifact_json = ?, validation_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&ended_at)
        .bind(artifact.to_string())
        .bind(validation.to_string())
        .bind(&ended_at)
        .bind(text(&recording, "id"))
        .execute(&self.pool)
        .await?;
        let isolated = json!({
            "status": "ready",
            "session_id": text(&recording, "id"),
            "recording_mode": text(&recording, "recording_mode"),
            "started_at": text(&recording, "started_at"),
            "ended_at": ended_at,
            "artifact": artifact,
            "validation": validation,
            "storage": "local_then_archive"
        });
        sqlx::query(
            "UPDATE obs_guest_participants SET isolated_recording_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(isolated.to_string())
        .bind(&ended_at)
        .bind(participant_id)
        .execute(&self.pool)
        .await?;
        self.add_event(
            Some(&text(&recording, "broadcast_id")),
            "guest_isolated_recording_stop",
            "Isolated guest recording finalized",
        )
        .await?;
        self.guest_room(&text(&recording, "broadcast_id")).await
    }

    async fn refresh_guest_active_speaker(
        &self,
        broadcast_id: &str,
        updated_at: &str,
    ) -> Result<(), ObsStoreError> {
        let participants = self
            .list(
                "SELECT * FROM obs_guest_participants WHERE broadcast_id = ? ORDER BY created_at ASC",
                &[broadcast_id],
            )
            .await?;
        let active = participants
            .iter()
            .filter(|participant| guest_can_be_active_speaker(participant))
            .filter_map(|participant| {
                let state = participant.get("media_state_json")?;
                let score = state
                    .get("score")
                    .and_then(Value::as_f64)
                    .unwrap_or_default();
                if score <= 0.0 {
                    return None;
                }
                Some((participant, score))
            })
            .max_by(|(_, left), (_, right)| {
                left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
            });
        for participant in &participants {
            let state = participant
                .get("media_state_json")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let mut next_state = if state.is_object() { state } else { json!({}) };
            if let Some(object) = next_state.as_object_mut() {
                object.insert(
                    "active_speaker".to_string(),
                    json!(
                        active
                            .map(|(active_participant, _)| {
                                text(active_participant, "id") == text(participant, "id")
                            })
                            .unwrap_or(false)
                    ),
                );
            }
            sqlx::query(
                "UPDATE obs_guest_participants SET media_state_json = ?, updated_at = ? WHERE id = ?",
            )
            .bind(next_state.to_string())
            .bind(updated_at)
            .bind(text(participant, "id"))
            .execute(&self.pool)
            .await?;
        }
        let room = self.guest_room(broadcast_id).await?;
        let mut shared = room
            .get("shared_program_context_json")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if !shared.is_object() {
            shared = json!({});
        }
        let mode = text(&room, "room_mode");
        let max_participants = int(&room, "max_participants");
        let routing = room
            .get("routing_policy_json")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let latency_target_ms = int(&routing, "target_latency_ms")
            .max(int(&shared, "latency_target_ms"))
            .max(60);
        let shared_feed_source_id = shared
            .get("shared_feed_source_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let participants = room
            .get("participants_json")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if let Some(object) = shared.as_object_mut() {
            let (active_participant, active_score) = active
                .map_or((None, 0.0), |(participant, score)| {
                    (Some(participant), score)
                });
            object.insert(
                "active_speaker".to_string(),
                active_participant.map_or(Value::Null, |participant| {
                    json!({
                        "participant_id": text(participant, "id"),
                        "display_name": text(participant, "display_name"),
                        "source_id": text(participant, "source_id"),
                        "score": active_score,
                        "selected_at": updated_at
                    })
                }),
            );
            object.insert(
                "active_speaker_policy".to_string(),
                json!({
                    "threshold_db": -55.0,
                    "min_score": 1.0,
                    "ignores_muted_disabled_removed": true,
                    "routing": "layout_and_return_feed_priority"
                }),
            );
            object.insert(
                "media_transport".to_string(),
                guest_room_media_transport_plan(
                    &mode,
                    max_participants,
                    latency_target_ms,
                    &shared_feed_source_id,
                    None,
                    routing
                        .get("mirrored_channels")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    object.get("active_speaker").cloned().unwrap_or(Value::Null),
                    &participants,
                ),
            );
        }
        sqlx::query(
            "UPDATE obs_guest_rooms SET shared_program_context_json = ?, updated_at = ? WHERE broadcast_id = ?",
        )
        .bind(shared.to_string())
        .bind(updated_at)
        .bind(broadcast_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_guest(&self, participant_id: &str) -> Result<Value, ObsStoreError> {
        let participant = self
            .row(
                "SELECT * FROM obs_guest_participants WHERE id = ?",
                &[participant_id],
            )
            .await?;
        let broadcast_id = text(&participant, "broadcast_id");
        sqlx::query("UPDATE obs_guest_participants SET status = 'removed', scene_id = NULL, solo = 0, updated_at = ? WHERE id = ?")
            .bind(now())
            .bind(participant_id)
            .execute(&self.pool)
            .await?;
        self.add_event(
            Some(&broadcast_id),
            "guest_remove",
            "Guest removed from program",
        )
        .await?;
        self.guest_room(&broadcast_id).await
    }

    pub async fn create_instance(
        &self,
        scene_id: &str,
        input: InstanceInput,
    ) -> Result<Value, ObsStoreError> {
        self.create_instance_raw(
            scene_id,
            &input.source_id,
            input.order_index,
            input.x,
            input.y,
            input.width,
            input.height,
            1.0,
        )
        .await
    }

    pub async fn patch_instance(
        &self,
        instance_id: &str,
        input: InstancePatch,
    ) -> Result<Value, ObsStoreError> {
        let current = self
            .row(
                "SELECT * FROM obs_source_instances WHERE id = ?",
                &[instance_id],
            )
            .await?;
        sqlx::query("UPDATE obs_source_instances SET visible = ?, locked = ?, order_index = ?, x = ?, y = ?, width = ?, height = ?, opacity = ?, settings_json = ?, updated_at = ? WHERE id = ?")
            .bind(bool_int(input.visible.unwrap_or_else(|| int(&current, "visible") != 0)))
            .bind(bool_int(input.locked.unwrap_or_else(|| int(&current, "locked") != 0)))
            .bind(input.order_index.unwrap_or_else(|| int(&current, "order_index")))
            .bind(input.x.unwrap_or_else(|| num(&current, "x")))
            .bind(input.y.unwrap_or_else(|| num(&current, "y")))
            .bind(input.width.unwrap_or_else(|| num(&current, "width")))
            .bind(input.height.unwrap_or_else(|| num(&current, "height")))
            .bind(input.opacity.unwrap_or_else(|| num(&current, "opacity")))
            .bind(input.settings_json.unwrap_or_else(|| current["settings_json"].clone()).to_string())
            .bind(now())
            .bind(instance_id)
            .execute(&self.pool)
            .await?;
        self.row(
            "SELECT * FROM obs_source_instances WHERE id = ?",
            &[instance_id],
        )
        .await
    }

    pub async fn create_broadcast(&self, input: BroadcastInput) -> Result<Value, ObsStoreError> {
        let broadcast_id = id();
        self.create_broadcast_with_id(&broadcast_id, input).await?;
        self.row(
            "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
            &[&broadcast_id],
        )
        .await
    }

    pub async fn patch_broadcast(
        &self,
        broadcast_id: &str,
        input: BroadcastPatch,
    ) -> Result<Value, ObsStoreError> {
        let current = self
            .row(
                "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
                &[broadcast_id],
            )
            .await?;
        let tags = input
            .tags
            .map(|tags| {
                tags.into_iter()
                    .map(|tag| tag.trim().to_string())
                    .filter(|tag| !tag.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                current
                    .get("tags_json")
                    .and_then(Value::as_array)
                    .map(|tags| {
                        tags.iter()
                            .filter_map(Value::as_str)
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            });
        let now = now();
        sqlx::query(
            "UPDATE obs_broadcast_profiles
            SET title = ?, category = ?, tags_json = ?, mature_content = ?, language = ?,
                scheduled_start = ?, visibility = ?, follower_notification = ?, chat_mode = ?,
                updated_at = ?
            WHERE id = ?",
        )
        .bind(input.title.unwrap_or_else(|| text(&current, "title")))
        .bind(input.category.unwrap_or_else(|| text(&current, "category")))
        .bind(json!(tags).to_string())
        .bind(bool_int(
            input
                .mature_content
                .unwrap_or_else(|| int(&current, "mature_content") != 0),
        ))
        .bind(input.language.unwrap_or_else(|| text(&current, "language")))
        .bind(
            input
                .scheduled_start
                .or_else(|| optional_text(&current, "scheduled_start")),
        )
        .bind(
            input
                .visibility
                .unwrap_or_else(|| text(&current, "visibility")),
        )
        .bind(bool_int(input.follower_notification.unwrap_or_else(|| {
            int(&current, "follower_notification") != 0
        })))
        .bind(
            input
                .chat_mode
                .unwrap_or_else(|| text(&current, "chat_mode")),
        )
        .bind(&now)
        .bind(broadcast_id)
        .execute(&self.pool)
        .await?;
        self.add_event(
            Some(broadcast_id),
            "channel_update",
            "Live channel metadata updated",
        )
        .await?;
        self.dashboard().await
    }

    pub async fn add_moderator(
        &self,
        broadcast_id: &str,
        input: ModeratorInput,
    ) -> Result<Value, ObsStoreError> {
        self.row(
            "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
            &[broadcast_id],
        )
        .await?;
        let now = now();
        let permissions = match input.role.as_str() {
            "owner" => json!(["queue", "pin", "terms", "roles", "ban"]),
            "producer" => json!(["queue", "pin", "terms"]),
            _ => json!(["queue", "pin"]),
        };
        sqlx::query(
            "INSERT INTO obs_moderator_roles
            (id, broadcast_id, user_id, display_name, role, permissions_json, status, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, 'active', ?, ?)",
        )
        .bind(format!("moderator_{}", short_id()))
        .bind(broadcast_id)
        .bind(input.user_id)
        .bind(input.display_name)
        .bind(input.role)
        .bind(permissions.to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.add_event(Some(broadcast_id), "moderator_role", "Moderator role added")
            .await?;
        self.dashboard().await
    }

    pub async fn add_blocked_term(
        &self,
        broadcast_id: &str,
        input: BlockedTermInput,
    ) -> Result<Value, ObsStoreError> {
        self.row(
            "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
            &[broadcast_id],
        )
        .await?;
        let now = now();
        sqlx::query(
            "INSERT INTO obs_blocked_terms (id, broadcast_id, term, action, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(format!("blocked_term_{}", short_id()))
        .bind(broadcast_id)
        .bind(input.term.trim())
        .bind(input.action.unwrap_or_else(|| "hold".to_string()))
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.add_event(Some(broadcast_id), "blocked_term", "Blocked term added")
            .await?;
        self.dashboard().await
    }

    pub async fn enqueue_moderation(
        &self,
        broadcast_id: &str,
        input: ModerationQueueInput,
    ) -> Result<Value, ObsStoreError> {
        self.row(
            "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
            &[broadcast_id],
        )
        .await?;
        let blocked_terms = self
            .list(
                "SELECT * FROM obs_blocked_terms WHERE broadcast_id = ? ORDER BY created_at ASC",
                &[broadcast_id],
            )
            .await?;
        let message_lower = input.message.to_lowercase();
        let matched_term = blocked_terms
            .iter()
            .map(|term| text(term, "term"))
            .find(|term| !term.is_empty() && message_lower.contains(&term.to_lowercase()));
        let reason = input
            .reason
            .filter(|value| !value.trim().is_empty())
            .or_else(|| matched_term.map(|term| format!("blocked term: {term}")))
            .unwrap_or_else(|| "manual review".to_string());
        let now = now();
        sqlx::query(
            "INSERT INTO obs_moderation_queue
            (id, broadcast_id, author_id, author_name, message, reason, status, moderator_id, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, 'pending', NULL, ?, ?)",
        )
        .bind(format!("mod_queue_{}", short_id()))
        .bind(broadcast_id)
        .bind(input.author_id)
        .bind(input.author_name)
        .bind(input.message)
        .bind(reason)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.add_event(
            Some(broadcast_id),
            "moderation_queue",
            "Chat message queued for moderation",
        )
        .await?;
        self.dashboard().await
    }

    pub async fn resolve_moderation(
        &self,
        item_id: &str,
        input: ModerationResolveInput,
    ) -> Result<Value, ObsStoreError> {
        let item = self
            .row(
                "SELECT * FROM obs_moderation_queue WHERE id = ?",
                &[item_id],
            )
            .await?;
        let broadcast_id = text(&item, "broadcast_id");
        sqlx::query(
            "UPDATE obs_moderation_queue SET status = ?, moderator_id = ?, updated_at = ? WHERE id = ?",
        )
        .bind(input.status)
        .bind(input.moderator_id)
        .bind(now())
        .bind(item_id)
        .execute(&self.pool)
        .await?;
        self.add_event(
            Some(&broadcast_id),
            "moderation_resolve",
            "Moderation queue item resolved",
        )
        .await?;
        self.dashboard().await
    }

    pub async fn pin_message(
        &self,
        broadcast_id: &str,
        input: PinnedMessageInput,
    ) -> Result<Value, ObsStoreError> {
        self.row(
            "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
            &[broadcast_id],
        )
        .await?;
        let now = now();
        sqlx::query("UPDATE obs_pinned_messages SET status = 'unpinned', unpinned_at = ?, updated_at = ? WHERE broadcast_id = ? AND status = 'active'")
            .bind(&now)
            .bind(&now)
            .bind(broadcast_id)
            .execute(&self.pool)
            .await?;
        sqlx::query(
            "INSERT INTO obs_pinned_messages
            (id, broadcast_id, author_name, message, status, pinned_at, unpinned_at, created_at, updated_at)
            VALUES (?, ?, ?, ?, 'active', ?, NULL, ?, ?)",
        )
        .bind(format!("pin_{}", short_id()))
        .bind(broadcast_id)
        .bind(input.author_name)
        .bind(input.message)
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.add_event(Some(broadcast_id), "pinned_message", "Chat message pinned")
            .await?;
        self.dashboard().await
    }

    pub async fn unpin_message(&self, message_id: &str) -> Result<Value, ObsStoreError> {
        let message = self
            .row(
                "SELECT * FROM obs_pinned_messages WHERE id = ?",
                &[message_id],
            )
            .await?;
        let broadcast_id = text(&message, "broadcast_id");
        let now = now();
        sqlx::query("UPDATE obs_pinned_messages SET status = 'unpinned', unpinned_at = ?, updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(&now)
            .bind(message_id)
            .execute(&self.pool)
            .await?;
        self.add_event(
            Some(&broadcast_id),
            "pinned_message",
            "Chat message unpinned",
        )
        .await?;
        self.dashboard().await
    }

    pub async fn moderation_state(&self, broadcast_id: &str) -> Result<Value, ObsStoreError> {
        let moderators = self
            .list(
                "SELECT * FROM obs_moderator_roles WHERE broadcast_id = ? ORDER BY created_at ASC",
                &[broadcast_id],
            )
            .await?;
        let blocked_terms = self
            .list(
                "SELECT * FROM obs_blocked_terms WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 20",
                &[broadcast_id],
            )
            .await?;
        let queue = self
            .list(
                "SELECT * FROM obs_moderation_queue WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 20",
                &[broadcast_id],
            )
            .await?;
        let pinned = self
            .list(
                "SELECT * FROM obs_pinned_messages WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 10",
                &[broadcast_id],
            )
            .await?;
        let pending_count = queue
            .iter()
            .filter(|item| text(item, "status") == "pending")
            .count();
        Ok(json!({
            "moderators_json": moderators,
            "blocked_terms_json": blocked_terms,
            "queue_json": queue,
            "pinned_messages_json": pinned,
            "pending_count": pending_count,
            "active_pin": pinned.iter().find(|message| text(message, "status") == "active").cloned().unwrap_or(Value::Null)
        }))
    }

    async fn require_action_guard(
        &self,
        broadcast: &Value,
        action: &str,
        expected_confirmation: Option<&str>,
        allowed_roles: &[&str],
        required_risks: &[&str],
        input: &impl GuardInput,
    ) -> Result<(), ObsStoreError> {
        let role = input.operator_role().unwrap_or_default();
        let role_allowed = allowed_roles.is_empty() || allowed_roles.contains(&role);
        let confirmation_allowed = expected_confirmation.is_none_or(|expected| {
            input
                .confirmation_text()
                .map(|value| value.trim().eq_ignore_ascii_case(expected))
                .unwrap_or(false)
        });
        let acknowledged = input
            .acknowledged_risks()
            .map(|risks| risks.iter().map(String::as_str).collect::<HashSet<_>>())
            .unwrap_or_default();
        let missing_risks = required_risks
            .iter()
            .filter(|risk| !acknowledged.contains(**risk))
            .copied()
            .collect::<Vec<_>>();

        if role_allowed && confirmation_allowed && missing_risks.is_empty() {
            return Ok(());
        }

        let broadcast_id = text(broadcast, "id");
        let reason = if !role_allowed {
            format!("operator role {role} cannot run {action}")
        } else if !confirmation_allowed {
            format!(
                "{action} requires confirmation phrase {}",
                expected_confirmation.unwrap_or("")
            )
        } else {
            format!(
                "{action} requires acknowledgement for {}",
                missing_risks.join(", ")
            )
        };
        self.add_event_with_severity(
            Some(&broadcast_id),
            "action_guard_block",
            "warning",
            &reason,
        )
        .await?;
        self.record_incident(
            &broadcast_id,
            "action_guard_block",
            "warning",
            "open",
            input.operator_id(),
            &reason,
            None,
            json!({
                "action": action,
                "operator_role": role,
                "expected_confirmation": expected_confirmation,
                "missing_risks": missing_risks
            }),
        )
        .await?;
        Err(ObsStoreError::SafetyBlocked(reason))
    }

    fn campaign_recording_risks(&self, broadcast: &Value) -> Vec<&'static str> {
        if optional_text(broadcast, "sponsor_campaign_id").is_some() {
            vec!["campaign_recording"]
        } else {
            Vec::new()
        }
    }

    pub async fn start_broadcast(&self, broadcast_id: &str) -> Result<Value, ObsStoreError> {
        let collection = self.active_collection().await?;
        let preflight = self
            .evaluate_preflight(&PreflightInput {
                broadcast_id: broadcast_id.to_string(),
                collection_id: text(&collection, "id"),
            })
            .await?;
        if !preflight.ready {
            let blocker_text = preflight.blockers.join(", ");
            self.record_incident(
                broadcast_id,
                "preflight_block",
                "warning",
                "open",
                None,
                &format!("Start blocked by preflight: {blocker_text}"),
                None,
                json!({"blockers":preflight.blockers,"warnings":preflight.warnings}),
            )
            .await?;
            return Err(ObsStoreError::SafetyBlocked(blocker_text));
        }
        let broadcast = self
            .row(
                "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
                &[broadcast_id],
            )
            .await?;
        let runtime = self.runtime(broadcast_id).await?;
        let ingest_session_id = text(&runtime, "live_ingest_session_id");
        let target_id = format!("target_{}", short_id());
        let output_id = format!("output_{}", short_id());
        let readiness_id = format!("playback_ready_{}", short_id());
        let telemetry_id = format!("telemetry_{}", short_id());
        let stream_secret = format!("vanta_live_{}_{}", broadcast_id, short_id());
        let stream_key_hash = stable_hash(&stream_secret);
        let stream_key_hint = stream_secret
            .chars()
            .rev()
            .take(6)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        let latency_profile = text(&broadcast, "latency_profile");
        let protocol = match latency_profile.as_str() {
            "ultra_low" => "webrtc",
            "normal" => "rtmp",
            _ => "srt",
        };
        let target_url = format!(
            "{}://runtime.vanta.local/outputs/{}",
            protocol, broadcast_id
        );
        let now = now();
        let publish = start_local_publish(StreamPublishRequest {
            broadcast_id: broadcast_id.to_string(),
            output_id: output_id.clone(),
            protocol: protocol.to_string(),
            target_url: target_url.clone(),
            latency_profile: latency_profile.clone(),
            width: int(&collection, "canvas_width"),
            height: int(&collection, "canvas_height"),
            frame_rate: int(&collection, "frame_rate"),
            bitrate_kbps: 6200,
        })
        .await?;
        sqlx::query(
            "INSERT INTO vanta_live_ingest_sessions
            (id, creator_id, broadcast_id, status, ingest_protocol, stream_key_hash, stream_key_hint, ingest_url, backup_ingest_url, started_at, ended_at, reconnect_policy_json, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, 'active', ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET status = 'active', ingest_protocol = excluded.ingest_protocol, stream_key_hash = excluded.stream_key_hash, stream_key_hint = excluded.stream_key_hint, ingest_url = excluded.ingest_url, backup_ingest_url = excluded.backup_ingest_url, started_at = excluded.started_at, ended_at = NULL, reconnect_policy_json = excluded.reconnect_policy_json, updated_at = excluded.updated_at",
        )
        .bind(&ingest_session_id)
        .bind(broadcast_id)
        .bind(protocol)
        .bind(stream_key_hash)
        .bind(stream_key_hint)
        .bind(format!("{}://ingest.vanta.local/live/{}", protocol, broadcast_id))
        .bind(format!("{}://backup-ingest.vanta.local/live/{}", protocol, broadcast_id))
        .bind(&now)
        .bind(json!({"max_retries":12,"initial_backoff_ms":500,"max_backoff_ms":8000,"failover":"backup_ingest_url","emergency_scene_id":"scene_emergency_holding"}).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "INSERT INTO vanta_live_runtime_targets
            (id, broadcast_id, target_kind, status, protocol, endpoint_url, latency_profile, negotiation_json, created_at, updated_at)
            VALUES (?, ?, 'vanta_primary', 'ready', ?, ?, ?, ?, ?, ?)",
        )
        .bind(&target_id)
        .bind(broadcast_id)
        .bind(protocol)
        .bind(&target_url)
        .bind(&latency_profile)
        .bind(json!({"selected_protocol":protocol,"fallback_protocols":["rtmp","srt","webrtc"],"quality":"1080p30","viewer_playback_grants_required":true,"start_confirmation":"issued"}).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "INSERT INTO vanta_live_runtime_outputs
            (id, broadcast_id, ingest_session_id, output_kind, status, target_id, health_json, started_at, ended_at, created_at, updated_at)
            VALUES (?, ?, ?, 'program', 'publishing', ?, ?, ?, NULL, ?, ?)",
        )
        .bind(&output_id)
        .bind(broadcast_id)
        .bind(&ingest_session_id)
        .bind(&target_id)
        .bind(publish.health_json.to_string())
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "INSERT INTO vanta_live_playback_readiness
            (id, broadcast_id, ingest_session_id, status, grant_id, playback_url, checks_json, created_at, updated_at)
            VALUES (?, ?, ?, 'ready', ?, ?, ?, ?, ?)",
        )
        .bind(&readiness_id)
        .bind(broadcast_id)
        .bind(&ingest_session_id)
        .bind(format!("playback_grant_{}", short_id()))
        .bind(format!("https://watch.vanta.local/{}/live.m3u8", broadcast_id))
        .bind(json!([{"key":"ingest_session","status":"pass"},{"key":"runtime_output","status":"pass"},{"key":"viewer_grant","status":"pass"}]).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "INSERT INTO vanta_live_runtime_telemetry
            (id, broadcast_id, ingest_session_id, sample_kind, bitrate_kbps, upload_mbps, ingest_latency_ms, dropped_frames, cpu_percent, reconnect_count, health_json, created_at)
            VALUES (?, ?, ?, 'start_confirmation', 6200, 18.4, 830, 12, 44, 0, ?, ?)",
        )
        .bind(&telemetry_id)
        .bind(broadcast_id)
        .bind(&ingest_session_id)
        .bind(json!({"status":"green","bandwidth_estimate_mbps":18.4,"dynamic_bitrate":"stable","viewer_playback_ready":true,"local_publish_manifest":publish.manifest_path}).to_string())
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "UPDATE obs_broadcast_profiles SET status = 'live', updated_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(broadcast_id)
        .execute(&self.pool)
        .await?;
        sqlx::query("UPDATE obs_runtime_bindings SET stream_state = 'live', runtime_state = 'healthy', last_heartbeat_at = ?, updated_at = ? WHERE broadcast_id = ?")
            .bind(&now)
            .bind(&now)
            .bind(broadcast_id)
            .execute(&self.pool)
            .await?;
        self.sync_vanta_authoritative_runtime(
            broadcast_id,
            "stream_start",
            "live",
            json!({
                "output_id": output_id,
                "target_id": target_id,
                "readiness_id": readiness_id,
                "local_publish_manifest": publish.manifest_path
            }),
        )
        .await?;
        self.add_event(
            Some(broadcast_id),
            "stream_start",
            "Vanta runtime ingest issued and stream started",
        )
        .await?;
        self.dashboard().await
    }

    pub async fn ingest_runtime_error(
        &self,
        broadcast_id: &str,
        input: RuntimeErrorInput,
    ) -> Result<Value, ObsStoreError> {
        self.row(
            "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
            &[broadcast_id],
        )
        .await?;
        let severity = input.severity.unwrap_or_else(|| "error".to_string());
        let message = input.message.trim().to_string();
        let error_code = input
            .error_code
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "runtime_error".to_string());
        let source = input
            .source
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "vanta_runtime".to_string());
        let details = input.details_json.unwrap_or_else(|| json!({}));
        let runtime = self.runtime(broadcast_id).await?;
        let ingest_session_id = text(&runtime, "live_ingest_session_id");
        let latest = self.latest_runtime_telemetry(broadcast_id).await?;
        let now = now();
        sqlx::query(
            "INSERT INTO vanta_live_runtime_telemetry
            (id, broadcast_id, ingest_session_id, sample_kind, bitrate_kbps, upload_mbps, ingest_latency_ms, dropped_frames, cpu_percent, reconnect_count, health_json, created_at)
            VALUES (?, ?, ?, 'runtime_error', ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(format!("telemetry_{}", short_id()))
        .bind(broadcast_id)
        .bind(&ingest_session_id)
        .bind(latest.as_ref().and_then(|row| row.get("bitrate_kbps")).and_then(Value::as_i64).unwrap_or(0))
        .bind(latest.as_ref().and_then(|row| row.get("upload_mbps")).and_then(Value::as_f64).unwrap_or(0.0))
        .bind(latest.as_ref().and_then(|row| row.get("ingest_latency_ms")).and_then(Value::as_i64).unwrap_or(0))
        .bind(latest.as_ref().and_then(|row| row.get("dropped_frames")).and_then(Value::as_i64).unwrap_or(0))
        .bind(latest.as_ref().and_then(|row| row.get("cpu_percent")).and_then(Value::as_i64).unwrap_or(0))
        .bind(latest.as_ref().and_then(|row| row.get("reconnect_count")).and_then(Value::as_i64).unwrap_or(0))
        .bind(json!({
            "status": if severity == "critical" { "red" } else { "yellow" },
            "runtime_error": {
                "error_code": error_code,
                "message": message,
                "source": source,
                "severity": severity,
                "details": details
            },
            "viewer_playback_ready": runtime.get("playback_readiness_json").and_then(|row| row.get("status")).and_then(Value::as_str) == Some("ready")
        }).to_string())
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query("UPDATE obs_runtime_bindings SET runtime_state = 'degraded', last_heartbeat_at = ?, updated_at = ? WHERE broadcast_id = ?")
            .bind(&now)
            .bind(&now)
            .bind(broadcast_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("UPDATE vanta_live_runtime_outputs SET status = 'degraded', health_json = ?, updated_at = ? WHERE broadcast_id = ? AND status IN ('publishing', 'ready', 'degraded')")
            .bind(json!({"status":"degraded","runtime_error":{"error_code":error_code,"message":message,"source":source,"severity":severity}}).to_string())
            .bind(&now)
            .bind(broadcast_id)
            .execute(&self.pool)
            .await?;
        self.sync_vanta_authoritative_runtime(
            broadcast_id,
            "runtime_error",
            "degraded",
            json!({
                "error_code": error_code.clone(),
                "message": message.clone(),
                "source": source.clone(),
                "severity": severity.clone(),
                "details": details.clone()
            }),
        )
        .await?;
        self.record_incident(
            broadcast_id,
            "runtime_error",
            &severity,
            "open",
            input.operator_id.as_deref(),
            &message,
            None,
            json!({"error_code":error_code,"source":source,"details":details}),
        )
        .await?;
        self.add_event_with_severity(
            Some(broadcast_id),
            "runtime_error",
            &severity,
            &format!("{source} reported {error_code}: {message}"),
        )
        .await?;
        self.dashboard().await
    }

    pub async fn ingest_runtime_telemetry(
        &self,
        broadcast_id: &str,
        input: RuntimeTelemetryInput,
    ) -> Result<Value, ObsStoreError> {
        self.row(
            "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
            &[broadcast_id],
        )
        .await?;
        let runtime = self.runtime(broadcast_id).await?;
        let ingest_session_id = text(&runtime, "live_ingest_session_id");
        let collection = self.active_collection().await?;
        let ingest = self
            .row_optional(
                "SELECT * FROM vanta_live_ingest_sessions WHERE id = ?",
                &[&ingest_session_id],
            )
            .await?;
        let reconnect_policy = ingest
            .as_ref()
            .and_then(|row| row.get("reconnect_policy_json"))
            .cloned()
            .unwrap_or_else(|| json!({"max_retries":12,"initial_backoff_ms":500,"max_backoff_ms":8000,"failover":"backup_ingest_url"}));
        let latest = self.latest_runtime_telemetry(broadcast_id).await?;
        let previous_runtime_stats = sqlx::query(
            "SELECT COUNT(*) AS sample_count,
                    COALESCE(SUM(dropped_frames), 0) AS dropped_frames,
                    COALESCE(MAX(reconnect_count), 0) AS max_reconnect_count,
                    COALESCE(MAX(ingest_latency_ms), 0) AS max_latency_ms
             FROM vanta_live_runtime_telemetry WHERE broadcast_id = ?",
        )
        .bind(broadcast_id)
        .fetch_one(&self.pool)
        .await?;
        let existing_output = self.runtime_output(broadcast_id).await?;
        let reconnect_count = input.reconnect_count.unwrap_or_else(|| {
            latest
                .as_ref()
                .and_then(|row| row.get("reconnect_count"))
                .and_then(Value::as_i64)
                .unwrap_or_default()
        });
        let health = stream_health_for(&input, reconnect_count);
        let output_status = match health["status"].as_str().unwrap_or("yellow") {
            "green" => "publishing",
            "yellow" => "degraded",
            _ => "reconnecting",
        };
        let ingest_status = if reconnect_count > 0 {
            "reconnecting"
        } else {
            "active"
        };
        let mut health = merge_output_health(existing_output.as_ref(), health);
        health["long_session"] = runtime_long_session_health(
            previous_runtime_stats.get::<i64, _>("sample_count"),
            previous_runtime_stats.get::<i64, _>("dropped_frames"),
            previous_runtime_stats.get::<i64, _>("max_reconnect_count"),
            previous_runtime_stats.get::<i64, _>("max_latency_ms"),
            latest
                .as_ref()
                .and_then(|row| row.get("ingest_latency_ms"))
                .and_then(Value::as_i64),
            &input,
            reconnect_count,
        );
        health["reconnect_attempts"] = reconnect_attempt_plan(
            reconnect_count,
            output_status,
            ingest_status,
            &reconnect_policy,
        );
        if reconnect_count == 0
            && output_status == "publishing"
            && existing_output
                .as_ref()
                .is_some_and(|output| text(output, "status") == "reconnecting")
        {
            let target = self.runtime_target(broadcast_id).await?;
            let target_url = target
                .as_ref()
                .map(|row| text(row, "endpoint_url"))
                .unwrap_or_else(|| format!("srt://runtime.vanta.local/outputs/{broadcast_id}"));
            let protocol = target
                .as_ref()
                .map(|row| text(row, "protocol"))
                .unwrap_or_else(|| "srt".to_string());
            let recovered_publish = start_local_publish(StreamPublishRequest {
                broadcast_id: broadcast_id.to_string(),
                output_id: format!("output_recovered_{}", short_id()),
                protocol,
                target_url,
                latency_profile: target
                    .as_ref()
                    .map(|row| text(row, "latency_profile"))
                    .unwrap_or_else(|| "low".to_string()),
                width: int(&collection, "canvas_width"),
                height: int(&collection, "canvas_height"),
                frame_rate: int(&collection, "frame_rate"),
                bitrate_kbps: health
                    .pointer("/adaptation/target_bitrate_kbps")
                    .and_then(Value::as_i64)
                    .unwrap_or(6200),
            })
            .await?;
            health["local_publish"] = recovered_publish.health_json["local_publish"].clone();
            health["reconnect_attempts"]["recovered_manifest_path"] =
                json!(recovered_publish.manifest_path);
        }
        let runtime_state = match health["status"].as_str().unwrap_or("yellow") {
            "green" => "healthy",
            "yellow" => "degraded",
            _ => "reconnecting",
        };
        let sample_kind = input
            .sample_kind
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "runtime_sample".to_string());
        let now = now();
        sqlx::query(
            "INSERT INTO vanta_live_runtime_telemetry
            (id, broadcast_id, ingest_session_id, sample_kind, bitrate_kbps, upload_mbps, ingest_latency_ms, dropped_frames, cpu_percent, reconnect_count, health_json, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(format!("telemetry_{}", short_id()))
        .bind(broadcast_id)
        .bind(&ingest_session_id)
        .bind(&sample_kind)
        .bind(input.bitrate_kbps)
        .bind(input.upload_mbps)
        .bind(input.ingest_latency_ms)
        .bind(input.dropped_frames)
        .bind(input.cpu_percent)
        .bind(reconnect_count)
        .bind(health.to_string())
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query("UPDATE vanta_live_runtime_outputs SET status = ?, health_json = ?, updated_at = ? WHERE broadcast_id = ? AND status IN ('publishing', 'ready', 'degraded', 'reconnecting')")
            .bind(output_status)
            .bind(health.to_string())
            .bind(&now)
            .bind(broadcast_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("UPDATE obs_runtime_bindings SET runtime_state = ?, last_heartbeat_at = ?, updated_at = ? WHERE broadcast_id = ?")
            .bind(runtime_state)
            .bind(&now)
            .bind(&now)
            .bind(broadcast_id)
            .execute(&self.pool)
            .await?;
        if reconnect_count > 0 {
            sqlx::query("UPDATE vanta_live_ingest_sessions SET status = 'reconnecting', updated_at = ? WHERE id = ? AND status = 'active'")
                .bind(&now)
                .bind(&ingest_session_id)
                .execute(&self.pool)
                .await?;
        } else if health["status"].as_str() == Some("green") {
            sqlx::query("UPDATE vanta_live_ingest_sessions SET status = 'active', updated_at = ? WHERE id = ? AND status = 'reconnecting'")
                .bind(&now)
                .bind(&ingest_session_id)
                .execute(&self.pool)
                .await?;
        }
        self.sync_vanta_authoritative_runtime(
            broadcast_id,
            &sample_kind,
            runtime_state,
            json!({
                "bitrate_kbps": input.bitrate_kbps,
                "upload_mbps": input.upload_mbps,
                "ingest_latency_ms": input.ingest_latency_ms,
                "dropped_frames": input.dropped_frames,
                "cpu_percent": input.cpu_percent,
                "reconnect_count": reconnect_count,
                "health": health.clone()
            }),
        )
        .await?;
        self.add_event_with_severity(
            Some(broadcast_id),
            "runtime_telemetry",
            if health["status"].as_str() == Some("red") {
                "critical"
            } else if health["status"].as_str() == Some("yellow") {
                "warning"
            } else {
                "info"
            },
            &format!(
                "Runtime {}: {} kbps at {:.1} mbps upload",
                health["status"].as_str().unwrap_or("yellow"),
                input.bitrate_kbps,
                input.upload_mbps
            ),
        )
        .await?;
        self.dashboard().await
    }

    pub async fn end_broadcast(
        &self,
        broadcast_id: &str,
        input: ActionConfirmationInput,
    ) -> Result<Value, ObsStoreError> {
        let broadcast = self
            .row(
                "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
                &[broadcast_id],
            )
            .await?;
        let required_risks = self.campaign_recording_risks(&broadcast);
        self.require_action_guard(
            &broadcast,
            "stream_end",
            Some("END STREAM"),
            &["creator_owner", "producer", "live_ops"],
            &required_risks,
            &input,
        )
        .await?;
        let runtime = self.runtime(broadcast_id).await?;
        let ingest_session_id = text(&runtime, "live_ingest_session_id");
        let now = now();
        sqlx::query("UPDATE vanta_live_ingest_sessions SET status = 'ended', ended_at = ?, updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(&now)
            .bind(&ingest_session_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("UPDATE vanta_live_runtime_outputs SET status = 'ended', ended_at = ?, updated_at = ? WHERE broadcast_id = ? AND status IN ('publishing', 'ready', 'degraded', 'held')")
            .bind(&now)
            .bind(&now)
            .bind(broadcast_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("UPDATE vanta_live_playback_readiness SET status = 'ended', updated_at = ? WHERE broadcast_id = ?")
            .bind(&now)
            .bind(broadcast_id)
            .execute(&self.pool)
            .await?;
        sqlx::query(
            "INSERT INTO vanta_live_runtime_telemetry
            (id, broadcast_id, ingest_session_id, sample_kind, bitrate_kbps, upload_mbps, ingest_latency_ms, dropped_frames, cpu_percent, reconnect_count, health_json, created_at)
            VALUES (?, ?, ?, 'end_confirmation', 0, 0.0, 0, 12, 31, 0, ?, ?)",
        )
        .bind(format!("telemetry_{}", short_id()))
        .bind(broadcast_id)
        .bind(&ingest_session_id)
        .bind(json!({"status":"ended","archive_packaging":"queued","viewer_playback_ready":false}).to_string())
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "UPDATE obs_broadcast_profiles SET status = 'ended', updated_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(broadcast_id)
        .execute(&self.pool)
        .await?;
        sqlx::query("UPDATE obs_runtime_bindings SET stream_state = 'ended', runtime_state = 'post_show', last_heartbeat_at = ?, updated_at = ? WHERE broadcast_id = ?")
            .bind(&now)
            .bind(&now)
            .bind(broadcast_id)
            .execute(&self.pool)
            .await?;
        self.sync_vanta_authoritative_runtime(
            broadcast_id,
            "stream_end",
            "ended",
            json!({
                "ended_at": now,
                "archive_packaging": "queued",
                "viewer_playback_ready": false
            }),
        )
        .await?;
        self.add_event(
            Some(broadcast_id),
            "stream_end",
            "Stream ended and archive packaging started",
        )
        .await?;
        self.ensure_post_show(broadcast_id).await?;
        self.dashboard().await
    }

    pub async fn emergency_disconnect(
        &self,
        broadcast_id: &str,
        input: EmergencyDisconnectInput,
    ) -> Result<Value, ObsStoreError> {
        let holding_scene = self
            .row_optional(
                "SELECT * FROM obs_scenes WHERE name = 'Emergency Holding' ORDER BY updated_at DESC LIMIT 1",
                &[],
            )
            .await?;
        let holding_scene_id = holding_scene
            .as_ref()
            .map(|scene| text(scene, "id"))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "scene_emergency_holding".to_string());
        let now = now();
        sqlx::query("UPDATE obs_runtime_bindings SET stream_state = 'emergency_disconnected', runtime_state = 'safe_mode', program_scene_id = ?, active_scene_id = ?, last_heartbeat_at = ?, updated_at = ? WHERE broadcast_id = ?")
            .bind(&holding_scene_id)
            .bind(&holding_scene_id)
            .bind(&now)
            .bind(&now)
            .bind(broadcast_id)
            .execute(&self.pool)
            .await?;
        sqlx::query(
            "UPDATE obs_broadcast_profiles SET status = 'interrupted', updated_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(broadcast_id)
        .execute(&self.pool)
        .await?;
        sqlx::query("UPDATE vanta_live_ingest_sessions SET status = 'emergency_disconnected', ended_at = ?, updated_at = ? WHERE broadcast_id = ? AND status = 'active'")
            .bind(&now)
            .bind(&now)
            .bind(broadcast_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("UPDATE vanta_live_runtime_outputs SET status = 'emergency_disconnected', ended_at = ?, updated_at = ? WHERE broadcast_id = ? AND status IN ('publishing', 'ready', 'degraded', 'held')")
            .bind(&now)
            .bind(&now)
            .bind(broadcast_id)
            .execute(&self.pool)
            .await?;
        let reason = input
            .reason
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "Operator emergency disconnect".to_string());
        self.record_incident(
            broadcast_id,
            "emergency_disconnect",
            "critical",
            "open",
            input.operator_id.as_deref(),
            &reason,
            Some(&holding_scene_id),
            json!({"action":"routed_to_holding_scene","forced_output_stop":true}),
        )
        .await?;
        self.add_event(
            Some(broadcast_id),
            "emergency_disconnect",
            "Emergency disconnect routed program to holding scene",
        )
        .await?;
        self.dashboard().await
    }

    pub async fn live_ops_override(
        &self,
        broadcast_id: &str,
        input: LiveOpsOverrideInput,
    ) -> Result<Value, ObsStoreError> {
        let broadcast = self
            .row(
                "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
                &[broadcast_id],
            )
            .await?;
        let required_risks = if input.action == "force_end" {
            self.campaign_recording_risks(&broadcast)
        } else {
            Vec::new()
        };
        self.require_action_guard(
            &broadcast,
            &format!("live_ops_{}", input.action),
            if input.action == "force_end" {
                Some("FORCE END")
            } else {
                None
            },
            &["creator_owner", "producer", "live_ops"],
            &required_risks,
            &input,
        )
        .await?;
        let now = now();
        let reason = input.reason.trim().to_string();
        match input.action.as_str() {
            "force_end" => {
                sqlx::query("UPDATE vanta_live_ingest_sessions SET status = 'force_ended', ended_at = ?, updated_at = ? WHERE broadcast_id = ? AND status IN ('active', 'emergency_disconnected')")
                    .bind(&now)
                    .bind(&now)
                    .bind(broadcast_id)
                    .execute(&self.pool)
                    .await?;
                sqlx::query("UPDATE vanta_live_runtime_outputs SET status = 'force_ended', ended_at = ?, updated_at = ? WHERE broadcast_id = ? AND status IN ('publishing', 'ready', 'degraded', 'held', 'emergency_disconnected')")
                    .bind(&now)
                    .bind(&now)
                    .bind(broadcast_id)
                    .execute(&self.pool)
                    .await?;
                sqlx::query("UPDATE vanta_live_playback_readiness SET status = 'ended', updated_at = ? WHERE broadcast_id = ?")
                    .bind(&now)
                    .bind(broadcast_id)
                    .execute(&self.pool)
                    .await?;
                sqlx::query("UPDATE obs_runtime_bindings SET stream_state = 'ended', runtime_state = 'live_ops_force_ended', last_heartbeat_at = ?, updated_at = ? WHERE broadcast_id = ?")
                    .bind(&now)
                    .bind(&now)
                    .bind(broadcast_id)
                    .execute(&self.pool)
                    .await?;
                sqlx::query("UPDATE obs_broadcast_profiles SET status = 'force_ended', updated_at = ? WHERE id = ?")
                    .bind(&now)
                    .bind(broadcast_id)
                    .execute(&self.pool)
                    .await?;
                self.ensure_post_show(broadcast_id).await?;
            }
            "safe_mode" => {
                let holding_scene_id = input
                    .target_scene_id
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "scene_emergency_holding".to_string());
                self.row(
                    "SELECT * FROM obs_scenes WHERE id = ?",
                    &[&holding_scene_id],
                )
                .await?;
                sqlx::query("UPDATE obs_runtime_bindings SET runtime_state = 'safe_mode', stream_state = 'live_ops_hold', program_scene_id = ?, active_scene_id = ?, last_heartbeat_at = ?, updated_at = ? WHERE broadcast_id = ?")
                    .bind(&holding_scene_id)
                    .bind(&holding_scene_id)
                    .bind(&now)
                    .bind(&now)
                    .bind(broadcast_id)
                    .execute(&self.pool)
                    .await?;
                sqlx::query("UPDATE vanta_live_runtime_outputs SET status = 'held', health_json = ?, updated_at = ? WHERE broadcast_id = ? AND status IN ('publishing', 'ready', 'degraded')")
                    .bind(json!({"status":"held","override":"safe_mode","reason":reason}).to_string())
                    .bind(&now)
                    .bind(broadcast_id)
                    .execute(&self.pool)
                    .await?;
            }
            "clear_incidents" => {
                sqlx::query("UPDATE obs_runtime_incidents SET status = 'resolved', operator_id = COALESCE(operator_id, ?), updated_at = ? WHERE broadcast_id = ? AND status = 'open'")
                    .bind(input.operator_id.as_deref())
                    .bind(&now)
                    .bind(broadcast_id)
                    .execute(&self.pool)
                    .await?;
            }
            _ => {
                return Err(ObsStoreError::Invalid(
                    "unsupported live ops override action".to_string(),
                ));
            }
        }
        if input.action != "clear_incidents" {
            self.record_incident(
                broadcast_id,
                "live_ops_override",
                if input.action == "force_end" {
                    "critical"
                } else {
                    "warning"
                },
                if input.action == "force_end" {
                    "resolved"
                } else {
                    "open"
                },
                input.operator_id.as_deref(),
                &reason,
                input.target_scene_id.as_deref(),
                json!({"action":input.action.clone(),"operator_override":true}),
            )
            .await?;
        }
        self.add_event_with_severity(
            Some(broadcast_id),
            "live_ops_override",
            if input.action == "force_end" {
                "critical"
            } else {
                "warning"
            },
            &format!("Live Ops override {}: {}", input.action, reason),
        )
        .await?;
        self.dashboard().await
    }

    pub async fn start_recording(
        &self,
        broadcast_id: &str,
        input: RecordingInput,
    ) -> Result<Value, ObsStoreError> {
        let job = id();
        let now = now();
        let recording_mode = input.recording_mode;
        let existing = self
            .row_optional(
                "SELECT * FROM obs_recording_jobs WHERE broadcast_id = ? AND status IN ('recording', 'paused') ORDER BY created_at DESC LIMIT 1",
                &[broadcast_id],
            )
            .await?;
        if existing.is_some() {
            return Err(ObsStoreError::Invalid(
                "an active recording already exists for this broadcast".to_string(),
            ));
        }
        let output_paths =
            recording_media::start_layout(broadcast_id, &job, &recording_mode, &now).await?;
        sqlx::query(
            "INSERT INTO obs_recording_jobs
            (id, creator_id, broadcast_id, live_ingest_session_id, recording_mode, status, started_at, ended_at, output_media_asset_id, output_paths_json, error_message, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, 'ingest_prime_launch', ?, 'recording', ?, NULL, NULL, ?, NULL, ?, ?)",
        )
        .bind(&job)
        .bind(broadcast_id)
        .bind(recording_mode)
        .bind(&now)
        .bind(output_paths.to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query("UPDATE obs_runtime_bindings SET recording_state = 'recording', updated_at = ? WHERE broadcast_id = ?")
            .bind(&now)
            .bind(broadcast_id)
            .execute(&self.pool)
            .await?;
        self.add_event(
            Some(broadcast_id),
            "recording_start",
            "Runtime-backed recording started",
        )
        .await?;
        self.row("SELECT * FROM obs_recording_jobs WHERE id = ?", &[&job])
            .await
    }

    pub async fn pause_recording(&self, broadcast_id: &str) -> Result<Value, ObsStoreError> {
        let active = self
            .row(
                "SELECT * FROM obs_recording_jobs WHERE broadcast_id = ? AND status = 'recording' ORDER BY created_at DESC LIMIT 1",
                &[broadcast_id],
            )
            .await?;
        let paused_at = now();
        let output_paths = recording_media::pause_layout(&active["output_paths_json"], &paused_at)?;
        sqlx::query("UPDATE obs_recording_jobs SET status = 'paused', output_paths_json = ?, updated_at = ? WHERE id = ?")
            .bind(output_paths.to_string())
            .bind(&paused_at)
            .bind(text(&active, "id"))
            .execute(&self.pool)
            .await?;
        sqlx::query("UPDATE obs_runtime_bindings SET recording_state = 'paused', updated_at = ? WHERE broadcast_id = ?")
            .bind(&paused_at)
            .bind(broadcast_id)
            .execute(&self.pool)
            .await?;
        self.add_event(
            Some(broadcast_id),
            "recording_pause",
            "Recording paused safely",
        )
        .await?;
        self.row(
            "SELECT * FROM obs_recording_jobs WHERE id = ?",
            &[&text(&active, "id")],
        )
        .await
    }

    pub async fn resume_recording(&self, broadcast_id: &str) -> Result<Value, ObsStoreError> {
        let active = self
            .row(
                "SELECT * FROM obs_recording_jobs WHERE broadcast_id = ? AND status = 'paused' ORDER BY created_at DESC LIMIT 1",
                &[broadcast_id],
            )
            .await?;
        let resumed_at = now();
        let output_paths =
            recording_media::resume_layout(&active["output_paths_json"], &resumed_at)?;
        sqlx::query("UPDATE obs_recording_jobs SET status = 'recording', output_paths_json = ?, updated_at = ? WHERE id = ?")
            .bind(output_paths.to_string())
            .bind(&resumed_at)
            .bind(text(&active, "id"))
            .execute(&self.pool)
            .await?;
        sqlx::query("UPDATE obs_runtime_bindings SET recording_state = 'recording', updated_at = ? WHERE broadcast_id = ?")
            .bind(&resumed_at)
            .bind(broadcast_id)
            .execute(&self.pool)
            .await?;
        self.add_event(
            Some(broadcast_id),
            "recording_resume",
            "Recording resumed safely",
        )
        .await?;
        self.row(
            "SELECT * FROM obs_recording_jobs WHERE id = ?",
            &[&text(&active, "id")],
        )
        .await
    }

    pub async fn stop_recording(
        &self,
        broadcast_id: &str,
        input: ActionConfirmationInput,
    ) -> Result<Value, ObsStoreError> {
        let broadcast = self
            .row(
                "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
                &[broadcast_id],
            )
            .await?;
        let required_risks = self.campaign_recording_risks(&broadcast);
        self.require_action_guard(
            &broadcast,
            "recording_stop",
            Some("STOP RECORDING"),
            &["creator_owner", "producer", "live_ops"],
            &required_risks,
            &input,
        )
        .await?;
        let active = self
            .row(
                "SELECT * FROM obs_recording_jobs WHERE broadcast_id = ? AND status IN ('recording', 'paused') ORDER BY created_at DESC LIMIT 1",
                &[broadcast_id],
            )
            .await?;
        let participant_rows = self
            .list(
                "SELECT * FROM obs_guest_participants WHERE broadcast_id = ? AND status != 'removed' ORDER BY created_at ASC",
                &[broadcast_id],
            )
            .await?;
        let participant_inputs = participant_archive_inputs(&participant_rows);
        let ended_at = now();
        let media_asset_id = format!("media_asset_recording_{}", short_id());
        let output_paths = recording_media::finalize_layout(
            broadcast_id,
            &text(&active, "id"),
            &text(&active, "recording_mode"),
            &media_asset_id,
            &text(&active, "started_at"),
            &ended_at,
            &active["output_paths_json"],
            &participant_inputs,
        )
        .await?;
        self.persist_participant_archives(&participant_rows, &output_paths, &ended_at)
            .await?;
        self.persist_recording_media_asset(broadcast_id, &media_asset_id, &output_paths, &ended_at)
            .await?;
        sqlx::query("UPDATE obs_recording_jobs SET status = 'packaging', ended_at = ?, output_media_asset_id = ?, output_paths_json = ?, updated_at = ? WHERE id = ?")
            .bind(&ended_at)
            .bind(&media_asset_id)
            .bind(output_paths.to_string())
            .bind(&ended_at)
            .bind(text(&active, "id"))
            .execute(&self.pool)
            .await?;
        sqlx::query("UPDATE obs_runtime_bindings SET recording_state = 'packaging', updated_at = ? WHERE broadcast_id = ?")
            .bind(now())
            .bind(broadcast_id)
            .execute(&self.pool)
            .await?;
        self.add_event(
            Some(broadcast_id),
            "recording_stop",
            "Recording stopped and package job queued",
        )
        .await?;
        Ok(json!(
            self.list(
                "SELECT * FROM obs_recording_jobs WHERE broadcast_id = ? ORDER BY created_at DESC",
                &[broadcast_id]
            )
            .await?
        ))
    }

    pub async fn discard_recording(
        &self,
        broadcast_id: &str,
        input: ActionConfirmationInput,
    ) -> Result<Value, ObsStoreError> {
        let broadcast = self
            .row(
                "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
                &[broadcast_id],
            )
            .await?;
        let required_risks = self.campaign_recording_risks(&broadcast);
        self.require_action_guard(
            &broadcast,
            "recording_discard",
            Some("DISCARD RECORDING"),
            &["creator_owner", "producer", "live_ops"],
            &required_risks,
            &input,
        )
        .await?;
        let recording = self
            .row(
                "SELECT * FROM obs_recording_jobs WHERE broadcast_id = ? AND status IN ('recording', 'paused', 'packaging') ORDER BY created_at DESC LIMIT 1",
                &[broadcast_id],
            )
            .await?;
        let discarded_at = now();
        let output_paths =
            recording_media::discard_layout(&recording["output_paths_json"], &discarded_at).await?;
        if !text(&recording, "output_media_asset_id").is_empty() {
            sqlx::query("UPDATE vanta_media_assets SET status = 'discarded', metadata_json = ?, validation_json = ?, updated_at = ? WHERE id = ?")
                .bind(output_paths.to_string())
                .bind(output_paths["integrity"].to_string())
                .bind(&discarded_at)
                .bind(text(&recording, "output_media_asset_id"))
                .execute(&self.pool)
                .await?;
        }
        sqlx::query("UPDATE obs_recording_jobs SET status = 'discarded', ended_at = COALESCE(ended_at, ?), output_paths_json = ?, error_message = NULL, updated_at = ? WHERE id = ?")
            .bind(&discarded_at)
            .bind(output_paths.to_string())
            .bind(&discarded_at)
            .bind(text(&recording, "id"))
            .execute(&self.pool)
            .await?;
        sqlx::query("UPDATE obs_runtime_bindings SET recording_state = 'discarded', updated_at = ? WHERE broadcast_id = ?")
            .bind(&discarded_at)
            .bind(broadcast_id)
            .execute(&self.pool)
            .await?;
        self.add_event_with_severity(
            Some(broadcast_id),
            "recording_discard",
            "warning",
            "Recording package discarded by operator confirmation",
        )
        .await?;
        self.row(
            "SELECT * FROM obs_recording_jobs WHERE id = ?",
            &[&text(&recording, "id")],
        )
        .await
    }

    async fn persist_recording_media_asset(
        &self,
        broadcast_id: &str,
        media_asset_id: &str,
        output_paths: &Value,
        now: &str,
    ) -> Result<(), ObsStoreError> {
        let asset = &output_paths["vanta_asset"];
        if asset.as_object().is_none_or(|value| value.is_empty()) {
            return Ok(());
        }
        sqlx::query(
            "INSERT INTO vanta_media_assets
            (id, creator_id, broadcast_id, asset_kind, status, source_path, asset_path, manifest_path, metadata_json, validation_json, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, 'recording_package', 'ready', ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET status = excluded.status, source_path = excluded.source_path, asset_path = excluded.asset_path, manifest_path = excluded.manifest_path, metadata_json = excluded.metadata_json, validation_json = excluded.validation_json, updated_at = excluded.updated_at",
        )
        .bind(media_asset_id)
        .bind(broadcast_id)
        .bind(text(output_paths, "manifest"))
        .bind(text(asset, "asset_dir"))
        .bind(text(asset, "manifest_path"))
        .bind(asset.to_string())
        .bind(output_paths["integrity"].to_string())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn persist_participant_archives(
        &self,
        participants: &[Value],
        output_paths: &Value,
        now: &str,
    ) -> Result<(), ObsStoreError> {
        let archives = output_paths
            .get("participant_archives")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for participant in participants {
            let participant_id = text(participant, "id");
            let Some(archive) = archives.iter().find(|archive| {
                archive.get("participant_id").and_then(Value::as_str)
                    == Some(participant_id.as_str())
            }) else {
                continue;
            };
            let mut isolated = participant["isolated_recording_json"].clone();
            if isolated.as_object().is_none() {
                isolated = json!({});
            }
            let Some(object) = isolated.as_object_mut() else {
                continue;
            };
            object.insert("status".to_string(), json!("archived_participant_package"));
            object.insert(
                "archive".to_string(),
                json!({
                    "status": archive["status"],
                    "archive_id": archive["id"],
                    "path": archive["path"],
                    "sha256": archive["sha256"],
                    "source_feed": archive["source_feed"],
                    "source_mode": archive["source_mode"],
                    "validation": archive["validation"],
                    "updated_at": now
                }),
            );
            sqlx::query(
                "UPDATE obs_guest_participants SET isolated_recording_json = ?, updated_at = ? WHERE id = ?",
            )
            .bind(isolated.to_string())
            .bind(now)
            .bind(&participant_id)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    pub async fn save_replay(
        &self,
        broadcast_id: &str,
        input: ReplayInput,
    ) -> Result<Value, ObsStoreError> {
        self.save_replay_with_clip(
            broadcast_id,
            id(),
            input,
            ReplayClip {
                media_asset_id: String::new(),
                output_path: String::new(),
                manifest_json: json!({}),
                pressure_json: json!({"disk_pressure":"unknown","memory_pressure":"unknown"}),
                buffer_json: json!({}),
                upload_queue_json: json!({"mode":"deferred_local_queue","status":"queued"}),
                asset_json: json!({}),
                segments: Vec::new(),
            },
        )
        .await
    }

    pub async fn replay_media_source(
        &self,
        broadcast_id: &str,
    ) -> Result<Option<ReplayMediaSource>, ObsStoreError> {
        if let Some(recording) = self
            .row_optional(
                "SELECT * FROM obs_recording_jobs
                WHERE broadcast_id = ? AND status IN ('packaging', 'recording', 'paused')
                ORDER BY updated_at DESC LIMIT 1",
                &[broadcast_id],
            )
            .await?
            && let Some(source) = self.replay_source_from_recording(&recording)
        {
            return Ok(Some(source));
        }

        if let Some(asset) = self
            .row_optional(
                "SELECT * FROM vanta_media_assets
                WHERE broadcast_id = ? AND asset_kind = 'recording_package' AND status = 'ready'
                ORDER BY updated_at DESC LIMIT 1",
                &[broadcast_id],
            )
            .await?
            && let Some(source) = self.replay_source_from_recording_asset(&asset)
        {
            return Ok(Some(source));
        }

        Ok(None)
    }

    pub async fn save_replay_with_clip(
        &self,
        broadcast_id: &str,
        marker: String,
        input: ReplayInput,
        clip: ReplayClip,
    ) -> Result<Value, ObsStoreError> {
        let now = now();
        let clip_id = format!("clip_draft_{}", short_id());
        self.replace_replay_buffer_segments(broadcast_id, &clip.segments, &now)
            .await?;
        self.persist_vanta_media_asset(broadcast_id, &clip, &now)
            .await?;
        sqlx::query(
            "INSERT INTO obs_replay_markers
            (id, creator_id, broadcast_id, label, duration_seconds, sponsor_proof, status, clip_media_asset_id, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, ?, ?, ?, 'clip_draft_ready', ?, ?, ?)",
        )
        .bind(&marker)
        .bind(broadcast_id)
        .bind(input.label.unwrap_or_else(|| format!("Last {} seconds", input.duration_seconds)))
        .bind(input.duration_seconds)
        .bind(bool_int(input.sponsor_proof.unwrap_or(false)))
        .bind(if clip.media_asset_id.is_empty() {
            &clip_id
        } else {
            &clip.media_asset_id
        })
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "INSERT INTO obs_replay_clip_drafts
            (id, creator_id, replay_marker_id, broadcast_id, clip_media_asset_id, status, output_path, manifest_json, pressure_json, buffer_json, upload_queue_json, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, ?, ?, 'queued', ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id())
        .bind(&marker)
        .bind(broadcast_id)
        .bind(if clip.media_asset_id.is_empty() {
            &clip_id
        } else {
            &clip.media_asset_id
        })
        .bind(&clip.output_path)
        .bind(clip.manifest_json.to_string())
        .bind(clip.pressure_json.to_string())
        .bind(clip.buffer_json.to_string())
        .bind(clip.upload_queue_json.to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.add_event(
            Some(broadcast_id),
            "replay_save",
            "Replay buffer saved as clip draft",
        )
        .await?;
        self.replay_marker(&marker).await
    }

    async fn persist_vanta_media_asset(
        &self,
        broadcast_id: &str,
        clip: &ReplayClip,
        now: &str,
    ) -> Result<(), ObsStoreError> {
        if clip.media_asset_id.is_empty()
            || clip
                .asset_json
                .as_object()
                .is_none_or(|value| value.is_empty())
        {
            return Ok(());
        }
        sqlx::query(
            "INSERT INTO vanta_media_assets
            (id, creator_id, broadcast_id, asset_kind, status, source_path, asset_path, manifest_path, metadata_json, validation_json, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, 'replay_clip', 'ready', ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET status = excluded.status, source_path = excluded.source_path, asset_path = excluded.asset_path, manifest_path = excluded.manifest_path, metadata_json = excluded.metadata_json, validation_json = excluded.validation_json, updated_at = excluded.updated_at",
        )
        .bind(&clip.media_asset_id)
        .bind(broadcast_id)
        .bind(&clip.output_path)
        .bind(text(&clip.asset_json, "asset_path"))
        .bind(text(&clip.asset_json, "manifest_path"))
        .bind(clip.asset_json["metadata_json"].to_string())
        .bind(clip.asset_json["validation_json"].to_string())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn replace_replay_buffer_segments(
        &self,
        broadcast_id: &str,
        segments: &[crate::obs::replay_media::ReplayBufferSegment],
        now: &str,
    ) -> Result<(), ObsStoreError> {
        if segments.is_empty() {
            return Ok(());
        }
        sqlx::query("DELETE FROM obs_replay_buffer_segments WHERE broadcast_id = ?")
            .bind(broadcast_id)
            .execute(&self.pool)
            .await?;
        for segment in segments {
            sqlx::query(
                "INSERT INTO obs_replay_buffer_segments
                (id, creator_id, broadcast_id, segment_index, duration_seconds, status, artifact_path, validation_json, pressure_json, created_at, updated_at)
                VALUES (?, 'creator_vanta_originals', ?, ?, ?, 'ready', ?, ?, ?, ?, ?)",
            )
            .bind(&segment.id)
            .bind(broadcast_id)
            .bind(segment.segment_index)
            .bind(segment.duration_seconds)
            .bind(&segment.artifact_path)
            .bind(segment.validation_json.to_string())
            .bind(segment.pressure_json.to_string())
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    fn replay_source_candidate(
        &self,
        source_kind: &str,
        source_path: &str,
        source_id: Option<String>,
        metadata_json: Value,
    ) -> Option<ReplayMediaSource> {
        if source_path.trim().is_empty() || !Path::new(source_path).is_file() {
            return None;
        }
        Some(ReplayMediaSource {
            source_kind: source_kind.to_string(),
            source_path: source_path.to_string(),
            source_id,
            metadata_json,
        })
    }

    fn replay_source_from_recording(&self, recording: &Value) -> Option<ReplayMediaSource> {
        let output_paths = recording.get("output_paths_json")?;
        let segment = select_replay_video_segment(output_paths.get("segments")?)?;
        let path = segment.get("path").and_then(Value::as_str)?;
        self.replay_source_candidate(
            "recording_program_segment",
            path,
            Some(text(recording, "id")),
            json!({
                "recording_id": text(recording, "id"),
                "recording_status": text(recording, "status"),
                "recording_mode": text(recording, "recording_mode"),
                "feed": segment.get("feed").cloned().unwrap_or(Value::Null),
                "segment_id": segment.get("id").cloned().unwrap_or(Value::Null),
                "segment_index": segment.get("index").cloned().unwrap_or(Value::Null),
                "source_table": "obs_recording_jobs"
            }),
        )
    }

    fn replay_source_from_recording_asset(&self, asset: &Value) -> Option<ReplayMediaSource> {
        let metadata = asset.get("metadata_json")?;
        let segment = select_replay_video_segment(metadata.get("segments")?)?;
        let path = segment
            .get("asset_path")
            .or_else(|| segment.get("source_path"))
            .and_then(Value::as_str)?;
        self.replay_source_candidate(
            "recording_asset_segment",
            path,
            Some(text(asset, "id")),
            json!({
                "asset_id": text(asset, "id"),
                "asset_kind": text(asset, "asset_kind"),
                "feed": segment.get("feed").cloned().unwrap_or(Value::Null),
                "source_table": "vanta_media_assets"
            }),
        )
    }

    pub async fn create_cue(
        &self,
        broadcast_id: &str,
        input: CueInput,
    ) -> Result<Value, ObsStoreError> {
        self.create_cue_for_broadcast(broadcast_id, input).await
    }

    pub async fn trigger_cue(&self, cue_id: &str) -> Result<Value, ObsStoreError> {
        let proof = format!("proof_{}", short_id());
        sqlx::query("UPDATE obs_live_cues SET status = 'shown_live', proof_marker_id = ?, updated_at = ? WHERE id = ?")
            .bind(proof)
            .bind(now())
            .bind(cue_id)
            .execute(&self.pool)
            .await?;
        let cue = self
            .row("SELECT * FROM obs_live_cues WHERE id = ?", &[cue_id])
            .await?;
        self.add_event(
            cue["broadcast_id"].as_str(),
            "cue_trigger",
            &format!("{} shown live", text(&cue, "label")),
        )
        .await?;
        Ok(cue)
    }

    pub async fn save_preflight(
        &self,
        input: PreflightInput,
    ) -> Result<PreflightResult, ObsStoreError> {
        let result = self.evaluate_preflight(&input).await?;
        sqlx::query(
            "INSERT INTO obs_preflight_checks
            (id, creator_id, broadcast_id, collection_id, ready, checks_json, blockers_json, warnings_json, created_at)
            VALUES (?, 'creator_vanta_originals', ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id())
        .bind(&input.broadcast_id)
        .bind(&input.collection_id)
        .bind(bool_int(result.ready))
        .bind(serde_json::to_string(&result.checks)?)
        .bind(serde_json::to_string(&result.blockers)?)
        .bind(serde_json::to_string(&result.warnings)?)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(result)
    }

    pub async fn runtime(&self, broadcast_id: &str) -> Result<Value, ObsStoreError> {
        let mut runtime = self
            .row(
                "SELECT * FROM obs_runtime_bindings WHERE broadcast_id = ?",
                &[broadcast_id],
            )
            .await?;
        let broadcast_id = broadcast_id.to_string();
        let target = self.runtime_target(&broadcast_id).await?;
        let readiness = self.playback_readiness(&broadcast_id).await?;
        let output = self.runtime_output(&broadcast_id).await?;
        let transition = self.latest_transition(&broadcast_id).await?;
        let authoritative_binding = self.vanta_authoritative_binding(&broadcast_id).await?;
        let authoritative_events = self.vanta_authoritative_events(&broadcast_id).await?;
        let recording = self
            .row_optional(
                "SELECT * FROM obs_recording_jobs WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 1",
                &[&broadcast_id],
            )
            .await?;
        let runtime_status = self
            .runtime_status(
                &broadcast_id,
                &runtime,
                target.as_ref(),
                output.as_ref(),
                readiness.as_ref(),
                authoritative_binding.as_ref(),
                recording.as_ref(),
            )
            .await?;
        if let Some(object) = runtime.as_object_mut() {
            if let Some(target) = target {
                object.insert("runtime_target_json".to_string(), target);
            }
            if let Some(readiness) = readiness {
                object.insert("playback_readiness_json".to_string(), readiness);
            }
            if let Some(output) = output {
                object.insert("runtime_output_json".to_string(), output);
            }
            if let Some(authoritative_binding) = authoritative_binding {
                object.insert(
                    "authoritative_binding_json".to_string(),
                    authoritative_binding,
                );
            }
            object.insert(
                "authoritative_events_json".to_string(),
                json!(authoritative_events),
            );
            if let Some(transition) = transition {
                object.insert("latest_transition_json".to_string(), transition);
            }
            if let Some(recording) = recording {
                object.insert("latest_recording_json".to_string(), recording);
            }
            object.insert("runtime_status_json".to_string(), runtime_status);
        }
        Ok(runtime)
    }

    async fn runtime_status(
        &self,
        broadcast_id: &str,
        runtime: &Value,
        target: Option<&Value>,
        output: Option<&Value>,
        readiness: Option<&Value>,
        authoritative_binding: Option<&Value>,
        recording: Option<&Value>,
    ) -> Result<Value, ObsStoreError> {
        let ingest_session_id = text(runtime, "live_ingest_session_id");
        let ingest = self
            .row_optional(
                "SELECT * FROM vanta_live_ingest_sessions WHERE id = ?",
                &[&ingest_session_id],
            )
            .await?;
        let broadcast = self
            .row(
                "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
                &[broadcast_id],
            )
            .await?;
        let telemetry = self.latest_runtime_telemetry(broadcast_id).await?;
        let post_show = self.post_show(broadcast_id).await?;
        let guest_relays = self
            .list(
                "SELECT * FROM obs_guest_media_relays WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 12",
                &[broadcast_id],
            )
            .await?;
        let native_fallback = fallback_plan();
        let sources = self.sources().await?;
        let mut ready_sources = 0;
        let mut warning_sources = 0;
        let mut blocked_sources = 0;
        for source in &sources {
            match source
                .pointer("/source_validation_json/status")
                .and_then(Value::as_str)
                .unwrap_or("blocked")
            {
                "ready" => ready_sources += 1,
                "warning" => warning_sources += 1,
                _ => blocked_sources += 1,
            }
        }
        let source_status = if blocked_sources > 0 {
            "blocked"
        } else if warning_sources > 0 {
            "warning"
        } else {
            "ready"
        };
        let reconnect_policy = ingest
            .as_ref()
            .and_then(|row| row.get("reconnect_policy_json"))
            .cloned()
            .unwrap_or_else(|| json!({"max_retries":0,"failover":"none"}));
        let reconnect_count = telemetry
            .as_ref()
            .and_then(|row| row.get("reconnect_count"))
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let stream_health = telemetry
            .as_ref()
            .and_then(|row| row.get("health_json"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        let post_show_status = text(&post_show, "status");
        let archive_status = post_show
            .get("metrics_json")
            .and_then(|metrics| metrics.get("archive_integrity"))
            .and_then(Value::as_str)
            .unwrap_or(&post_show_status)
            .to_string();
        Ok(json!({
            "reconnect": {
                "status": if reconnect_count > 0 { "recovering" } else { "armed" },
                "count": reconnect_count,
                "policy": reconnect_policy,
                "ingest_status": ingest.as_ref().map(|row| text(row, "status")).unwrap_or_else(|| "pending".to_string())
            },
            "stream_health": {
                "status": stream_health.get("status").and_then(Value::as_str).unwrap_or("pending"),
                "bandwidth_estimate_mbps": stream_health.get("bandwidth_estimate_mbps").and_then(Value::as_f64).unwrap_or_default(),
                "dynamic_bitrate": stream_health.get("dynamic_bitrate").and_then(Value::as_str).unwrap_or("pending"),
                "adaptation": stream_health.get("adaptation").cloned().unwrap_or_else(|| json!({})),
                "thresholds": stream_health.get("thresholds").cloned().unwrap_or_else(|| json!({}))
            },
            "channel": {
                "title": text(&broadcast, "title"),
                "category": text(&broadcast, "category"),
                "tags": broadcast.get("tags_json").cloned().unwrap_or_else(|| json!([])),
                "mature_content": int(&broadcast, "mature_content") != 0,
                "language": text(&broadcast, "language"),
                "scheduled_start": optional_text(&broadcast, "scheduled_start"),
                "visibility": text(&broadcast, "visibility"),
                "follower_notification": int(&broadcast, "follower_notification") != 0,
                "chat_mode": text(&broadcast, "chat_mode"),
                "live_status": text(&broadcast, "status")
            },
            "packaging": {
                "status": if text(runtime, "stream_state") == "ended" { post_show_status.clone() } else { "ready".to_string() },
                "recording_status": recording.map(|row| text(row, "status")).unwrap_or_else(|| text(runtime, "recording_state")),
                "post_show_status": post_show_status
            },
            "archive": {
                "status": archive_status,
                "package_id": text(&post_show, "id"),
                "clip_count": post_show.pointer("/metrics_json/clip_pack_count").and_then(Value::as_i64).unwrap_or_default(),
                "proof_count": post_show.pointer("/metrics_json/proof_count").and_then(Value::as_i64).unwrap_or_default()
            },
            "source_validation": {
                "status": source_status,
                "total": sources.len(),
                "ready": ready_sources,
                "warning": warning_sources,
                "blocked": blocked_sources
            },
            "guest_media_relays": guest_relays,
            "native_fallback": native_fallback,
            "authoritative_vanta_live": {
                "authority": authoritative_binding.map(|row| text(row, "authority")).unwrap_or_else(|| "pending".to_string()),
                "status": authoritative_binding.map(|row| text(row, "status")).unwrap_or_else(|| "unbound".to_string()),
                "version": authoritative_binding.and_then(|row| row.get("version")).and_then(Value::as_i64).unwrap_or_default(),
                "source_of_truth": authoritative_binding.and_then(|row| row.pointer("/binding_json/source_of_truth")).and_then(Value::as_str).unwrap_or("vanta_live_tables"),
                "external_broadcast_id": authoritative_binding.map(|row| text(row, "external_broadcast_id")).unwrap_or_default()
            },
            "target_status": target.map(|row| text(row, "status")).unwrap_or_else(|| "pending".to_string()),
            "output_status": output.map(|row| text(row, "status")).unwrap_or_else(|| "standby".to_string()),
            "playback_status": readiness.map(|row| text(row, "status")).unwrap_or_else(|| "pending".to_string())
        }))
    }

    pub async fn health(&self, broadcast_id: &str) -> Result<Value, ObsStoreError> {
        let runtime = self.runtime(broadcast_id).await?;
        let telemetry = self.latest_runtime_telemetry(broadcast_id).await?;
        let latest_error = self
            .row_optional(
                "SELECT * FROM obs_runtime_incidents WHERE broadcast_id = ? AND incident_kind = 'runtime_error' ORDER BY created_at DESC LIMIT 1",
                &[broadcast_id],
            )
            .await?;
        let live = text(&runtime, "stream_state") == "live";
        let telemetry_health = telemetry
            .as_ref()
            .and_then(|row| row.get("health_json"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        Ok(json!({
            "status": telemetry_health.get("status").and_then(Value::as_str).unwrap_or(if live { "green" } else { "yellow" }),
            "bitrate_kbps": telemetry.as_ref().and_then(|row| row.get("bitrate_kbps")).and_then(Value::as_i64).unwrap_or(6200),
            "upload_mbps": telemetry.as_ref().and_then(|row| row.get("upload_mbps")).and_then(Value::as_f64).unwrap_or(18.4),
            "bandwidth_estimate_mbps": telemetry_health.get("bandwidth_estimate_mbps").and_then(Value::as_f64).unwrap_or_else(|| telemetry.as_ref().and_then(|row| row.get("upload_mbps")).and_then(Value::as_f64).unwrap_or(18.4)),
            "dynamic_bitrate": telemetry_health.get("dynamic_bitrate").and_then(Value::as_str).unwrap_or("stable"),
            "adaptation_json": telemetry_health.get("adaptation").cloned().unwrap_or_else(|| json!({})),
            "thresholds_json": telemetry_health.get("thresholds").cloned().unwrap_or_else(|| json!({})),
            "dropped_frames": telemetry.as_ref().and_then(|row| row.get("dropped_frames")).and_then(Value::as_i64).unwrap_or(12),
            "cpu_percent": 38,
            "runtime_cpu_percent": telemetry.as_ref().and_then(|row| row.get("cpu_percent")).and_then(Value::as_i64).unwrap_or(44),
            "free_disk_gb": 412,
            "ingest_latency_ms": telemetry.as_ref().and_then(|row| row.get("ingest_latency_ms")).and_then(Value::as_i64).unwrap_or(830),
            "reconnect_count": telemetry.as_ref().and_then(|row| row.get("reconnect_count")).and_then(Value::as_i64).unwrap_or(0),
            "viewer_playback_ready": runtime.get("playback_readiness_json").and_then(|row| row.get("status")).and_then(Value::as_str) == Some("ready"),
            "packaging_status": if text(&runtime, "stream_state") == "ended" { "packaging" } else { "ready" },
            "artifact_health": "ready",
            "native_fallback_json": runtime.pointer("/runtime_status_json/native_fallback").cloned().unwrap_or_else(|| fallback_plan()),
            "last_runtime_error": latest_error.unwrap_or(Value::Null)
        }))
    }

    async fn vanta_authoritative_binding(
        &self,
        broadcast_id: &str,
    ) -> Result<Option<Value>, ObsStoreError> {
        self.row_optional(
            "SELECT * FROM vanta_live_authoritative_bindings WHERE broadcast_id = ? ORDER BY version DESC, updated_at DESC LIMIT 1",
            &[broadcast_id],
        )
        .await
    }

    async fn vanta_authoritative_events(
        &self,
        broadcast_id: &str,
    ) -> Result<Vec<Value>, ObsStoreError> {
        self.list(
            "SELECT * FROM vanta_live_authoritative_events WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 20",
            &[broadcast_id],
        )
        .await
    }

    async fn sync_vanta_authoritative_runtime(
        &self,
        broadcast_id: &str,
        event_kind: &str,
        status: &str,
        payload: Value,
    ) -> Result<Value, ObsStoreError> {
        let runtime = self
            .row(
                "SELECT * FROM obs_runtime_bindings WHERE broadcast_id = ?",
                &[broadcast_id],
            )
            .await?;
        let broadcast = self
            .row(
                "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
                &[broadcast_id],
            )
            .await?;
        let ingest_session_id = text(&runtime, "live_ingest_session_id");
        let ingest = self
            .row_optional(
                "SELECT * FROM vanta_live_ingest_sessions WHERE id = ?",
                &[&ingest_session_id],
            )
            .await?;
        let target = self.runtime_target(broadcast_id).await?;
        let output = self.runtime_output(broadcast_id).await?;
        let readiness = self.playback_readiness(broadcast_id).await?;
        let telemetry = self.latest_runtime_telemetry(broadcast_id).await?;
        let version: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM vanta_live_authoritative_bindings WHERE broadcast_id = ?",
        )
        .bind(broadcast_id)
        .fetch_one(&self.pool)
        .await?;
        let binding_id = format!("vanta_live_binding_{broadcast_id}");
        let now = now();
        let binding_json = json!({
            "authority": "vanta_live",
            "canonical_tables": [
                "vanta_live_ingest_sessions",
                "vanta_live_runtime_targets",
                "vanta_live_runtime_outputs",
                "vanta_live_playback_readiness",
                "vanta_live_runtime_telemetry",
                "vanta_live_authoritative_bindings",
                "vanta_live_authoritative_events"
            ],
            "obs_runtime_binding_id": text(&runtime, "id"),
            "live_ingest_session_id": ingest_session_id,
            "runtime_target_id": target.as_ref().map(|row| text(row, "id")),
            "runtime_output_id": output.as_ref().map(|row| text(row, "id")),
            "playback_readiness_id": readiness.as_ref().map(|row| text(row, "id")),
            "latest_telemetry_id": telemetry.as_ref().map(|row| text(row, "id")),
            "source_of_truth": "vanta_live_tables"
        });
        let snapshot = json!({
            "authority": "vanta_live",
            "version": version,
            "event_kind": event_kind,
            "status": status,
            "broadcast": {
                "id": broadcast_id,
                "external_broadcast_id": format!("vanta_live_{broadcast_id}"),
                "status": text(&broadcast, "status"),
                "title": text(&broadcast, "title"),
                "category": text(&broadcast, "category")
            },
            "runtime": {
                "runtime_state": text(&runtime, "runtime_state"),
                "stream_state": text(&runtime, "stream_state"),
                "recording_state": text(&runtime, "recording_state"),
                "program_scene_id": text(&runtime, "program_scene_id"),
                "preview_scene_id": optional_text(&runtime, "preview_scene_id")
            },
            "ingest": ingest.as_ref().map(|row| json!({
                "id": text(row, "id"),
                "status": text(row, "status"),
                "protocol": text(row, "ingest_protocol"),
                "stream_key_hint": text(row, "stream_key_hint")
            })),
            "target": target.as_ref().map(|row| json!({
                "id": text(row, "id"),
                "status": text(row, "status"),
                "protocol": text(row, "protocol"),
                "latency_profile": text(row, "latency_profile")
            })),
            "output": output.as_ref().map(|row| json!({
                "id": text(row, "id"),
                "status": text(row, "status"),
                "output_kind": text(row, "output_kind"),
                "health": row.get("health_json").cloned().unwrap_or_else(|| json!({}))
            })),
            "playback": readiness.as_ref().map(|row| json!({
                "id": text(row, "id"),
                "status": text(row, "status"),
                "playback_url": text(row, "playback_url")
            })),
            "telemetry": telemetry.as_ref().map(|row| json!({
                "id": text(row, "id"),
                "sample_kind": text(row, "sample_kind"),
                "bitrate_kbps": int(row, "bitrate_kbps"),
                "upload_mbps": row.get("upload_mbps").and_then(Value::as_f64).unwrap_or_default(),
                "ingest_latency_ms": int(row, "ingest_latency_ms"),
                "dropped_frames": int(row, "dropped_frames"),
                "reconnect_count": int(row, "reconnect_count"),
                "health": row.get("health_json").cloned().unwrap_or_else(|| json!({}))
            })),
            "payload": payload,
            "updated_at": now
        });
        sqlx::query(
            "INSERT INTO vanta_live_authoritative_bindings
            (id, creator_id, broadcast_id, obs_runtime_binding_id, live_ingest_session_id, external_broadcast_id, authority,
             status, version, binding_json, last_snapshot_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, 'vanta_live', ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET live_ingest_session_id = excluded.live_ingest_session_id,
            status = excluded.status, version = excluded.version, binding_json = excluded.binding_json,
            last_snapshot_json = excluded.last_snapshot_json, updated_at = excluded.updated_at",
        )
        .bind(&binding_id)
        .bind(text(&runtime, "creator_id"))
        .bind(broadcast_id)
        .bind(text(&runtime, "id"))
        .bind(&ingest_session_id)
        .bind(format!("vanta_live_{broadcast_id}"))
        .bind(status)
        .bind(version)
        .bind(binding_json.to_string())
        .bind(snapshot.to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "INSERT INTO vanta_live_authoritative_events
            (id, broadcast_id, binding_id, event_kind, status, payload_json, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(format!("vanta_live_event_{}", short_id()))
        .bind(broadcast_id)
        .bind(&binding_id)
        .bind(event_kind)
        .bind(status)
        .bind(snapshot.to_string())
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.row(
            "SELECT * FROM vanta_live_authoritative_bindings WHERE id = ?",
            &[&binding_id],
        )
        .await
    }

    pub async fn safety_state(&self, broadcast_id: &str) -> Result<Value, ObsStoreError> {
        let broadcast = self
            .row(
                "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
                &[broadcast_id],
            )
            .await?;
        let preflight = self
            .latest_preflight(broadcast_id)
            .await
            .unwrap_or_else(|_| json!({"ready":false,"blockers_json":["missing preflight"]}));
        let incidents = self.incidents(broadcast_id).await?;
        let open_incidents = incidents
            .iter()
            .filter(|incident| text(incident, "status") == "open")
            .cloned()
            .collect::<Vec<_>>();
        let support_bundles = self.support_bundles(broadcast_id).await?;
        let campaign_risk = optional_text(&broadcast, "sponsor_campaign_id").is_some();
        Ok(json!({
            "preflight_ready": preflight.get("ready").and_then(Value::as_i64).unwrap_or_default() == 1,
            "latest_incident": open_incidents.first().cloned().unwrap_or(Value::Null),
            "incident_count": open_incidents.len(),
            "resolved_incident_count": incidents.len().saturating_sub(open_incidents.len()),
            "latest_support_bundle": support_bundles.first().cloned().unwrap_or(Value::Null),
            "action_guards_json": {
                "roles": ["creator_owner", "producer", "live_ops"],
                "stream_end": {
                    "confirmation_text": "END STREAM",
                    "requires_campaign_recording_ack": campaign_risk
                },
                "recording_stop": {
                    "confirmation_text": "STOP RECORDING",
                    "requires_campaign_recording_ack": campaign_risk
                },
                "recording_discard": {
                    "confirmation_text": "DISCARD RECORDING",
                    "requires_campaign_recording_ack": campaign_risk
                },
                "force_end": {
                    "confirmation_text": "FORCE END",
                    "requires_campaign_recording_ack": campaign_risk
                },
                "safe_mode": {
                    "confirmation_text": Value::Null,
                    "requires_campaign_recording_ack": false
                }
            }
        }))
    }

    pub async fn create_support_bundle(&self, broadcast_id: &str) -> Result<Value, ObsStoreError> {
        let bundle_id = format!("support_bundle_{}", short_id());
        let now = now();
        let collection = self.active_collection().await?;
        let collection_id = text(&collection, "id");
        let bundle = json!({
            "broadcast": self.row("SELECT * FROM obs_broadcast_profiles WHERE id = ?", &[broadcast_id]).await?,
            "runtime": self.runtime(broadcast_id).await?,
            "health": self.health(broadcast_id).await?,
            "preflight": self.latest_preflight(broadcast_id).await.unwrap_or(Value::Null),
            "sources": self.sources().await?,
            "audio": self.audio_channels(broadcast_id).await?,
            "incidents": self.incidents(broadcast_id).await?,
            "events": self.events(broadcast_id).await?,
            "scene_collection_id": collection_id
        });
        sqlx::query(
            "INSERT INTO obs_support_bundles (id, broadcast_id, status, bundle_json, created_at)
            VALUES (?, ?, 'ready', ?, ?)",
        )
        .bind(&bundle_id)
        .bind(broadcast_id)
        .bind(bundle.to_string())
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.add_event(
            Some(broadcast_id),
            "support_bundle",
            "Support bundle exported",
        )
        .await?;
        self.row(
            "SELECT * FROM obs_support_bundles WHERE id = ?",
            &[&bundle_id],
        )
        .await
    }

    async fn record_incident(
        &self,
        broadcast_id: &str,
        incident_kind: &str,
        severity: &str,
        status: &str,
        operator_id: Option<&str>,
        reason: &str,
        holding_scene_id: Option<&str>,
        details_json: Value,
    ) -> Result<(), ObsStoreError> {
        let now = now();
        sqlx::query(
            "INSERT INTO obs_runtime_incidents
            (id, broadcast_id, incident_kind, severity, status, operator_id, reason, holding_scene_id, details_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(format!("incident_{}", short_id()))
        .bind(broadcast_id)
        .bind(incident_kind)
        .bind(severity)
        .bind(status)
        .bind(operator_id)
        .bind(reason)
        .bind(holding_scene_id)
        .bind(details_json.to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn post_show(&self, broadcast_id: &str) -> Result<Value, ObsStoreError> {
        if let Some(row) = self
            .row_optional(
                "SELECT * FROM obs_post_show_packages WHERE broadcast_id = ?",
                &[broadcast_id],
            )
            .await?
        {
            return Ok(row);
        }
        Ok(
            json!({"status":"not_started","saved_replays":self.replays(broadcast_id).await?.len(),"sponsor_proofs":0}),
        )
    }

    pub async fn send_to_editor(&self, broadcast_id: &str) -> Result<Value, ObsStoreError> {
        self.mark_post_show_sent_to_editor(broadcast_id).await?;
        self.add_event(
            Some(broadcast_id),
            "editor_handoff",
            "Archive package sent to Vanta Editor",
        )
        .await?;
        self.post_show(broadcast_id).await
    }

    pub(super) async fn create_broadcast_with_id(
        &self,
        broadcast_id: &str,
        input: BroadcastInput,
    ) -> Result<(), ObsStoreError> {
        let now = now();
        sqlx::query(
            "INSERT INTO obs_broadcast_profiles
            (id, creator_id, title, category, tags_json, thumbnail, mature_content, language, scheduled_start, visibility, follower_notification, chat_mode, recording_policy, archive_policy, latency_profile, output_quality_target, sponsor_campaign_id, collaboration_settings_json, status, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, ?, ?, ?, 0, 'en', ?, ?, 1, 'subscriber_slow_mode', ?, ?, ?, '1080p30', ?, ?, 'scheduled', ?, ?)",
        )
        .bind(broadcast_id)
        .bind(input.title)
        .bind(input.category)
        .bind(json!(["launch", "live", "sponsor"]).to_string())
        .bind("https://images.unsplash.com/photo-1495567720989-cebdbdd97913?auto=format&fit=crop&w=1600&q=80")
        .bind(input.scheduled_start)
        .bind(input.visibility)
        .bind(input.recording_policy)
        .bind(input.archive_policy)
        .bind(input.latency_profile)
        .bind(input.sponsor_campaign_id)
        .bind(json!({"guest_room":true,"isolated_audio":true,"mirror_pickups":true}).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(super) async fn create_instance_raw(
        &self,
        scene_id: &str,
        source_id: &str,
        order_index: i64,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        opacity: f64,
    ) -> Result<Value, ObsStoreError> {
        let instance = id();
        let now = now();
        sqlx::query(
            "INSERT INTO obs_source_instances
            (id, scene_id, source_id, order_index, visible, locked, x, y, width, height, crop_json, transform_json, opacity, settings_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, 1, 0, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&instance)
        .bind(scene_id)
        .bind(source_id)
        .bind(order_index)
        .bind(x)
        .bind(y)
        .bind(width)
        .bind(height)
        .bind(json!({"top":0,"right":0,"bottom":0,"left":0}).to_string())
        .bind(json!({"fit":"contain","rotation":0}).to_string())
        .bind(opacity)
        .bind(json!({}).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.row(
            "SELECT * FROM obs_source_instances WHERE id = ?",
            &[&instance],
        )
        .await
    }

    pub(super) async fn create_cue_for_broadcast(
        &self,
        broadcast_id: &str,
        input: CueInput,
    ) -> Result<Value, ObsStoreError> {
        let cue = id();
        let now = now();
        sqlx::query(
            "INSERT INTO obs_live_cues
            (id, creator_id, broadcast_id, campaign_id, offer_id, cue_kind, label, scheduled_at_seconds, required_duration_seconds, status, scene_id, source_id, proof_marker_id, requirements_json, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, ?, NULL, ?, ?, ?, ?, 'ready', ?, ?, NULL, ?, ?, ?)",
        )
        .bind(&cue)
        .bind(broadcast_id)
        .bind(input.campaign_id)
        .bind(input.cue_kind)
        .bind(input.label)
        .bind(input.scheduled_at_seconds)
        .bind(input.required_duration_seconds)
        .bind(input.scene_id)
        .bind(input.source_id)
        .bind(input.requirements_json.unwrap_or_else(|| json!({})).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.row("SELECT * FROM obs_live_cues WHERE id = ?", &[&cue])
            .await
    }
}

fn stable_hash(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn guest_can_be_active_speaker(participant: &Value) -> bool {
    text(participant, "status") != "removed"
        && int(participant, "muted") == 0
        && int(participant, "safety_disabled") == 0
}

fn guest_active_speaker_score(participant: &Value, input: &GuestMediaTelemetryInput) -> f64 {
    if !guest_can_be_active_speaker(participant) || !input.speaking {
        return 0.0;
    }
    let level_score = (input.audio_level_db + 80.0).max(0.0);
    if level_score < 25.0 {
        return 0.0;
    }
    let latency_penalty = (input.round_trip_ms.max(0) as f64 / 100.0).min(4.0);
    let packet_penalty = input.packet_loss_percent.max(0.0).min(10.0);
    let jitter_penalty = (input.jitter_ms.unwrap_or_default().max(0) as f64 / 80.0).min(2.0);
    (level_score - latency_penalty - packet_penalty - jitter_penalty).max(0.0)
}

fn guest_long_session_health(
    previous_sample_count: i64,
    previous_dropped_frames: i64,
    previous_max_round_trip_ms: i64,
    previous_max_jitter_ms: i64,
    previous_max_packet_loss_percent: f64,
    input: &GuestMediaTelemetryInput,
) -> Value {
    let sample_count = previous_sample_count + 1;
    let current_dropped_frames = input.dropped_frames.unwrap_or_default();
    let cumulative_dropped_frames = previous_dropped_frames + current_dropped_frames;
    let max_round_trip_ms = previous_max_round_trip_ms.max(input.round_trip_ms);
    let max_jitter_ms = previous_max_jitter_ms.max(input.jitter_ms.unwrap_or_default());
    let max_packet_loss_percent = previous_max_packet_loss_percent.max(input.packet_loss_percent);
    let status = if input.round_trip_ms > 900
        || input.packet_loss_percent > 8.0
        || input.jitter_ms.unwrap_or_default() > 160
        || current_dropped_frames > 120
    {
        "degrading"
    } else if input.round_trip_ms > 450
        || input.packet_loss_percent > 3.0
        || input.jitter_ms.unwrap_or_default() > 80
        || current_dropped_frames > 30
        || cumulative_dropped_frames > 120
    {
        "watch"
    } else {
        "stable"
    };
    json!({
        "sample_count": sample_count,
        "status": status,
        "cumulative_dropped_frames": cumulative_dropped_frames,
        "current_dropped_frames": current_dropped_frames,
        "max_round_trip_ms": max_round_trip_ms,
        "current_round_trip_ms": input.round_trip_ms,
        "max_jitter_ms": max_jitter_ms,
        "current_jitter_ms": input.jitter_ms.unwrap_or_default(),
        "max_packet_loss_percent": max_packet_loss_percent,
        "current_packet_loss_percent": input.packet_loss_percent,
        "degradation_action": if status == "degrading" {
            "reduce_guest_video_layer_keep_audio_and_mix_minus"
        } else if status == "watch" {
            "prepare_guest_layer_downshift"
        } else {
            "maintain_guest_quality"
        },
        "protect_host_program": true,
        "protect_audio_continuity": true
    })
}

fn guest_media_state(participant: &Value, telemetry: &Value) -> Value {
    json!({
        "speaking": telemetry.get("speaking").and_then(Value::as_bool).unwrap_or(false),
        "active_speaker": false,
        "audio_level_db": telemetry.get("audio_level_db").and_then(Value::as_f64).unwrap_or(-80.0),
        "video_active": telemetry.get("video_active").and_then(Value::as_bool).unwrap_or(false),
        "round_trip_ms": telemetry.get("round_trip_ms").and_then(Value::as_i64).unwrap_or_default(),
        "packet_loss_percent": telemetry.get("packet_loss_percent").and_then(Value::as_f64).unwrap_or_default(),
        "jitter_ms": telemetry.get("jitter_ms").and_then(Value::as_i64).unwrap_or_default(),
        "dropped_frames": telemetry.get("dropped_frames").and_then(Value::as_i64).unwrap_or_default(),
        "long_session": telemetry.get("long_session").cloned().unwrap_or_else(|| json!({})),
        "score": telemetry.get("active_speaker_score").and_then(Value::as_f64).unwrap_or_default(),
        "eligible": guest_can_be_active_speaker(participant),
        "sampled_at": telemetry.get("sampled_at").and_then(Value::as_str).unwrap_or_default()
    })
}

fn guest_room_default_capacity(mode: &str) -> i64 {
    match mode {
        "dual" => 2,
        "group" | "shared_game" => 8,
        _ => 1,
    }
}

fn guest_room_layout_policy(mode: &str, max_participants: i64) -> Value {
    match mode {
        "dual" => {
            json!({"layout":"side_by_side","visible_speakers":2,"active_speaker_priority":false})
        }
        "shared_game" => {
            json!({"layout":"shared_feed_primary","visible_speakers":max_participants.min(4),"active_speaker_priority":true})
        }
        "group" => {
            json!({"layout":"adaptive_grid","visible_speakers":max_participants.min(8),"active_speaker_priority":true})
        }
        _ => json!({"layout":"host_primary","visible_speakers":1,"active_speaker_priority":false}),
    }
}

fn guest_room_simulcast_layers(max_participants: i64) -> Value {
    if max_participants <= 2 {
        json!(["720p30", "360p30"])
    } else if max_participants <= 4 {
        json!(["720p30", "480p30", "180p15"])
    } else {
        json!(["720p30", "360p30", "180p15"])
    }
}

fn guest_room_media_transport_plan(
    mode: &str,
    max_participants: i64,
    latency_target_ms: i64,
    shared_feed_source_id: &str,
    shared_source: Option<&Value>,
    mirrored_channels: bool,
    active_speaker: Value,
    participants: &[Value],
) -> Value {
    let active_participant_id = active_speaker
        .get("participant_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let active_participants: Vec<&Value> = participants
        .iter()
        .filter(|participant| text(participant, "status") != "removed")
        .collect();
    let layers = guest_room_simulcast_layers(max_participants);
    let participant_plans: Vec<Value> = active_participants
        .iter()
        .map(|participant| {
            let participant_id = text(participant, "id");
            let connection = participant
                .get("connection_health_json")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let receive_participants: Vec<Value> = active_participants
                .iter()
                .filter(|other| text(other, "id") != participant_id)
                .map(|other| {
                    json!({
                        "participant_id": text(other, "id"),
                        "source_id": optional_text(other, "source_id"),
                        "preferred_layer": if text(other, "id") == active_participant_id { "720p30" } else { guest_recommended_layer(other) },
                        "active_speaker_priority": text(other, "id") == active_participant_id
                    })
                })
                .collect();
            json!({
                "participant_id": participant_id,
                "source_id": optional_text(participant, "source_id"),
                "status": text(participant, "status"),
                "publish": {
                    "audio": !matches!(int(participant, "muted"), 1),
                    "audio_codec": "opus",
                    "video": !matches!(int(participant, "safety_disabled"), 1),
                    "video_codec": "h264",
                    "layers": layers.clone()
                },
                "receive": {
                    "program": true,
                    "shared_feed": mode == "shared_game" && !shared_feed_source_id.is_empty(),
                    "participants": receive_participants,
                    "mix_minus": true,
                    "mirrored_channel": mirrored_channels
                },
                "degradation": guest_connection_degradation(&connection),
                "active_speaker": participant_id == active_participant_id
            })
        })
        .collect();
    json!({
        "transport": "selective_forwarding",
        "controller": "vanta_realtime_sfu",
        "room_mode": mode,
        "target_latency_ms": latency_target_ms,
        "max_participants": max_participants,
        "forwarded_stream_count": active_participants.len() as i64
            + if mode == "shared_game" && !shared_feed_source_id.is_empty() { 2 } else { 1 },
        "shared_feed": {
            "enabled": mode == "shared_game" && !shared_feed_source_id.is_empty(),
            "source_id": if shared_feed_source_id.is_empty() { Value::Null } else { json!(shared_feed_source_id) },
            "source_kind": shared_source.map_or(Value::Null, |source| json!(text(source, "source_kind"))),
            "priority": "host_program_adjacent",
            "layer": "1080p60"
        },
        "active_speaker": active_speaker,
        "participant_plans": participant_plans,
        "degradation": {
            "order": [
                "drop_inactive_guest_to_180p15",
                "drop_non_speaker_guest_video",
                "preserve_audio_and_host_program",
                "hold_shared_feed_at_720p30"
            ],
            "weak_guest_policy": "reduce_guest_layer_before_host_program",
            "host_program_protected": true
        },
        "recording_hooks": {
            "isolated_per_participant": true,
            "track_manifest_source": "obs_guest_isolated_recordings"
        }
    })
}

fn guest_return_feed_transport_plan(
    mode: &str,
    latency_target_ms: i64,
    shared_feed_source_id: &str,
    participant: Option<&Value>,
) -> Value {
    let participant_layer = participant.map(guest_recommended_layer).unwrap_or("720p30");
    json!({
        "transport": "selective_forwarding",
        "controller": "vanta_realtime_sfu",
        "receive_policy": if mode == "shared_game" && !shared_feed_source_id.is_empty() {
            "program_plus_shared_feed"
        } else {
            "program_return"
        },
        "audio": {
            "mode": "mix_minus",
            "codec": "opus",
            "priority": "continuity"
        },
        "video": {
            "program_layer": "720p30",
            "shared_feed_layer": if mode == "shared_game" && !shared_feed_source_id.is_empty() { "1080p60" } else { "off" },
            "participant_layer": participant_layer
        },
        "latency_target_ms": latency_target_ms,
        "degradation_order": [
            "reduce_participant_layer",
            "freeze_shared_feed_frame_before_audio_drop",
            "audio_never_below_opus_24kbps"
        ]
    })
}

fn guest_recommended_layer(participant: &Value) -> &'static str {
    let health = participant
        .get("connection_health_json")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let recommended = health
        .get("recommended_layer")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match recommended {
        "audio_only" | "180p15" => "180p15",
        "360p30" => "360p30",
        "480p30" => "480p30",
        "720p30" => "720p30",
        _ => "720p30",
    }
}

fn guest_connection_degradation(connection: &Value) -> Value {
    let status = connection
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let recommended_layer = connection
        .get("recommended_layer")
        .and_then(Value::as_str)
        .unwrap_or("720p30");
    json!({
        "status": status,
        "recommended_layer": recommended_layer,
        "packet_loss_threshold_percent": 5.0,
        "round_trip_threshold_ms": 220,
        "action": if matches!(recommended_layer, "audio_only" | "180p15") {
            "protect_audio"
        } else if status == "warning" {
            "hold_or_reduce_layer"
        } else {
            "maintain"
        }
    })
}

struct GuestRtpPacket {
    marker: bool,
    payload_type: u8,
    sequence_number: u16,
    timestamp: u32,
    ssrc: u32,
    payload_bytes: usize,
    payload: Vec<u8>,
}

fn parse_guest_rtp_packet(bytes: &[u8]) -> Result<GuestRtpPacket, ObsStoreError> {
    if bytes.len() < 12 {
        return Err(ObsStoreError::Invalid(
            "RTP packet must contain a 12-byte header".to_string(),
        ));
    }
    let version = bytes[0] >> 6;
    if version != 2 {
        return Err(ObsStoreError::Invalid(
            "RTP packet must use version 2".to_string(),
        ));
    }
    let csrc_count = (bytes[0] & 0x0f) as usize;
    let extension = (bytes[0] & 0x10) != 0;
    let padding = (bytes[0] & 0x20) != 0;
    let mut header_len = 12 + csrc_count * 4;
    if bytes.len() < header_len {
        return Err(ObsStoreError::Invalid(
            "RTP packet CSRC header is truncated".to_string(),
        ));
    }
    if extension {
        if bytes.len() < header_len + 4 {
            return Err(ObsStoreError::Invalid(
                "RTP extension header is truncated".to_string(),
            ));
        }
        let extension_words =
            u16::from_be_bytes([bytes[header_len + 2], bytes[header_len + 3]]) as usize;
        header_len += 4 + extension_words * 4;
    }
    if bytes.len() <= header_len {
        return Err(ObsStoreError::Invalid(
            "RTP packet payload is empty".to_string(),
        ));
    }
    let padding_len = if padding {
        *bytes.last().unwrap_or(&0) as usize
    } else {
        0
    };
    if padding_len >= bytes.len().saturating_sub(header_len) {
        return Err(ObsStoreError::Invalid(
            "RTP packet padding exceeds payload".to_string(),
        ));
    }
    let payload_end = bytes.len() - padding_len;
    let payload = bytes[header_len..payload_end].to_vec();
    Ok(GuestRtpPacket {
        marker: (bytes[1] & 0x80) != 0,
        payload_type: bytes[1] & 0x7f,
        sequence_number: u16::from_be_bytes([bytes[2], bytes[3]]),
        timestamp: u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        ssrc: u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
        payload_bytes: payload.len(),
        payload,
    })
}

fn rtp_sequence_gap(previous: u16, current: u16) -> i64 {
    let expected = previous.wrapping_add(1);
    if current == expected {
        return 0;
    }
    let gap = current.wrapping_sub(expected);
    if gap > 0 && gap < 3000 { gap as i64 } else { 0 }
}

fn rtp_packet_order(previous: Option<u16>, current: u16) -> &'static str {
    let Some(previous) = previous else {
        return "first";
    };
    if current == previous.wrapping_add(1) {
        return "in_order";
    }
    let forward = current.wrapping_sub(previous);
    if forward > 1 && forward < 3000 {
        "gap"
    } else {
        "out_of_order"
    }
}

fn rtp_clock_rate(payload_kind: &str, metadata: Option<&Value>) -> i64 {
    metadata
        .and_then(|value| value.get("clock_rate"))
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
        .unwrap_or_else(|| {
            if payload_kind == "audio" {
                48_000
            } else {
                90_000
            }
        })
}

fn rtp_payload_codec(payload_kind: &str, metadata: Option<&Value>) -> String {
    metadata
        .and_then(|value| value.get("codec"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_else(|| {
            if payload_kind == "audio" {
                "opus".to_string()
            } else {
                "h264".to_string()
            }
        })
}

fn media_worker_state(
    previous: Option<&Value>,
    payload_kind: &str,
    codec: &str,
    packet: &GuestRtpPacket,
    received_at_ms: i64,
    clock_rate: i64,
    packet_order: &str,
    dropped_since_last: i64,
) -> Value {
    let previous_jitter = previous
        .and_then(|worker| worker.get("jitter_ms"))
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let previous_transit = previous
        .and_then(|worker| worker.get("last_transit_ms"))
        .and_then(Value::as_f64);
    let rtp_media_time_ms = packet.timestamp as f64 * 1000.0 / clock_rate.max(1) as f64;
    let transit_ms = received_at_ms as f64 - rtp_media_time_ms;
    let jitter_ms = previous_transit
        .map(|last| previous_jitter + ((transit_ms - last).abs() - previous_jitter) / 16.0)
        .unwrap_or(0.0)
        .max(0.0);
    let reordered_packets = previous
        .and_then(|worker| worker.get("reordered_packets"))
        .and_then(Value::as_i64)
        .unwrap_or_default()
        + i64::from(packet_order == "out_of_order");
    let ready_frames = previous
        .and_then(|worker| worker.get("ready_frames"))
        .and_then(Value::as_i64)
        .unwrap_or_default()
        + i64::from(packet.marker);
    let packet_count = previous
        .and_then(|worker| worker.get("packet_count"))
        .and_then(Value::as_i64)
        .unwrap_or_default()
        + 1;
    let target_buffer_ms = if packet_order == "out_of_order" || jitter_ms > 45.0 {
        140
    } else if jitter_ms > 20.0 || dropped_since_last > 0 {
        110
    } else {
        70
    };
    json!({
        "status": if packet_order == "out_of_order" { "reordering" } else { "locked" },
        "stage": "rtp_jitter_buffer",
        "payload_kind": payload_kind,
        "codec": codec,
        "clock_rate": clock_rate,
        "packet_count": packet_count,
        "ready_frames": ready_frames,
        "reordered_packets": reordered_packets,
        "last_packet_order": packet_order,
        "last_sequence_number": if packet_order == "out_of_order" {
            previous
                .and_then(|worker| worker.get("last_sequence_number"))
                .cloned()
                .unwrap_or_else(|| json!(packet.sequence_number))
        } else {
            json!(packet.sequence_number)
        },
        "last_rtp_timestamp": packet.timestamp,
        "last_transit_ms": transit_ms,
        "jitter_ms": (jitter_ms * 100.0).round() / 100.0,
        "target_buffer_ms": target_buffer_ms,
        "playout_policy": "low_latency_program_clock",
        "depacketizer": "marker_delimited_access_unit"
    })
}

fn rtp_access_unit_json(payload_kind: &str, media_worker: &Value, packets: &[Value]) -> Value {
    let codec =
        media_worker
            .get("codec")
            .and_then(Value::as_str)
            .unwrap_or(if payload_kind == "audio" {
                "opus"
            } else {
                "h264"
            });
    if payload_kind == "video" && codec.eq_ignore_ascii_case("h264") {
        return h264_access_unit_json(packets);
    }
    let payloads = packet_payloads(packets);
    let byte_length = payloads.iter().map(Vec::len).sum::<usize>();
    json!({
        "codec": codec,
        "format": "rtp_payload_sequence",
        "packet_count": payloads.len(),
        "byte_length": byte_length,
        "payloads_base64": payloads
            .iter()
            .map(|payload| general_purpose::STANDARD.encode(payload))
            .collect::<Vec<_>>(),
        "ready_for_decode": !payloads.is_empty()
    })
}

fn h264_access_unit_json(packets: &[Value]) -> Value {
    let mut annex_b = Vec::new();
    let mut warnings = Vec::new();
    let mut nal_units = Vec::new();
    let mut fu_started = false;
    let mut fu_sequence_start: Option<i64> = None;

    for packet in packets {
        let sequence_number = int(packet, "sequence_number");
        let Some(payload) = packet
            .get("packet_json")
            .and_then(|packet_json| packet_json.get("payload_base64"))
            .and_then(Value::as_str)
            .and_then(|payload| general_purpose::STANDARD.decode(payload).ok())
        else {
            warnings.push(format!("missing_payload:{sequence_number}"));
            continue;
        };
        if payload.is_empty() {
            warnings.push(format!("empty_payload:{sequence_number}"));
            continue;
        }
        let nal_type = payload[0] & 0x1f;
        match nal_type {
            1..=23 => {
                annex_b.extend_from_slice(&[0, 0, 0, 1]);
                annex_b.extend_from_slice(&payload);
                nal_units.push(json!({
                    "sequence_number": sequence_number,
                    "packetization": "single_nal",
                    "nal_type": nal_type,
                    "byte_length": payload.len()
                }));
            }
            24 => {
                let mut offset = 1;
                let mut stap_units = 0;
                while offset + 2 <= payload.len() {
                    let length =
                        u16::from_be_bytes([payload[offset], payload[offset + 1]]) as usize;
                    offset += 2;
                    if length == 0 || offset + length > payload.len() {
                        warnings.push(format!("invalid_stap_a:{sequence_number}"));
                        break;
                    }
                    annex_b.extend_from_slice(&[0, 0, 0, 1]);
                    annex_b.extend_from_slice(&payload[offset..offset + length]);
                    nal_units.push(json!({
                        "sequence_number": sequence_number,
                        "packetization": "stap_a",
                        "nal_type": payload[offset] & 0x1f,
                        "byte_length": length
                    }));
                    stap_units += 1;
                    offset += length;
                }
                if stap_units == 0 {
                    warnings.push(format!("empty_stap_a:{sequence_number}"));
                }
            }
            28 => {
                if payload.len() < 2 {
                    warnings.push(format!("invalid_fu_a:{sequence_number}"));
                    continue;
                }
                let fu_indicator = payload[0];
                let fu_header = payload[1];
                let start = (fu_header & 0x80) != 0;
                let end = (fu_header & 0x40) != 0;
                let reconstructed_header = (fu_indicator & 0xe0) | (fu_header & 0x1f);
                if start {
                    annex_b.extend_from_slice(&[0, 0, 0, 1, reconstructed_header]);
                    fu_started = true;
                    fu_sequence_start = Some(sequence_number);
                } else if !fu_started {
                    warnings.push(format!("fu_a_missing_start:{sequence_number}"));
                }
                annex_b.extend_from_slice(&payload[2..]);
                nal_units.push(json!({
                    "sequence_number": sequence_number,
                    "packetization": "fu_a",
                    "nal_type": fu_header & 0x1f,
                    "fragment_start": start,
                    "fragment_end": end,
                    "byte_length": payload.len().saturating_sub(2)
                }));
                if end {
                    fu_started = false;
                    fu_sequence_start = None;
                }
            }
            _ => {
                warnings.push(format!(
                    "unsupported_h264_packetization:{sequence_number}:{nal_type}"
                ));
            }
        }
    }

    if let Some(sequence_number) = fu_sequence_start {
        warnings.push(format!("fu_a_missing_end:{sequence_number}"));
    }

    json!({
        "codec": "h264",
        "format": "h264_annex_b",
        "packet_count": packets.len(),
        "byte_length": annex_b.len(),
        "base64": general_purpose::STANDARD.encode(&annex_b),
        "nal_units": nal_units,
        "warnings": warnings,
        "ready_for_decode": !annex_b.is_empty() && !fu_started
    })
}

fn packet_payloads(packets: &[Value]) -> Vec<Vec<u8>> {
    packets
        .iter()
        .filter_map(|packet| {
            packet
                .get("packet_json")
                .and_then(|packet_json| packet_json.get("payload_base64"))
                .and_then(Value::as_str)
                .and_then(|payload| general_purpose::STANDARD.decode(payload).ok())
        })
        .collect()
}

fn guest_decoded_route_frames(
    relay: &Value,
    media_worker_frame_id: &str,
    decoded_id: &str,
    decoded_frame: &Value,
    payload_kind: &str,
    codec: &str,
    playout_at_ms: i64,
) -> Vec<Value> {
    let decode_ready = decoded_frame
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(|status| status == "decoded");
    let artifact_path = decoded_frame
        .get("artifact_path")
        .and_then(Value::as_str)
        .unwrap_or("");
    [
        (
            "program_composition",
            relay
                .pointer("/route_json/program_composition")
                .cloned()
                .unwrap_or_else(|| json!({})),
        ),
        (
            "return_feed",
            relay
                .pointer("/route_json/return_feed")
                .cloned()
                .unwrap_or_else(|| json!({})),
        ),
        (
            "archive",
            relay
                .pointer("/route_json/archive")
                .cloned()
                .unwrap_or_else(|| json!({})),
        ),
    ]
    .into_iter()
    .map(|(route_kind, route)| {
        let route_configured = route.as_object().is_some_and(|object| !object.is_empty());
        let status = if decode_ready && route_configured {
            "ready"
        } else if decode_ready {
            "unconfigured"
        } else {
            "waiting_for_decode"
        };
        json!({
            "id": format!("guest_route_frame_{}", short_id()),
            "decoded_media_frame_id": decoded_id,
            "media_worker_frame_id": media_worker_frame_id,
            "relay_id": text(relay, "id"),
            "participant_id": text(relay, "participant_id"),
            "broadcast_id": text(relay, "broadcast_id"),
            "route_kind": route_kind,
            "status": status,
            "payload_kind": payload_kind,
            "codec": codec,
            "artifact_path": artifact_path,
            "width": decoded_frame.get("width").cloned().unwrap_or_else(|| json!(0)),
            "height": decoded_frame.get("height").cloned().unwrap_or_else(|| json!(0)),
            "sample_rate": decoded_frame.get("sample_rate").cloned().unwrap_or_else(|| json!(0)),
            "channels": decoded_frame.get("channels").cloned().unwrap_or_else(|| json!(0)),
            "playout_at_ms": playout_at_ms,
            "route": route,
            "program_sync": {
                "clock": "program_clock",
                "sync_policy": "decoded_frame_playout",
                "playout_at_ms": playout_at_ms
            }
        })
    })
    .collect()
}

fn guest_media_sync_pair(
    relay: &Value,
    current: &Value,
    opposite: &Value,
    created_at: &str,
) -> Value {
    let current_kind = text(current, "payload_kind");
    let current_playout = int(current, "playout_at_ms");
    let opposite_playout = int(opposite, "playout_at_ms");
    let (audio, video) = if current_kind == "audio" {
        (current, opposite)
    } else {
        (opposite, current)
    };
    let audio_playout = int(audio, "playout_at_ms");
    let video_playout = int(video, "playout_at_ms");
    let drift_ms = audio_playout - video_playout;
    let abs_drift = drift_ms.abs();
    let sync_status = if abs_drift <= 40 {
        "locked"
    } else if abs_drift <= 100 {
        "drift_warning"
    } else {
        "resync_required"
    };
    let correction_action = if sync_status == "locked" {
        "play"
    } else if drift_ms > 0 {
        "delay_video_or_trim_audio_buffer"
    } else {
        "delay_audio_or_hold_video_frame"
    };
    json!({
        "id": format!("guest_sync_pair_{}", short_id()),
        "relay_id": text(relay, "id"),
        "participant_id": text(relay, "participant_id"),
        "broadcast_id": text(relay, "broadcast_id"),
        "route_kind": text(current, "route_kind"),
        "audio_route_frame_id": text(audio, "id"),
        "video_route_frame_id": text(video, "id"),
        "audio_decoded_media_frame_id": text(audio, "decoded_media_frame_id"),
        "video_decoded_media_frame_id": text(video, "decoded_media_frame_id"),
        "sync_status": sync_status,
        "drift_ms": drift_ms,
        "absolute_drift_ms": abs_drift,
        "audio_playout_at_ms": audio_playout,
        "video_playout_at_ms": video_playout,
        "current_playout_at_ms": current_playout,
        "opposite_playout_at_ms": opposite_playout,
        "sync_window_ms": 40,
        "resync_threshold_ms": 100,
        "correction_action": correction_action,
        "created_at": created_at,
        "program_sync": {
            "clock": "program_clock",
            "sync_policy": "audio_video_route_pair",
            "status": sync_status
        }
    })
}

async fn create_guest_program_compositor_frame(
    relay: &Value,
    sync_pair: &Value,
    current: &Value,
    opposite: &Value,
    created_at: &str,
) -> Result<Value, ObsStoreError> {
    let current_kind = text(current, "payload_kind");
    let (audio, video) = if current_kind == "audio" {
        (current, opposite)
    } else {
        (opposite, current)
    };
    let video_artifact_path = text(video, "artifact_path");
    let audio_artifact_path = text(audio, "artifact_path");
    if video_artifact_path.is_empty() {
        return Ok(json!({
            "id": format!("guest_compositor_frame_{}", short_id()),
            "relay_id": text(relay, "id"),
            "participant_id": text(relay, "participant_id"),
            "broadcast_id": text(relay, "broadcast_id"),
            "route_kind": "program_composition",
            "sync_pair_id": text(sync_pair, "id"),
            "status": "waiting_for_video_artifact",
            "reason": "missing_video_artifact_path",
            "created_at": created_at
        }));
    }

    let frame_id = format!("guest_compositor_frame_{}", short_id());
    let dir = guest_compositor_dir(&text(relay, "id"), &frame_id);
    fs::create_dir_all(&dir).await?;
    let artifact_path = dir.join("program-frame.png");
    let partial_path = dir.join("program-frame.partial.png");
    remove_store_file_if_exists(&artifact_path).await?;
    remove_store_file_if_exists(&partial_path).await?;

    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(&video_artifact_path)
        .arg("-vf")
        .arg("scale=w=1920:h=1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2:color=black,format=rgba")
        .arg("-frames:v")
        .arg("1")
        .arg("-f")
        .arg("image2")
        .arg(&partial_path)
        .output()
        .await;
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            return Ok(guest_program_compositor_failure(
                relay,
                sync_pair,
                video,
                audio,
                &frame_id,
                &artifact_path,
                created_at,
                "ffmpeg_unavailable",
                &error.to_string(),
            ));
        }
    };
    if !output.status.success() {
        remove_store_file_if_exists(&partial_path).await?;
        return Ok(guest_program_compositor_failure(
            relay,
            sync_pair,
            video,
            audio,
            &frame_id,
            &artifact_path,
            created_at,
            "ffmpeg_rejected_composition",
            String::from_utf8_lossy(&output.stderr).trim(),
        ));
    }

    fs::rename(&partial_path, &artifact_path).await?;
    let png = fs::read(&artifact_path).await?;
    let sha256 = Sha256::digest(&png);
    let (width, height) = png_dimensions(&png).unwrap_or((0, 0));
    Ok(json!({
        "id": frame_id,
        "relay_id": text(relay, "id"),
        "participant_id": text(relay, "participant_id"),
        "broadcast_id": text(relay, "broadcast_id"),
        "route_kind": "program_composition",
        "sync_pair_id": text(sync_pair, "id"),
        "audio_route_frame_id": text(sync_pair, "audio_route_frame_id"),
        "video_route_frame_id": text(sync_pair, "video_route_frame_id"),
        "status": "ready",
        "artifact_kind": "guest_program_compositor_png",
        "artifact_path": artifact_path,
        "source_video_artifact_path": video_artifact_path,
        "source_audio_artifact_path": audio_artifact_path,
        "byte_length": png.len(),
        "sha256": format!("{sha256:x}"),
        "width": width,
        "height": height,
        "sync": {
            "sync_status": text(sync_pair, "sync_status"),
            "drift_ms": int(sync_pair, "drift_ms"),
            "absolute_drift_ms": int(sync_pair, "absolute_drift_ms"),
            "correction_action": text(sync_pair, "correction_action")
        },
        "layout": {
            "canvas": {"width": 1920, "height": 1080, "pixel_format": "rgba"},
            "source_rect": {"x": 0, "y": 0, "width": int(video, "width"), "height": int(video, "height")},
            "output_rect": {"x": 0, "y": 0, "width": 1920, "height": 1080},
            "transform": {
                "fit": "contain",
                "crop": {"top": 0, "right": 0, "bottom": 0, "left": 0},
                "opacity": 1.0,
                "rotation_degrees": 0,
                "safe_area": true,
                "locked": false,
                "visible": true
            }
        },
        "compositor": {
            "engine": "ffmpeg_software_fallback",
            "output_surface": "program_composition",
            "frame_pacing": "program_clock",
            "dropped_frame_policy": "hold_last_good_frame_then_resync",
            "gpu_acceleration": false
        },
        "created_at": created_at
    }))
}

#[allow(clippy::too_many_arguments)]
fn guest_program_compositor_failure(
    relay: &Value,
    sync_pair: &Value,
    video: &Value,
    audio: &Value,
    frame_id: &str,
    artifact_path: &Path,
    created_at: &str,
    reason: &str,
    error: &str,
) -> Value {
    json!({
        "id": frame_id,
        "relay_id": text(relay, "id"),
        "participant_id": text(relay, "participant_id"),
        "broadcast_id": text(relay, "broadcast_id"),
        "route_kind": "program_composition",
        "sync_pair_id": text(sync_pair, "id"),
        "audio_route_frame_id": text(sync_pair, "audio_route_frame_id"),
        "video_route_frame_id": text(sync_pair, "video_route_frame_id"),
        "status": "compose_failed",
        "reason": reason,
        "error": error,
        "artifact_path": artifact_path,
        "source_video_artifact_path": text(video, "artifact_path"),
        "source_audio_artifact_path": text(audio, "artifact_path"),
        "created_at": created_at
    })
}

async fn create_runtime_gpu_playout_artifact(
    relay: &Value,
    compositor_frame: &Value,
    program_frame_sequence: i64,
) -> Result<Value, ObsStoreError> {
    let frame_path = text(compositor_frame, "artifact_path");
    let audio_path = text(compositor_frame, "source_audio_artifact_path");
    if frame_path.is_empty() || audio_path.is_empty() {
        return Ok(json!({
            "status": "unavailable",
            "reason": "missing_compositor_or_audio_artifact",
            "program_frame_sequence": program_frame_sequence
        }));
    }
    let frame_id = text(compositor_frame, "id");
    let dir = guest_compositor_dir(&text(relay, "id"), &frame_id).join("runtime-playout");
    fs::create_dir_all(&dir).await?;
    let artifact_path = dir.join(format!("program-playout-{program_frame_sequence:06}.mp4"));
    let partial_path = artifact_path.with_extension("partial.mp4");
    remove_store_file_if_exists(&artifact_path).await?;
    remove_store_file_if_exists(&partial_path).await?;

    let attempts = [
        (
            "h264_videotoolbox",
            true,
            "hardware_videotoolbox_low_latency",
        ),
        ("libx264", false, "software_x264_low_latency_fallback"),
    ];
    let mut errors = Vec::new();
    for (encoder, hardware_accelerated, profile) in attempts {
        remove_store_file_if_exists(&partial_path).await?;
        let mut command = Command::new("ffmpeg");
        command
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-y")
            .arg("-loop")
            .arg("1")
            .arg("-framerate")
            .arg("30")
            .arg("-t")
            .arg("1")
            .arg("-i")
            .arg(&frame_path)
            .arg("-i")
            .arg(&audio_path)
            .arg("-map")
            .arg("0:v:0")
            .arg("-map")
            .arg("1:a:0")
            .arg("-t")
            .arg("1")
            .arg("-shortest")
            .arg("-c:v")
            .arg(encoder);
        if encoder == "libx264" {
            command
                .arg("-preset")
                .arg("veryfast")
                .arg("-tune")
                .arg("zerolatency");
        } else {
            command.arg("-realtime").arg("1").arg("-allow_sw").arg("0");
        }
        let output = command
            .arg("-pix_fmt")
            .arg("yuv420p")
            .arg("-r")
            .arg("30")
            .arg("-g")
            .arg("30")
            .arg("-c:a")
            .arg("aac")
            .arg("-b:a")
            .arg("128k")
            .arg("-af")
            .arg("aresample=async=1000:first_pts=0")
            .arg("-movflags")
            .arg("+frag_keyframe+empty_moov+default_base_moof")
            .arg(&partial_path)
            .output()
            .await;
        let output = match output {
            Ok(output) => output,
            Err(error) => {
                errors.push(json!({
                    "encoder": encoder,
                    "profile": profile,
                    "error": error.to_string()
                }));
                continue;
            }
        };
        if !output.status.success() {
            errors.push(json!({
                "encoder": encoder,
                "profile": profile,
                "status": output.status.to_string(),
                "error": String::from_utf8_lossy(&output.stderr).trim()
            }));
            continue;
        }
        let validation = validate_runtime_playout_artifact(&partial_path).await;
        let validation = match validation {
            Ok(validation) => validation,
            Err(error) => {
                errors.push(json!({
                    "encoder": encoder,
                    "profile": profile,
                    "error": error.to_string()
                }));
                continue;
            }
        };
        fs::rename(&partial_path, &artifact_path).await?;
        return Ok(json!({
            "status": "ready",
            "artifact_kind": "guest_runtime_program_playout_mp4",
            "artifact_path": artifact_path,
            "program_frame_sequence": program_frame_sequence,
            "compositor_frame_id": frame_id,
            "source_frame_path": frame_path,
            "source_audio_path": audio_path,
            "encoder": {
                "name": encoder,
                "profile": profile,
                "hardware_accelerated": hardware_accelerated,
                "low_latency": true
            },
            "transport_contract": {
                "target": "vanta_realtime_sfu",
                "chunk_duration_ms": 1000,
                "fragmented_mp4": true,
                "program_clock_paced": true
            },
            "validation": validation
        }));
    }
    remove_store_file_if_exists(&partial_path).await?;
    Ok(json!({
        "status": "failed",
        "artifact_kind": "guest_runtime_program_playout_mp4",
        "program_frame_sequence": program_frame_sequence,
        "compositor_frame_id": frame_id,
        "source_frame_path": frame_path,
        "source_audio_path": audio_path,
        "attempts": errors
    }))
}

async fn validate_runtime_playout_artifact(path: &Path) -> Result<Value, ObsStoreError> {
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
        .await?;
    if !output.status.success() {
        return Err(ObsStoreError::Invalid(format!(
            "ffprobe runtime playout exited with status {}: {}",
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
            ObsStoreError::Invalid("runtime playout chunk has no video stream".to_string())
        })?;
    let audio = streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("audio"))
        .ok_or_else(|| {
            ObsStoreError::Invalid("runtime playout chunk has no audio stream".to_string())
        })?;
    let bytes = fs::read(path).await?;
    let sha256 = Sha256::digest(&bytes);
    let probed_frames = video
        .get("nb_read_frames")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<i64>().ok())
        .or_else(|| {
            video
                .get("nb_frames")
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<i64>().ok())
        })
        .unwrap_or_default();
    let width = video
        .get("width")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let height = video
        .get("height")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let video_duration_seconds = video
        .get("duration")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or_default();
    let frames = if probed_frames > 0 {
        probed_frames
    } else if width > 0 && height > 0 && video_duration_seconds > 0.0 {
        1
    } else {
        0
    };
    if width <= 0 || height <= 0 || frames <= 0 {
        return Err(ObsStoreError::Invalid(format!(
            "runtime playout validation failed: {width}x{height}, {frames} frames"
        )));
    }
    Ok(json!({
        "playable": true,
        "format": "mp4",
        "fragmented_mp4": true,
        "width": width,
        "height": height,
        "observed_video_frames": frames,
        "video_codec": video.get("codec_name").cloned().unwrap_or_else(|| json!("unknown")),
        "audio_codec": audio.get("codec_name").cloned().unwrap_or_else(|| json!("unknown")),
        "audio_sample_rate": audio.get("sample_rate").cloned().unwrap_or_else(|| json!("unknown")),
        "duration_seconds": probed.pointer("/format/duration").cloned().unwrap_or_else(|| json!("unknown")),
        "byte_length": bytes.len(),
        "sha256": format!("{sha256:x}")
    }))
}

async fn decode_h264_access_unit_artifact(
    relay_id: &str,
    frame_id: &str,
    access_unit: &Value,
) -> Result<Value, ObsStoreError> {
    let Some(base64) = access_unit.get("base64").and_then(Value::as_str) else {
        return Ok(json!({
            "status": "decode_failed",
            "decodeable": false,
            "reason": "missing_access_unit_base64"
        }));
    };
    let bytes = match general_purpose::STANDARD.decode(base64) {
        Ok(bytes) if !bytes.is_empty() => bytes,
        _ => {
            return Ok(json!({
                "status": "decode_failed",
                "decodeable": false,
                "reason": "invalid_access_unit_base64"
            }));
        }
    };
    let dir = guest_decode_dir(relay_id, frame_id);
    fs::create_dir_all(&dir).await?;
    let input_path = dir.join("access-unit.h264");
    let artifact_path = dir.join("decoded-frame.png");
    let partial_path = dir.join("decoded-frame.partial.png");
    remove_store_file_if_exists(&artifact_path).await?;
    remove_store_file_if_exists(&partial_path).await?;
    fs::write(&input_path, &bytes).await?;

    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-f")
        .arg("h264")
        .arg("-i")
        .arg(&input_path)
        .arg("-frames:v")
        .arg("1")
        .arg("-f")
        .arg("image2")
        .arg(&partial_path)
        .output()
        .await;
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            return Ok(json!({
                "status": "decode_failed",
                "decodeable": false,
                "reason": "ffmpeg_unavailable",
                "error": error.to_string(),
                "access_unit_path": input_path,
                "artifact_path": artifact_path
            }));
        }
    };
    if !output.status.success() {
        remove_store_file_if_exists(&partial_path).await?;
        return Ok(json!({
            "status": "decode_failed",
            "decodeable": false,
            "reason": "ffmpeg_rejected_access_unit",
            "error": String::from_utf8_lossy(&output.stderr).trim(),
            "access_unit_path": input_path,
            "artifact_path": artifact_path,
            "access_unit": {
                "format": access_unit.get("format").cloned().unwrap_or_else(|| json!("unknown")),
                "byte_length": bytes.len(),
                "packet_count": access_unit.get("packet_count").cloned().unwrap_or_else(|| json!(0))
            }
        }));
    }
    fs::rename(&partial_path, &artifact_path).await?;
    let png = fs::read(&artifact_path).await?;
    let sha256 = Sha256::digest(&png);
    let (width, height) = png_dimensions(&png).unwrap_or((0, 0));
    Ok(json!({
        "status": "decoded",
        "decodeable": true,
        "codec": "h264",
        "pixel_format": "rgba_or_native_png",
        "artifact_kind": "guest_decoded_video_png",
        "access_unit_path": input_path,
        "artifact_path": artifact_path,
        "byte_length": png.len(),
        "sha256": format!("{sha256:x}"),
        "width": width,
        "height": height,
        "decoder": {
            "engine": "ffmpeg",
            "input_format": "h264_annex_b",
            "output_format": "png",
            "frames_decoded": 1
        }
    }))
}

async fn decode_opus_access_unit_artifact(
    relay_id: &str,
    frame_id: &str,
    access_unit: &Value,
) -> Result<Value, ObsStoreError> {
    let payloads = access_unit
        .get("payloads_base64")
        .and_then(Value::as_array)
        .map(|payloads| {
            payloads
                .iter()
                .filter_map(|payload| {
                    payload
                        .as_str()
                        .and_then(|payload| general_purpose::STANDARD.decode(payload).ok())
                        .filter(|payload| !payload.is_empty())
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if payloads.is_empty() {
        return Ok(json!({
            "status": "decode_failed",
            "decodeable": false,
            "reason": "missing_opus_payloads"
        }));
    }
    let dir = guest_decode_dir(relay_id, frame_id);
    fs::create_dir_all(&dir).await?;
    let input_path = dir.join("access-unit.opus.ogg");
    let artifact_path = dir.join("decoded-audio.wav");
    let partial_path = dir.join("decoded-audio.partial.wav");
    remove_store_file_if_exists(&artifact_path).await?;
    remove_store_file_if_exists(&partial_path).await?;
    let ogg = ogg_opus_file(&payloads);
    fs::write(&input_path, &ogg).await?;

    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(&input_path)
        .arg("-ac")
        .arg("1")
        .arg("-ar")
        .arg("48000")
        .arg("-c:a")
        .arg("pcm_s16le")
        .arg(&partial_path)
        .output()
        .await;
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            return Ok(json!({
                "status": "decode_failed",
                "decodeable": false,
                "reason": "ffmpeg_unavailable",
                "error": error.to_string(),
                "access_unit_path": input_path,
                "artifact_path": artifact_path
            }));
        }
    };
    if !output.status.success() {
        remove_store_file_if_exists(&partial_path).await?;
        return Ok(json!({
            "status": "decode_failed",
            "decodeable": false,
            "reason": "ffmpeg_rejected_access_unit",
            "error": String::from_utf8_lossy(&output.stderr).trim(),
            "access_unit_path": input_path,
            "artifact_path": artifact_path,
            "access_unit": {
                "format": access_unit.get("format").cloned().unwrap_or_else(|| json!("unknown")),
                "byte_length": access_unit.get("byte_length").cloned().unwrap_or_else(|| json!(0)),
                "packet_count": payloads.len()
            }
        }));
    }
    fs::rename(&partial_path, &artifact_path).await?;
    let wav = fs::read(&artifact_path).await?;
    let sha256 = Sha256::digest(&wav);
    let audio = wav_metadata(&wav);
    Ok(json!({
        "status": "decoded",
        "decodeable": true,
        "codec": "opus",
        "sample_format": "pcm_s16le",
        "artifact_kind": "guest_decoded_audio_wav",
        "access_unit_path": input_path,
        "artifact_path": artifact_path,
        "byte_length": wav.len(),
        "sha256": format!("{sha256:x}"),
        "sample_rate": audio.get("sample_rate").cloned().unwrap_or_else(|| json!(48_000)),
        "channels": audio.get("channels").cloned().unwrap_or_else(|| json!(1)),
        "duration_samples": audio.get("duration_samples").cloned().unwrap_or_else(|| json!(0)),
        "decoder": {
            "engine": "ffmpeg",
            "input_format": "ogg_opus",
            "output_format": "wav_pcm_s16le",
            "frames_decoded": payloads.len()
        }
    }))
}

fn ogg_opus_file(payloads: &[Vec<u8>]) -> Vec<u8> {
    let serial = 0x564f4253;
    let mut sequence = 0;
    let mut file = Vec::new();
    let opus_head = opus_head_packet();
    file.extend(ogg_page(&[opus_head], serial, sequence, 0x02, 0));
    sequence += 1;
    file.extend(ogg_page(&[opus_tags_packet()], serial, sequence, 0x00, 0));
    sequence += 1;
    for (index, payload) in payloads.iter().enumerate() {
        let header_type = if index + 1 == payloads.len() {
            0x04
        } else {
            0x00
        };
        let granule_position = ((index as u64) + 1) * 960;
        file.extend(ogg_page(
            &[payload.clone()],
            serial,
            sequence,
            header_type,
            granule_position,
        ));
        sequence += 1;
    }
    file
}

fn opus_head_packet() -> Vec<u8> {
    let mut packet = Vec::new();
    packet.extend_from_slice(b"OpusHead");
    packet.push(1);
    packet.push(1);
    packet.extend_from_slice(&312u16.to_le_bytes());
    packet.extend_from_slice(&48_000u32.to_le_bytes());
    packet.extend_from_slice(&0i16.to_le_bytes());
    packet.push(0);
    packet
}

fn opus_tags_packet() -> Vec<u8> {
    let vendor = b"Vanta OBS";
    let mut packet = Vec::new();
    packet.extend_from_slice(b"OpusTags");
    packet.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    packet.extend_from_slice(vendor);
    packet.extend_from_slice(&0u32.to_le_bytes());
    packet
}

fn ogg_page(
    packets: &[Vec<u8>],
    serial: u32,
    sequence: u32,
    header_type: u8,
    granule_position: u64,
) -> Vec<u8> {
    let mut segment_table = Vec::new();
    let mut body = Vec::new();
    for packet in packets {
        let mut remaining = packet.len();
        let mut offset = 0;
        while remaining >= 255 {
            segment_table.push(255);
            body.extend_from_slice(&packet[offset..offset + 255]);
            offset += 255;
            remaining -= 255;
        }
        segment_table.push(remaining as u8);
        body.extend_from_slice(&packet[offset..]);
    }
    let mut page = Vec::new();
    page.extend_from_slice(b"OggS");
    page.push(0);
    page.push(header_type);
    page.extend_from_slice(&granule_position.to_le_bytes());
    page.extend_from_slice(&serial.to_le_bytes());
    page.extend_from_slice(&sequence.to_le_bytes());
    page.extend_from_slice(&0u32.to_le_bytes());
    page.push(segment_table.len() as u8);
    page.extend_from_slice(&segment_table);
    page.extend_from_slice(&body);
    let checksum = ogg_crc(&page);
    page[22..26].copy_from_slice(&checksum.to_le_bytes());
    page
}

fn ogg_crc(bytes: &[u8]) -> u32 {
    let mut crc = 0u32;
    for byte in bytes {
        crc ^= (*byte as u32) << 24;
        for _ in 0..8 {
            crc = if (crc & 0x8000_0000) != 0 {
                (crc << 1) ^ 0x04c1_1db7
            } else {
                crc << 1
            };
        }
    }
    crc
}

fn guest_decode_dir(relay_id: &str, frame_id: &str) -> PathBuf {
    let root = std::env::var("VANTA_OBS_MEDIA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("vanta-obs-media"));
    root.join("guest-media")
        .join(safe_path_segment(relay_id))
        .join(safe_path_segment(frame_id))
}

fn guest_compositor_dir(relay_id: &str, frame_id: &str) -> PathBuf {
    let root = std::env::var("VANTA_OBS_MEDIA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("vanta-obs-media"));
    root.join("guest-compositor")
        .join(safe_path_segment(relay_id))
        .join(safe_path_segment(frame_id))
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

async fn remove_store_file_if_exists(path: &Path) -> Result<(), ObsStoreError> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[0..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((width, height))
}

fn wav_metadata(bytes: &[u8]) -> Value {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return json!({});
    }
    let mut offset = 12;
    let mut sample_rate = 48_000u32;
    let mut channels = 1u16;
    let mut bits_per_sample = 16u16;
    let mut data_bytes = 0u32;
    while offset + 8 <= bytes.len() {
        let chunk_id = &bytes[offset..offset + 4];
        let chunk_size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
        let chunk_start = offset + 8;
        let chunk_end = chunk_start
            .saturating_add(chunk_size as usize)
            .min(bytes.len());
        if chunk_id == b"fmt " && chunk_end >= chunk_start + 16 {
            channels =
                u16::from_le_bytes(bytes[chunk_start + 2..chunk_start + 4].try_into().unwrap());
            sample_rate =
                u32::from_le_bytes(bytes[chunk_start + 4..chunk_start + 8].try_into().unwrap());
            bits_per_sample = u16::from_le_bytes(
                bytes[chunk_start + 14..chunk_start + 16]
                    .try_into()
                    .unwrap(),
            );
        } else if chunk_id == b"data" {
            data_bytes = chunk_size;
        }
        offset = chunk_end + (chunk_size as usize % 2);
    }
    let bytes_per_sample = (bits_per_sample as u32 / 8).max(1) * u32::from(channels.max(1));
    json!({
        "sample_rate": sample_rate,
        "channels": channels,
        "bits_per_sample": bits_per_sample,
        "duration_samples": data_bytes / bytes_per_sample
    })
}
