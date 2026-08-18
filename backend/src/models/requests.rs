use super::*;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressInput {
    pub content_id: Id,
    pub kind: String,
    pub episode_id: Option<Id>,
    pub progress_sec: i64,
    #[serde(rename = "durationSec")]
    pub _duration_sec: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatInput {
    pub color: Option<String>,
    pub body: String,
}

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
pub struct UpdateUploadRequest {
    pub title: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub visibility: Option<String>,
    pub release_at: Option<String>,
    pub access_policy: Option<String>,
    pub access_tier_id: Option<String>,
    pub price_cents: Option<i64>,
    pub currency: Option<String>,
    pub rental_window_hours: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUploadLifecycleRequest {
    pub release_at: Option<String>,
    pub visibility: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkUploadRequest {
    pub upload_ids: Vec<Id>,
    pub action: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorContentQuery {
    pub kind: Option<String>,
    pub status: Option<String>,
    pub q: Option<String>,
    pub sort: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    pub label: String,
    pub scopes: Option<Vec<String>>,
    pub expires_in_days: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileRequest {
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub mature_content_allowed: Option<bool>,
    pub default_audio: Option<String>,
    pub subtitle_preset: Option<String>,
    pub autoplay_trailers: Option<bool>,
    pub live_chat_filter: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsRequest {
    pub playback: Option<PlaybackSettings>,
    pub notifications: Option<NotificationSettings>,
    pub privacy: Option<PrivacySettings>,
    pub parental: Option<ParentalControls>,
    pub downloads: Option<DownloadSettings>,
    pub language: Option<LanguageSettings>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCreatorLiveSettingsRequest {
    pub subscriber_only: Option<bool>,
    pub slow_mode_seconds: Option<i64>,
    pub auto_mod_level: Option<String>,
    pub notify_followers_default: Option<bool>,
    pub active_scene_id: Option<String>,
    pub scenes: Option<Vec<CreatorScene>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCreatorOperationalStateRequest {
    pub legal_name: Option<String>,
    pub support_email: Option<String>,
    pub business_type: Option<String>,
    pub payout_country: Option<String>,
    pub payout_provider: Option<String>,
    pub submit_onboarding: Option<bool>,
    pub submit_identity_verification: Option<bool>,
    pub submit_tax_profile: Option<bool>,
    pub submit_payout_method: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCreatorSubscriberTierRequest {
    pub tier_name: String,
    pub rank: Option<i64>,
    pub monthly_price: f64,
    pub accent_color: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCreatorSubscriberTierRequest {
    pub tier_name: Option<String>,
    pub rank: Option<i64>,
    pub monthly_price: Option<f64>,
    pub accent_color: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollaborationSessionRequest {
    pub broadcast_id: Option<Id>,
    pub title: Option<String>,
    pub chat_mode: Option<String>,
    pub recording_policy: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollaborationInviteRequest {
    pub invitee_user_id: Id,
    pub role: String,
    pub mirror_to_guest_channel: bool,
    pub message: Option<String>,
    pub expires_in_minutes: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCollaborationParticipantRequest {
    pub state: Option<String>,
    pub publish_to_host: Option<bool>,
    pub mirror_to_guest_channel: Option<bool>,
    pub can_speak_in_chat: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationEventsQuery {
    pub after_seq: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveReportRequest {
    pub reason: String,
    pub details: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCreatorModeratorRequest {
    pub user_id: Id,
    pub role: String,
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
pub struct CreateCreatorEnforcementActionRequest {
    pub scope: String,
    pub reason: String,
    pub expires_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseCreatorEnforcementActionRequest {
    pub resolution_note: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCreatorSeriesRequest {
    pub slug: String,
    pub title: String,
    pub synopsis: String,
    pub rating: String,
    pub genres: Vec<String>,
    pub hero_color: String,
    pub poster_url: String,
    pub backdrop_url: String,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCreatorSeriesRequest {
    pub title: Option<String>,
    pub synopsis: Option<String>,
    pub rating: Option<String>,
    pub genres: Option<Vec<String>>,
    pub hero_color: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub status: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationDeliveryQuery {
    pub state: Option<String>,
    pub creator_id: Option<Id>,
    pub limit: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminMediaJobQuery {
    pub status: Option<String>,
    pub creator_id: Option<Id>,
    pub limit: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminLiveIngestQuery {
    pub creator_id: Option<Id>,
    pub status: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminPlaybackSessionQuery {
    pub creator_id: Option<Id>,
    pub content_id: Option<Id>,
    pub state: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateUploadJobRequest {
    pub upload_id: Option<Id>,
    pub series_id: Option<Id>,
    pub kind: String,
    pub source_type: String,
    pub title: String,
    pub intended_visibility: String,
    pub bytes_expected: i64,
    pub storage_key: String,
    pub mime_type: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUploadJobRequest {
    pub title: Option<String>,
    pub intended_visibility: Option<String>,
    pub series_id: Option<Id>,
    pub mime_type: Option<String>,
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
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminateLiveIngestRequest {
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishUploadJobRequest {
    pub description: Option<String>,
    pub visibility: Option<String>,
    pub slug: Option<String>,
    pub release_at: Option<String>,
    pub access_policy: Option<String>,
    pub access_tier_id: Option<String>,
    pub price_cents: Option<i64>,
    pub currency: Option<String>,
    pub rental_window_hours: Option<i64>,
    pub season_number: Option<i64>,
    pub episode_number: Option<i64>,
    pub season_title: Option<String>,
    pub season_synopsis: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackAccessQuery {
    pub playback_token: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendUploadChunkQuery {
    pub offset: i64,
}
