use super::*;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLiveRequest {
    pub title: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub is_mature: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartBroadcastRequest {
    pub title: String,
    pub category: String,
    pub tags: Vec<String>,
    pub thumbnail: Option<String>,
    pub is_mature: bool,
    pub notify_followers: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveReportRequest {
    pub reason: String,
    pub details: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLiveModerationActionRequest {
    pub subject_user_id: Id,
    pub action_type: String,
    pub reason: String,
    pub duration_minutes: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveLiveStreamReportRequest {
    pub status: String,
    pub resolution_note: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSourceProbeInput {
    pub container_format: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub frame_rate: Option<f64>,
    pub audio_sample_rate_hz: Option<i64>,
    pub audio_channels: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestConnectRequest {
    pub stream_key: String,
    pub protocol: String,
    pub ingest_server: String,
    pub broadcast_id: Option<Id>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestConnectResponse {
    pub session: LiveIngestSession,
    pub ingest_token: String,
    pub live_stream_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestHeartbeatRequest {
    pub bitrate_kbps: i64,
    pub viewers: i64,
    pub dropped_frames: i64,
    pub cpu_percent: Option<i64>,
    pub free_disk_gb: Option<f64>,
    pub ingest_latency_ms: Option<i64>,
    pub source_probe: Option<LiveSourceProbeInput>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminateLiveIngestRequest {
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLiveRuntimeStateRequest {
    pub runtime_state: String,
    pub packaging_status: String,
    pub archive_status: String,
    pub manifest_relative_path: Option<String>,
    pub archive_relative_path: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairLiveRuntimeOutputRequest {
    pub reason: String,
    pub runtime_state: Option<String>,
    pub packaging_status: Option<String>,
    pub archive_status: Option<String>,
    pub manifest_relative_path: Option<String>,
    pub archive_relative_path: Option<String>,
    pub last_error: Option<String>,
    #[serde(default)]
    pub clear_manifest_relative_path: bool,
    #[serde(default)]
    pub clear_archive_relative_path: bool,
    #[serde(default)]
    pub clear_last_error: bool,
}
