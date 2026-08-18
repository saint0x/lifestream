use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorProfile {
    pub id: Id,
    pub user_id: Id,
    pub handle: String,
    pub display_name: String,
    pub avatar: String,
    pub banner: String,
    pub tagline: String,
    pub bio: String,
    pub partner_status: String,
    pub joined_at: String,
    pub stream_key: String,
    pub rtmp_url: String,
    pub default_category: String,
    pub default_tags: Vec<String>,
    pub followers: i64,
    pub subscribers: i64,
    pub monthly_viewers: i64,
    pub total_watch_hours: i64,
    pub live_status: String,
    pub current_broadcast_id: Option<Id>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Broadcast {
    pub id: Id,
    pub title: String,
    pub category: String,
    pub tags: Vec<String>,
    pub status: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_sec: Option<i64>,
    pub peak_viewers: i64,
    pub average_viewers: i64,
    pub chat_messages: i64,
    pub new_followers: i64,
    pub new_subscribers: i64,
    pub revenue: f64,
    pub thumbnail: String,
    pub is_mature: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSourceProbe {
    pub container_format: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub frame_rate: Option<f64>,
    pub audio_sample_rate_hz: Option<i64>,
    pub audio_channels: Option<i64>,
    pub probed_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSourceValidationIssue {
    pub code: String,
    pub message: String,
    pub severity: String,
    pub repairable: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSourceValidationReport {
    pub state: String,
    pub issues: Vec<LiveSourceValidationIssue>,
    pub validated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveIngestSession {
    pub id: Id,
    pub creator_id: Id,
    pub broadcast_id: Id,
    pub previous_session_id: Option<Id>,
    pub protocol: String,
    pub contribution_class: String,
    pub contribution_state: String,
    pub ingest_server: String,
    pub ingest_latency_ms: Option<i64>,
    pub source_probe: Option<LiveSourceProbe>,
    pub source_validation: Option<LiveSourceValidationReport>,
    pub status: String,
    pub bitrate_kbps: i64,
    pub viewers: i64,
    pub dropped_frames: i64,
    pub connected_at: String,
    pub last_heartbeat_at: String,
    pub disconnected_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminLiveIngestSessionRecord {
    pub session: LiveIngestSession,
    pub stale_connection: bool,
    pub runtime_output: Option<LiveRuntimeOutput>,
    pub runtime_targets: Vec<LiveRuntimeTarget>,
    pub telemetry_summary: LiveRuntimeTelemetrySummary,
    pub runtime_advisory: LiveRuntimeAdvisory,
    pub artifact_health: Option<LiveRuntimeArtifactHealth>,
    pub recent_telemetry: Vec<LiveRuntimeTelemetry>,
    pub recent_events: Vec<LiveIngestEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveIngestReconciliationAction {
    pub action_type: String,
    pub target_id: Id,
    pub previous_status: Option<String>,
    pub next_status: Option<String>,
    pub reason: String,
    pub occurred_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveIngestReconciliationReport {
    pub session_id: Id,
    pub reconciled_at: String,
    pub actions: Vec<LiveIngestReconciliationAction>,
    pub record: AdminLiveIngestSessionRecord,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLiveSnapshot {
    pub profile: CreatorProfile,
    pub current_broadcast: Option<Broadcast>,
    pub pending_broadcast: Option<Broadcast>,
    pub ingest_session: Option<LiveIngestSession>,
}
