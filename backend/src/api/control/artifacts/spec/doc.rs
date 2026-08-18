use super::*;
use crate::models::{
    CollaborationAudioRoute, CollaborationContributionAttachment, CollaborationExecutionPlan,
    CollaborationMediaLaunchRuntime, CollaborationMediaRuntime, CollaborationOutputRoute,
    CollaborationProgramRoute, CollaborationRuntimeBundle, CollaborationTopologyMember,
    LiveRuntimeAdvisory, LiveRuntimeArtifactHealth, LiveSourceProbe, LiveSourceValidationReport,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LiveRuntimeSpecDocument {
    pub(super) session: LiveRuntimeSpecSession,
    pub(super) runtime: LiveRuntimeSpecRuntime,
    pub(super) advisory: LiveRuntimeAdvisory,
    pub(super) artifact_health: LiveRuntimeArtifactHealth,
    pub(super) expected_paths: LiveRuntimeSpecPaths,
    pub(super) packaging: LiveRuntimePackagingSpec,
    pub(super) archive: LiveRuntimeArchiveSpec,
    pub(super) collaboration: Option<LiveRuntimeCollaborationSpec>,
    pub(super) reconnect_policy: LiveRuntimeReconnectSpec,
    pub(super) health: LiveRuntimeHealthSpec,
    pub(super) telemetry: LiveRuntimeTelemetrySpec,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LiveRuntimeSpecSession {
    pub(super) id: String,
    pub(super) creator_id: String,
    pub(super) broadcast_id: String,
    pub(super) previous_session_id: Option<String>,
    pub(super) protocol: String,
    pub(super) contribution_class: String,
    pub(super) contribution_state: String,
    pub(super) ingest_server: String,
    pub(super) status: String,
    pub(super) bitrate_kbps: i64,
    pub(super) viewers: i64,
    pub(super) dropped_frames: i64,
    pub(super) ingest_latency_ms: Option<i64>,
    pub(super) connected_at: String,
    pub(super) last_heartbeat_at: String,
    pub(super) disconnected_at: Option<String>,
    pub(super) session_ordinal: i64,
    pub(super) reconnect_session: bool,
    pub(super) source_probe: Option<LiveSourceProbe>,
    pub(super) source_validation: Option<LiveSourceValidationReport>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LiveRuntimeSpecRuntime {
    pub(super) state: String,
    pub(super) packaging_status: String,
    pub(super) archive_status: String,
    pub(super) runtime_class: String,
    pub(super) latency_profile: String,
    pub(super) segment_format: String,
    pub(super) partial_segments_enabled: bool,
    pub(super) blocking_reload_enabled: bool,
    pub(super) target_segment_duration_sec: i64,
    pub(super) hold_back_segments: i64,
    pub(super) discontinuity_sequence: i64,
    pub(super) ladder_policy: String,
    pub(super) content_class: String,
    pub(super) manifest_relative_path: Option<String>,
    pub(super) archive_relative_path: Option<String>,
    pub(super) last_error: Option<String>,
    pub(super) last_runtime_event_at: String,
    pub(super) updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LiveRuntimeSpecPaths {
    pub(super) manifest_relative_path: String,
    pub(super) archive_relative_path: String,
    pub(super) spec_relative_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LiveRuntimePackagingSpec {
    pub(super) runtime_class: String,
    pub(super) latency_profile: String,
    pub(super) playlist_mode: String,
    pub(super) segment_format: String,
    pub(super) segment_duration_sec: i64,
    pub(super) status: String,
    pub(super) master_manifest_relative_path: String,
    pub(super) output_root_relative_path: String,
    pub(super) live_edge_hold_back_segments: i64,
    pub(super) partial_segments_enabled: bool,
    pub(super) blocking_reload_enabled: bool,
    pub(super) target_latency_ms: i64,
    pub(super) variant_strategy: String,
    pub(super) ladder_policy: String,
    pub(super) content_class: String,
    pub(super) discontinuity_sequence: i64,
    pub(super) variants: Vec<LiveRuntimeVariantSpec>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LiveRuntimeArchiveSpec {
    pub(super) enabled: bool,
    pub(super) recording_mode: String,
    pub(super) target_container: String,
    pub(super) status: String,
    pub(super) staging_relative_path: String,
    pub(super) output_relative_path: String,
    pub(super) output_count: i64,
    pub(super) output_relative_paths: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LiveRuntimeCollaborationSpec {
    pub(super) session_id: String,
    pub(super) status: String,
    pub(super) source_broadcast_id: String,
    pub(super) chat_mode: String,
    pub(super) recording_policy: String,
    pub(super) shared_chat: bool,
    pub(super) mix_minus_required: bool,
    pub(super) audio_mix_mode: String,
    pub(super) connected_participants: i64,
    pub(super) recording_owner_creator_id: Option<String>,
    pub(super) host_output_participant_ids: Vec<String>,
    pub(super) mirrored_creator_ids: Vec<String>,
    pub(super) contributions: Vec<CollaborationContributionAttachment>,
    pub(super) outputs: Vec<CollaborationOutputRoute>,
    pub(super) programs: Vec<CollaborationProgramRoute>,
    pub(super) audio: Vec<CollaborationAudioRoute>,
    pub(super) engine: CollaborationExecutionPlan,
    pub(super) bundle: CollaborationRuntimeBundle,
    pub(super) media: CollaborationMediaRuntime,
    pub(super) launch: CollaborationMediaLaunchRuntime,
    pub(super) members: Vec<CollaborationTopologyMember>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LiveRuntimeReconnectSpec {
    pub(super) grace_window_sec: i64,
    pub(super) session_ordinal: i64,
    pub(super) replacement_mode: String,
    pub(super) requires_discontinuity_on_reconnect: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LiveRuntimeHealthSpec {
    pub(super) status: String,
    pub(super) current_cpu_percent: Option<i64>,
    pub(super) current_free_disk_gb: Option<f64>,
    pub(super) current_ingest_latency_ms: Option<i64>,
    pub(super) current_dropped_frames: i64,
    pub(super) cpu_warn_percent: i64,
    pub(super) cpu_critical_percent: i64,
    pub(super) free_disk_warn_gb: f64,
    pub(super) free_disk_critical_gb: f64,
    pub(super) ingest_latency_warn_ms: i64,
    pub(super) ingest_latency_critical_ms: i64,
    pub(super) dropped_frames_warn: i64,
    pub(super) dropped_frames_critical: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LiveRuntimeTelemetrySpec {
    pub(super) heartbeat_sample_kind: String,
    pub(super) runtime_report_sample_kind: String,
    pub(super) repair_sample_kind: String,
    pub(super) reconciliation_sample_kinds: Vec<String>,
}
