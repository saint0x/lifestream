use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadJob {
    pub id: Id,
    pub upload_id: Option<Id>,
    pub series_id: Option<Id>,
    pub kind: String,
    pub source_type: String,
    pub status: String,
    pub title: String,
    pub intended_visibility: String,
    pub bytes_expected: i64,
    pub bytes_received: i64,
    pub storage_key: String,
    pub created_at: String,
    pub updated_at: String,
    pub published_content_id: Option<Id>,
    pub mime_type: String,
    pub checksum_sha256: Option<String>,
    pub completed_at: Option<String>,
    pub processing_attempt_count: i64,
    pub last_processing_error: Option<String>,
    pub last_failed_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadIngestSession {
    pub job_id: Id,
    pub relative_path: String,
    pub status: String,
    pub mime_type: String,
    pub bytes_received: i64,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadIngestTicket {
    pub session: UploadIngestSession,
    pub upload_token: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaAssetVariant {
    pub id: Id,
    pub variant_type: String,
    pub label: String,
    pub relative_path: String,
    pub url: String,
    pub mime_type: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub bitrate_bps: Option<i64>,
    pub file_size_bytes: i64,
    pub is_default: bool,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaProcessingRun {
    pub id: Id,
    pub stage: String,
    pub status: String,
    pub details: serde_json::Value,
    pub started_at: String,
    pub completed_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaAsset {
    pub id: Id,
    pub upload_job_id: Id,
    pub upload_id: Option<Id>,
    pub series_id: Option<Id>,
    pub kind: String,
    pub title: String,
    pub status: String,
    pub visibility: String,
    pub source_path: String,
    pub source_url: String,
    pub poster_path: Option<String>,
    pub poster_url: Option<String>,
    pub playback_path: Option<String>,
    pub playback_url: Option<String>,
    pub mime_type: String,
    pub checksum_sha256: Option<String>,
    pub container_format: Option<String>,
    pub file_size_bytes: i64,
    pub duration_sec: f64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub frame_rate: Option<f64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub has_video: bool,
    pub has_audio: bool,
    pub created_at: String,
    pub updated_at: String,
    pub processed_at: Option<String>,
    pub published_content_id: Option<Id>,
    pub variants: Vec<MediaAssetVariant>,
    pub audio_tracks: Vec<PlaybackAudioTrack>,
    pub caption_tracks: Vec<PlaybackCaptionTrack>,
    pub preview_tracks: Vec<PlaybackPreviewTrack>,
    pub default_audio_track_id: Option<Id>,
    pub default_caption_track_id: Option<Id>,
    pub default_preview_track_id: Option<Id>,
    pub processing_runs: Vec<MediaProcessingRun>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminMediaJobRecord {
    pub creator_id: Id,
    pub upload_job: UploadJob,
    pub asset_status: Option<String>,
    pub processing_runs: Vec<MediaProcessingRun>,
    pub stale_processing: bool,
    pub repair_required: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaJobReconciliationAction {
    pub action_type: String,
    pub target_id: Id,
    pub previous_status: Option<String>,
    pub next_status: Option<String>,
    pub reason: String,
    pub occurred_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaJobReconciliationReport {
    pub job_id: Id,
    pub reconciled_at: String,
    pub actions: Vec<MediaJobReconciliationAction>,
    pub record: AdminMediaJobRecord,
}
