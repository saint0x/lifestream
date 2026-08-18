use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLiveSocketPresence {
    pub id: Id,
    pub creator_id: Id,
    pub user_id: Id,
    pub connected_at: String,
    pub last_seen_at: String,
    pub disconnected_at: Option<String>,
    pub is_stale: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLiveSocketPresenceReconciliationAction {
    pub action_type: String,
    pub target_id: Id,
    pub previous_state: Option<String>,
    pub next_state: Option<String>,
    pub reason: String,
    pub occurred_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLiveSocketPresenceReconciliationReport {
    pub creator_id: Id,
    pub socket_session_id: Id,
    pub reconciled_at: String,
    pub actions: Vec<CreatorLiveSocketPresenceReconciliationAction>,
    pub socket_session: CreatorLiveSocketPresence,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLiveControlResponse {
    pub snapshot: CreatorLiveSnapshot,
    pub settings: CreatorLiveSettings,
    pub health: CreatorLiveHealth,
    pub collaboration: CreatorLiveCollaborationSummary,
    pub subscriber_tiers: Vec<CreatorSubscriberTier>,
    pub is_live: bool,
    pub current_viewers: i64,
    pub bitrate_history: Vec<i64>,
    pub viewer_history: Vec<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveIngestEvent {
    pub id: Id,
    pub session_id: Id,
    pub creator_id: Id,
    pub broadcast_id: Id,
    pub event_type: String,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRuntimeOutput {
    pub id: Id,
    pub session_id: Id,
    pub creator_id: Id,
    pub broadcast_id: Id,
    pub runtime_state: String,
    pub packaging_status: String,
    pub archive_status: String,
    pub runtime_class: String,
    pub latency_profile: String,
    pub segment_format: String,
    pub partial_segments_enabled: bool,
    pub blocking_reload_enabled: bool,
    pub target_segment_duration_sec: i64,
    pub hold_back_segments: i64,
    pub discontinuity_sequence: i64,
    pub ladder_policy: String,
    pub content_class: String,
    pub manifest_relative_path: Option<String>,
    pub archive_relative_path: Option<String>,
    pub last_error: Option<String>,
    pub last_runtime_event_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRuntimeTarget {
    pub id: Id,
    pub session_id: Id,
    pub creator_id: Id,
    pub broadcast_id: Id,
    pub target_kind: String,
    pub target_key: String,
    pub target_label: String,
    pub route_state: String,
    pub target_creator_id: Option<Id>,
    pub target_broadcast_id: Option<Id>,
    pub playback_enabled: bool,
    pub recording_enabled: bool,
    pub mix_minus_required: bool,
    pub relative_path: Option<String>,
    pub source_participant_ids: Vec<Id>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRuntimeTelemetry {
    pub id: Id,
    pub session_id: Id,
    pub creator_id: Id,
    pub broadcast_id: Id,
    pub sample_kind: String,
    pub runtime_state: String,
    pub packaging_status: String,
    pub archive_status: String,
    pub bitrate_kbps: i64,
    pub viewers: i64,
    pub dropped_frames: i64,
    pub cpu_percent: Option<i64>,
    pub free_disk_gb: Option<f64>,
    pub detail: Value,
    pub collected_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRuntimeTelemetrySummary {
    pub total_samples: i64,
    pub degraded_samples: i64,
    pub packaging_degraded_samples: i64,
    pub failure_samples: i64,
    pub archive_failure_samples: i64,
    pub reconnect_events: i64,
    pub probe_samples: i64,
    pub validation_issue_samples: i64,
    pub repairable_validation_samples: i64,
    pub advisory_critical_samples: i64,
    pub advisory_repairable_samples: i64,
    pub runtime_artifact_reconciliation_samples: i64,
    pub runtime_archive_completion_samples: i64,
    pub artifact_attention_samples: i64,
    pub manifest_path_missing_samples: i64,
    pub archive_path_missing_samples: i64,
    pub collaboration_samples: i64,
    pub mix_minus_samples: i64,
    pub packaging_ready_samples: i64,
    pub archive_complete_samples: i64,
    pub avg_bitrate_kbps: Option<f64>,
    pub peak_bitrate_kbps: Option<i64>,
    pub avg_viewers: Option<f64>,
    pub peak_viewers: Option<i64>,
    pub total_dropped_frames: i64,
    pub peak_collaboration_participants: i64,
    pub peak_active_output_routes: i64,
    pub peak_runtime_target_count: i64,
    pub peak_playback_target_count: i64,
    pub peak_recording_target_count: i64,
    pub peak_variant_target_count: i64,
    pub peak_collaboration_target_count: i64,
    pub peak_program_target_count: i64,
    pub peak_audio_target_count: i64,
    pub peak_engine_target_count: i64,
    pub peak_host_channel_count: i64,
    pub peak_mirror_channel_count: i64,
    pub peak_shared_program_mirror_channel_count: i64,
    pub peak_guest_isolated_mirror_channel_count: i64,
    pub peak_archive_target_count: i64,
    pub peak_active_target_count: i64,
    pub peak_degraded_target_count: i64,
    pub peak_armed_target_count: i64,
    pub peak_pending_source_target_count: i64,
    pub ll_hls_samples: i64,
    pub peak_discontinuity_sequence: i64,
    pub last_collected_at: Option<String>,
    pub last_runtime_state: Option<String>,
    pub last_packaging_status: Option<String>,
    pub last_archive_status: Option<String>,
    pub last_contribution_state: Option<String>,
    pub last_ingest_latency_ms: Option<i64>,
    pub last_source_probe_present: bool,
    pub last_source_validation_state: Option<String>,
    pub last_advisory_status: Option<String>,
    pub last_manifest_artifact_state: Option<String>,
    pub last_archive_artifact_state: Option<String>,
    pub last_collaboration_session_id: Option<String>,
    pub last_collaboration_participant_count: Option<i64>,
    pub last_active_output_routes: Option<i64>,
    pub last_audio_mix_mode: Option<String>,
    pub last_runtime_target_count: Option<i64>,
    pub last_playback_target_count: Option<i64>,
    pub last_recording_target_count: Option<i64>,
    pub last_variant_target_count: Option<i64>,
    pub last_collaboration_target_count: Option<i64>,
    pub last_program_target_count: Option<i64>,
    pub last_audio_target_count: Option<i64>,
    pub last_engine_target_count: Option<i64>,
    pub last_host_channel_count: Option<i64>,
    pub last_mirror_channel_count: Option<i64>,
    pub last_shared_program_mirror_channel_count: Option<i64>,
    pub last_guest_isolated_mirror_channel_count: Option<i64>,
    pub last_archive_target_count: Option<i64>,
    pub last_active_target_count: Option<i64>,
    pub last_degraded_target_count: Option<i64>,
    pub last_armed_target_count: Option<i64>,
    pub last_pending_source_target_count: Option<i64>,
    pub last_runtime_class: Option<String>,
    pub last_latency_profile: Option<String>,
    pub last_ladder_policy: Option<String>,
    pub last_content_class: Option<String>,
    pub last_failure_at: Option<String>,
    pub last_failure_state: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRuntimeAdvisoryAction {
    pub code: String,
    pub severity: String,
    pub repairable: bool,
    pub title: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRuntimeArtifactState {
    pub expected_relative_path: Option<String>,
    pub persisted_relative_path: Option<String>,
    pub state: String,
    pub ready: bool,
    pub valid: bool,
    pub issue: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRuntimeArtifactHealth {
    pub status: String,
    pub checked_at: String,
    pub manifest: LiveRuntimeArtifactState,
    pub archive: LiveRuntimeArtifactState,
    pub issues: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRuntimeAdvisory {
    pub status: String,
    pub summary: String,
    pub requires_operator_action: bool,
    pub blocking_issue_count: i64,
    pub repairable_issue_count: i64,
    pub source_validation_state: Option<String>,
    pub runtime_failure_present: bool,
    pub recommended_actions: Vec<LiveRuntimeAdvisoryAction>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRuntimeRepairAction {
    pub field: String,
    pub previous_value: Option<String>,
    pub next_value: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRuntimeRepairReport {
    pub session_id: Id,
    pub creator_id: Id,
    pub broadcast_id: Id,
    pub actor_user_id: Id,
    pub actor_scope: String,
    pub reason: String,
    pub repaired_at: String,
    pub actions: Vec<LiveRuntimeRepairAction>,
    pub record: AdminLiveIngestSessionRecord,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminLiveIngestCreatorOverview {
    pub creator_id: Id,
    pub handle: String,
    pub display_name: String,
    pub active_sessions: i64,
    pub stale_sessions: i64,
    pub terminal_sessions: i64,
    pub ready_outputs: i64,
    pub degraded_outputs: i64,
    pub failed_outputs: i64,
    pub archive_finalizing_outputs: i64,
    pub archive_complete_outputs: i64,
    pub artifact_attention_outputs: i64,
    pub manifest_path_missing_outputs: i64,
    pub archive_path_missing_outputs: i64,
    pub last_runtime_state: Option<String>,
    pub last_packaging_status: Option<String>,
    pub last_archive_status: Option<String>,
    pub last_manifest_artifact_state: Option<String>,
    pub last_archive_artifact_state: Option<String>,
    pub avg_ready_latency_seconds: Option<f64>,
    pub avg_archive_completion_seconds: Option<f64>,
    pub total_samples: i64,
    pub degraded_samples: i64,
    pub failure_samples: i64,
    pub advisory_critical_samples: i64,
    pub advisory_repairable_samples: i64,
    pub runtime_artifact_reconciliation_samples: i64,
    pub runtime_archive_completion_samples: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminLiveIngestOverview {
    pub active_sessions: i64,
    pub stale_sessions: i64,
    pub terminal_sessions: i64,
    pub unique_creators: i64,
    pub ready_outputs: i64,
    pub degraded_outputs: i64,
    pub failed_outputs: i64,
    pub archive_finalizing_outputs: i64,
    pub archive_complete_outputs: i64,
    pub artifact_attention_outputs: i64,
    pub manifest_path_missing_outputs: i64,
    pub archive_path_missing_outputs: i64,
    pub avg_ready_latency_seconds: Option<f64>,
    pub avg_archive_completion_seconds: Option<f64>,
    pub total_samples: i64,
    pub degraded_samples: i64,
    pub failure_samples: i64,
    pub advisory_critical_samples: i64,
    pub advisory_repairable_samples: i64,
    pub runtime_artifact_reconciliation_samples: i64,
    pub runtime_archive_completion_samples: i64,
    pub peak_host_channel_count: i64,
    pub peak_mirror_channel_count: i64,
    pub peak_shared_program_mirror_channel_count: i64,
    pub peak_guest_isolated_mirror_channel_count: i64,
    pub peak_archive_target_count: i64,
    pub peak_active_target_count: i64,
    pub peak_degraded_target_count: i64,
    pub peak_armed_target_count: i64,
    pub peak_pending_source_target_count: i64,
    pub last_host_channel_count: Option<i64>,
    pub last_mirror_channel_count: Option<i64>,
    pub last_shared_program_mirror_channel_count: Option<i64>,
    pub last_guest_isolated_mirror_channel_count: Option<i64>,
    pub last_archive_target_count: Option<i64>,
    pub last_active_target_count: Option<i64>,
    pub last_degraded_target_count: Option<i64>,
    pub last_armed_target_count: Option<i64>,
    pub last_pending_source_target_count: Option<i64>,
    pub creator_breakdown: Vec<AdminLiveIngestCreatorOverview>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLiveRuntimeResponse {
    pub snapshot: CreatorLiveSnapshot,
    pub health: CreatorLiveHealth,
    pub collaboration: CreatorLiveCollaborationSummary,
    pub active_session: Option<LiveIngestSession>,
    pub active_runtime_output: Option<LiveRuntimeOutput>,
    pub active_runtime_targets: Vec<LiveRuntimeTarget>,
    pub telemetry_summary: LiveRuntimeTelemetrySummary,
    pub runtime_advisory: LiveRuntimeAdvisory,
    pub artifact_health: Option<LiveRuntimeArtifactHealth>,
    pub recent_sessions: Vec<LiveIngestSession>,
    pub recent_runtime_outputs: Vec<LiveRuntimeOutput>,
    pub recent_runtime_targets: Vec<LiveRuntimeTarget>,
    pub recent_telemetry: Vec<LiveRuntimeTelemetry>,
    pub recent_events: Vec<LiveIngestEvent>,
}
