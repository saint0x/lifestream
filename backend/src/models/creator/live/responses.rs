use super::*;

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
