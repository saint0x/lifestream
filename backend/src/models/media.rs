use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Upload {
    pub id: Id,
    pub slug: Option<String>,
    pub title: String,
    pub description: String,
    pub kind: String,
    pub duration_sec: i64,
    pub uploaded_at: String,
    pub published_at: Option<String>,
    pub release_at: Option<String>,
    pub status: String,
    pub visibility: String,
    pub access_policy: String,
    pub access_tier_id: Option<String>,
    pub price_cents: Option<i64>,
    pub currency: Option<String>,
    pub rental_window_hours: Option<i64>,
    pub views: i64,
    pub likes: i64,
    pub comments: i64,
    pub watch_hours: i64,
    pub thumbnail: String,
    pub series_title: Option<String>,
    pub season_number: Option<i64>,
    pub episode_number: Option<i64>,
    pub size_bytes: i64,
    pub resolution: String,
    pub transcode_progress: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorCatalogEpisode {
    pub id: Id,
    pub upload_id: Id,
    pub playback_session_url: Option<String>,
    pub slug: String,
    pub series_id: Id,
    pub series_slug: String,
    pub season_number: i64,
    pub episode_number: i64,
    pub title: String,
    pub synopsis: String,
    pub duration_sec: i64,
    pub release_at: String,
    pub thumbnail: String,
    pub access_policy: String,
    pub access_tier_id: Option<String>,
    pub price_cents: Option<i64>,
    pub currency: Option<String>,
    pub rental_window_hours: Option<i64>,
    pub playback_ready: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorCatalogSeason {
    pub id: Id,
    pub season_number: i64,
    pub title: String,
    pub synopsis: String,
    pub episodes: Vec<CreatorCatalogEpisode>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorCatalogSeries {
    pub id: Id,
    pub slug: String,
    pub title: String,
    pub synopsis: String,
    pub rating: String,
    pub genres: Vec<String>,
    pub hero_color: String,
    pub poster_url: String,
    pub backdrop_url: String,
    pub status: String,
    pub creator_handle: String,
    pub creator_display_name: String,
    pub published_episode_count: i64,
    pub seasons: Vec<CreatorCatalogSeason>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorCatalogFilm {
    pub id: Id,
    pub upload_id: Id,
    pub playback_session_url: Option<String>,
    pub slug: String,
    pub title: String,
    pub synopsis: String,
    pub duration_sec: i64,
    pub release_at: String,
    pub thumbnail: String,
    pub resolution: String,
    pub creator_handle: String,
    pub creator_display_name: String,
    pub access_policy: String,
    pub access_tier_id: Option<String>,
    pub price_cents: Option<i64>,
    pub currency: Option<String>,
    pub rental_window_hours: Option<i64>,
    pub playback_ready: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsPoint {
    pub date: String,
    pub viewers: i64,
    pub watch_minutes: i64,
    pub revenue: f64,
    pub new_followers: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficSource {
    pub source: String,
    pub sessions: i64,
    pub share: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopContent {
    pub id: Id,
    pub title: String,
    pub kind: String,
    pub views: i64,
    pub watch_hours: i64,
    pub trend: f64,
    pub thumbnail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevenueEntry {
    pub id: Id,
    pub date: String,
    pub source: String,
    pub description: String,
    pub amount: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorNotification {
    pub id: Id,
    pub kind: String,
    pub body: String,
    pub sent_at: String,
    pub amount: Option<f64>,
    pub actor: Option<String>,
    pub delivery_state: Option<String>,
    pub read_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserNotification {
    pub id: Id,
    pub kind: String,
    pub body: String,
    pub sent_at: String,
    pub amount: Option<f64>,
    pub actor: Option<String>,
    pub delivery_state: String,
    pub read_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationDeliveryRecord {
    pub id: Id,
    pub event_id: Id,
    pub kind: String,
    pub body: String,
    pub channel: String,
    pub state: String,
    pub actor: Option<String>,
    pub recipient_user_id: Option<Id>,
    pub recipient_creator_id: Option<Id>,
    pub sent_at: String,
    pub delivered_at: Option<String>,
    pub read_at: Option<String>,
    pub failed_at: Option<String>,
    pub last_error: Option<String>,
    pub retry_count: i64,
    pub last_attempted_at: Option<String>,
    pub next_attempt_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationDeliveryReconciliationAction {
    pub action_type: String,
    pub target_id: Id,
    pub previous_state: Option<String>,
    pub next_state: Option<String>,
    pub reason: String,
    pub occurred_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationDeliveryReconciliationReport {
    pub delivery_id: Id,
    pub reconciled_at: String,
    pub actions: Vec<NotificationDeliveryReconciliationAction>,
    pub delivery: NotificationDeliveryRecord,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorSeriesProject {
    pub id: Id,
    pub slug: String,
    pub title: String,
    pub synopsis: String,
    pub rating: String,
    pub genres: Vec<String>,
    pub hero_color: String,
    pub poster_url: String,
    pub backdrop_url: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

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
pub struct PlaybackAudioTrack {
    pub id: Id,
    pub label: String,
    pub language: String,
    pub codec: Option<String>,
    pub playlist_path: Option<String>,
    pub playlist_url: Option<String>,
    pub source: String,
    pub is_dubbed: bool,
    pub is_default: bool,
    pub published: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackCaptionTrack {
    pub id: Id,
    pub label: String,
    pub language: String,
    pub role: String,
    pub source: String,
    pub mime_type: String,
    pub url: String,
    pub is_default: bool,
    pub published: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackPreviewTrack {
    pub id: Id,
    pub label: String,
    pub image_path: String,
    pub image_url: String,
    pub vtt_path: String,
    pub vtt_url: String,
    pub tile_width: i64,
    pub tile_height: i64,
    pub columns_count: i64,
    pub rows_count: i64,
    pub interval_sec: f64,
    pub frame_count: i64,
    pub is_default: bool,
    pub published: bool,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSession {
    pub id: Id,
    pub content_id: Id,
    pub content_kind: String,
    pub access_scope: String,
    pub created_at: String,
    pub expires_at: String,
    pub last_used_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminPlaybackSessionRecord {
    pub session: PlaybackSession,
    pub user_id: Option<Id>,
    pub creator_id: Option<Id>,
    pub asset_id: Id,
    pub active: bool,
    pub valid_access: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackReconciliationAction {
    pub action_type: String,
    pub target_id: Id,
    pub previous_state: Option<String>,
    pub next_state: Option<String>,
    pub reason: String,
    pub occurred_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackReconciliationReport {
    pub session_id: Id,
    pub reconciled_at: String,
    pub actions: Vec<PlaybackReconciliationAction>,
    pub record: AdminPlaybackSessionRecord,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackGrant {
    pub session: PlaybackSession,
    pub playback_token: String,
    pub manifest_url: String,
    pub poster_url: Option<String>,
    pub content_title: String,
    pub content_kind: String,
    pub visibility: String,
    pub access_policy: String,
    pub access_tier_id: Option<String>,
    pub price_cents: Option<i64>,
    pub currency: Option<String>,
    pub rental_window_hours: Option<i64>,
    pub audio_tracks: Vec<PlaybackAudioTrack>,
    pub caption_tracks: Vec<PlaybackCaptionTrack>,
    pub preview_tracks: Vec<PlaybackPreviewTrack>,
    pub default_audio_track_id: Option<Id>,
    pub default_caption_track_id: Option<Id>,
    pub default_preview_track_id: Option<Id>,
}
