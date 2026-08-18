use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type Id = String;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Credit {
    pub id: Id,
    pub name: String,
    pub role: String,
    pub character: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageSet {
    pub poster: String,
    pub backdrop: String,
    pub thumbnail: String,
    pub logo: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    pub id: Id,
    pub series_id: Id,
    pub season_number: i64,
    pub episode_number: i64,
    pub title: String,
    pub synopsis: String,
    pub duration_sec: i64,
    pub aired_at: String,
    pub thumbnail: String,
    pub progress_sec: Option<i64>,
    pub playback_session_url: Option<String>,
    pub playback_ready: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Season {
    pub season_number: i64,
    pub title: String,
    pub episodes: Vec<Episode>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Series {
    pub id: Id,
    pub slug: String,
    pub kind: String,
    pub title: String,
    pub tagline: Option<String>,
    pub synopsis: String,
    pub year: i64,
    pub rating: String,
    pub genres: Vec<String>,
    pub images: ImageSet,
    pub credits: Vec<Credit>,
    pub score: i64,
    pub is_original: bool,
    pub trending: bool,
    pub hero_color: String,
    pub seasons: Vec<Season>,
    pub total_episodes: i64,
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Film {
    pub id: Id,
    pub slug: String,
    pub kind: String,
    pub title: String,
    pub tagline: Option<String>,
    pub synopsis: String,
    pub year: i64,
    pub rating: String,
    pub genres: Vec<String>,
    pub images: ImageSet,
    pub credits: Vec<Credit>,
    pub score: i64,
    pub is_original: bool,
    pub trending: bool,
    pub hero_color: String,
    pub duration_sec: i64,
    pub progress_sec: Option<i64>,
    pub playback_session_url: Option<String>,
    pub playback_ready: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Streamer {
    pub id: Id,
    pub handle: String,
    pub display_name: String,
    pub avatar: String,
    pub bio: String,
    pub followers: i64,
    pub is_partner: bool,
    pub is_live: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveStream {
    pub id: Id,
    pub slug: String,
    pub title: String,
    pub category: String,
    pub tags: Vec<String>,
    pub streamer: Streamer,
    pub viewers: i64,
    pub started_at: String,
    pub thumbnail: String,
    pub language: String,
    pub is_mature: bool,
    pub kind: String,
    pub playback_session_url: Option<String>,
    pub playback_ready: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub slug: String,
    pub name: String,
    pub cover_image: String,
    pub live_viewers: i64,
    pub live_channels: i64,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueWatchingEntry {
    pub content_id: Id,
    pub kind: String,
    pub episode_id: Option<Id>,
    pub progress_sec: i64,
    pub duration_sec: i64,
    pub last_watched_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchHistoryEntry {
    pub content_id: Id,
    pub kind: String,
    pub episode_id: Option<Id>,
    pub progress_sec: i64,
    pub duration_sec: i64,
    pub completed: bool,
    pub completed_at: Option<String>,
    pub last_watched_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: Id,
    pub handle: String,
    pub display_name: String,
    pub avatar: String,
    pub tier: String,
    pub joined_at: String,
    pub watchlist: Vec<Id>,
    pub following: Vec<Id>,
    pub continue_watching: Vec<ContinueWatchingEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserLibrary {
    pub continue_watching: Vec<ContinueWatchingEntry>,
    pub history: Vec<WatchHistoryEntry>,
    pub memberships: Vec<CreatorMembership>,
    pub purchases: Vec<ContentPurchase>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerAppState {
    pub user: User,
    pub library: UserLibrary,
    pub watchlist: WatchlistResponse,
    pub following: FollowingFeedResponse,
    pub entitlements: UserEntitlements,
    pub profile: UserProfileDetails,
    pub settings: UserSettingsBundle,
    pub plan: BillingPlan,
    pub notifications: Vec<UserNotification>,
    pub sessions: Vec<AuthSession>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: Id,
    pub sequence: i64,
    pub user_handle: String,
    pub display_name: String,
    pub color: String,
    pub badges: Vec<String>,
    pub body: String,
    pub sent_at: String,
}

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
pub struct LiveIngestSession {
    pub id: Id,
    pub creator_id: Id,
    pub broadcast_id: Id,
    pub protocol: String,
    pub ingest_server: String,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationInvite {
    pub id: Id,
    pub session_id: Id,
    pub host_creator_id: Id,
    pub invitee_user_id: Id,
    pub invitee_creator_id: Option<Id>,
    pub role: String,
    pub state: String,
    pub mirror_to_guest_channel: bool,
    pub message: Option<String>,
    pub created_at: String,
    pub responded_at: Option<String>,
    pub expires_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationParticipant {
    pub id: Id,
    pub session_id: Id,
    pub invite_id: Option<Id>,
    pub user_id: Id,
    pub creator_id: Option<Id>,
    pub role: String,
    pub state: String,
    pub publish_to_host: bool,
    pub mirror_to_guest_channel: bool,
    pub can_speak_in_chat: bool,
    pub joined_at: Option<String>,
    pub left_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationSession {
    pub id: Id,
    pub host_creator_id: Id,
    pub source_broadcast_id: Id,
    pub title: String,
    pub status: String,
    pub chat_mode: String,
    pub recording_policy: String,
    pub last_event_seq: i64,
    pub created_at: String,
    pub updated_at: String,
    pub activated_at: Option<String>,
    pub ended_at: Option<String>,
    pub invites: Vec<CollaborationInvite>,
    pub participants: Vec<CollaborationParticipant>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationHostSummary {
    pub creator_id: Id,
    pub user_id: Id,
    pub handle: String,
    pub display_name: String,
    pub avatar: String,
    pub partner_status: String,
    pub live_status: String,
    pub current_broadcast_id: Option<Id>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationSessionView {
    pub id: Id,
    pub host_creator_id: Id,
    pub source_broadcast_id: Id,
    pub title: String,
    pub status: String,
    pub chat_mode: String,
    pub recording_policy: String,
    pub last_event_seq: i64,
    pub created_at: String,
    pub updated_at: String,
    pub activated_at: Option<String>,
    pub ended_at: Option<String>,
    pub host: CollaborationHostSummary,
    pub participant: CollaborationParticipant,
    pub participants: Vec<CollaborationParticipant>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationEvent {
    pub id: Id,
    pub session_id: Id,
    pub sequence: i64,
    pub actor_user_id: Option<Id>,
    pub participant_id: Option<Id>,
    pub event_type: String,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationMirrorGrant {
    pub id: Id,
    pub session_id: Id,
    pub participant_id: Id,
    pub host_creator_id: Id,
    pub guest_creator_id: Id,
    pub scope: String,
    pub state: String,
    pub publish_to_host: bool,
    pub mirror_to_guest_channel: bool,
    pub issued_at: String,
    pub activated_at: Option<String>,
    pub revoked_at: Option<String>,
    pub expires_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationMirrorPickup {
    pub id: Id,
    pub session_id: Id,
    pub participant_id: Id,
    pub grant_id: Id,
    pub host_creator_id: Id,
    pub guest_creator_id: Id,
    pub source_broadcast_id: Id,
    pub guest_broadcast_id: Id,
    pub state: String,
    pub activated_at: String,
    pub updated_at: String,
    pub ended_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationTopologyMember {
    pub participant_id: Id,
    pub user_id: Id,
    pub creator_id: Option<Id>,
    pub role: String,
    pub state: String,
    pub publish_to_host: bool,
    pub mirror_to_guest_channel: bool,
    pub can_speak_in_chat: bool,
    pub host_output_state: String,
    pub mirror_pickup_state: String,
    pub mirror_pickup_broadcast_id: Option<Id>,
    pub mirror_pickup_activated_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationRuntimeTopology {
    pub session_id: Id,
    pub source_broadcast_id: Id,
    pub chat_mode: String,
    pub recording_policy: String,
    pub shared_chat: bool,
    pub recording_owner_creator_id: Option<Id>,
    pub connected_participants: i64,
    pub host_output_participant_ids: Vec<Id>,
    pub backstage_participant_ids: Vec<Id>,
    pub live_participant_ids: Vec<Id>,
    pub mirrored_creator_ids: Vec<Id>,
    pub members: Vec<CollaborationTopologyMember>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationRuntimeResponse {
    pub session: CollaborationSessionView,
    pub topology: CollaborationRuntimeTopology,
    pub grants: Vec<CollaborationMirrorGrant>,
    pub pickups: Vec<CollaborationMirrorPickup>,
    pub recent_events: Vec<CollaborationEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationSocketPresence {
    pub id: Id,
    pub session_id: Id,
    pub user_id: Id,
    pub creator_id: Option<Id>,
    pub participant_id: Option<Id>,
    pub connected_at: String,
    pub last_seen_at: String,
    pub disconnected_at: Option<String>,
    pub is_stale: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationSocketPresenceReconciliationAction {
    pub action_type: String,
    pub target_id: Id,
    pub previous_state: Option<String>,
    pub next_state: Option<String>,
    pub reason: String,
    pub occurred_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationSocketPresenceReconciliationReport {
    pub session_id: Id,
    pub socket_session_id: Id,
    pub reconciled_at: String,
    pub actions: Vec<CollaborationSocketPresenceReconciliationAction>,
    pub socket_session: CollaborationSocketPresence,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorCollaborationControlResponse {
    pub runtime: CollaborationRuntimeResponse,
    pub socket_sessions: Vec<CollaborationSocketPresence>,
    pub pending_invite_count: i64,
    pub active_grant_count: i64,
    pub issued_grant_count: i64,
    pub stale_socket_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLiveCollaborationSummary {
    pub active_session: Option<CollaborationSession>,
    pub active_control: Option<CreatorCollaborationControlResponse>,
    pub recent_sessions: Vec<CollaborationSession>,
    pub total_sessions: i64,
    pub active_session_count: i64,
    pub pending_invite_count: i64,
    pub active_grant_count: i64,
    pub issued_grant_count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationReconciliationAction {
    pub action_type: String,
    pub target_id: Id,
    pub previous_state: Option<String>,
    pub next_state: Option<String>,
    pub reason: String,
    pub occurred_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationReconciliationReport {
    pub session_id: Id,
    pub reconciled_at: String,
    pub actions: Vec<CollaborationReconciliationAction>,
    pub control: CreatorCollaborationControlResponse,
}

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
pub struct ConnectedAccount {
    pub id: Id,
    pub provider: String,
    pub display_name: String,
    pub connected_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfileDetails {
    pub user: User,
    pub email: String,
    pub email_verified: bool,
    pub mature_content_allowed: bool,
    pub default_audio: String,
    pub subtitle_preset: String,
    pub autoplay_trailers: bool,
    pub live_chat_filter: String,
    pub hours_watched: i64,
    pub connected_accounts: Vec<ConnectedAccount>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSettings {
    pub default_quality: String,
    pub audio_language: String,
    pub subtitle_language: String,
    pub subtitle_style: String,
    pub autoplay_next_episode: bool,
    pub autoplay_trailers: bool,
    pub reduced_motion: bool,
    pub prefer_dubbed: bool,
    pub playback_speed: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationChannelSetting {
    pub label: String,
    pub push: bool,
    pub email: bool,
    pub lock: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationSettings {
    pub series_releases: NotificationChannelSetting,
    pub live_streams: NotificationChannelSetting,
    pub originals: NotificationChannelSetting,
    pub watchlist_updates: NotificationChannelSetting,
    pub creator_updates: NotificationChannelSetting,
    pub security_alerts: NotificationChannelSetting,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacySettings {
    pub show_friend_activity: bool,
    pub improve_recommendations: bool,
    pub personalized_ads: bool,
    pub ab_tests: bool,
    pub data_export_size_mb: f64,
    pub delete_cooldown_days: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParentalControls {
    pub max_rating: String,
    pub require_pin_for_mature: bool,
    pub hide_live_chat_for_kids: bool,
    pub block_mature_live_streams: bool,
    pub pin_set: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadSettings {
    pub video_quality: String,
    pub wifi_only: bool,
    pub smart_downloads: bool,
    pub storage_used_gb: f64,
    pub storage_limit_gb: f64,
    pub device_limit: i64,
    pub active_devices: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageSettings {
    pub interface_language: String,
    pub subtitle_language: String,
    pub catalog_region: String,
    pub date_format: String,
    pub clock_format: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingPlan {
    pub plan_name: String,
    pub monthly_price: f64,
    pub next_renewal_date: String,
    pub payment_brand: String,
    pub payment_last4: String,
    pub billing_city: String,
    pub billing_region: String,
    pub billing_country: String,
    pub invoices_count: i64,
    pub screens: i64,
    pub features: Vec<String>,
    pub average_revenue_per_user: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserSettingsBundle {
    pub playback: PlaybackSettings,
    pub notifications: NotificationSettings,
    pub privacy: PrivacySettings,
    pub parental: ParentalControls,
    pub downloads: DownloadSettings,
    pub language: LanguageSettings,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorScene {
    pub id: String,
    pub label: String,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLiveSettings {
    pub subscriber_only: bool,
    pub slow_mode_seconds: i64,
    pub auto_mod_level: String,
    pub notify_followers_default: bool,
    pub active_scene_id: String,
    pub scenes: Vec<CreatorScene>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorHealthSample {
    pub collected_at: String,
    pub bitrate_kbps: i64,
    pub viewers: i64,
    pub cpu_percent: i64,
    pub dropped_frames: i64,
    pub free_disk_gb: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLiveHealth {
    pub current_bitrate_kbps: i64,
    pub current_cpu_percent: i64,
    pub current_dropped_frames: i64,
    pub current_free_disk_gb: f64,
    pub samples: Vec<CreatorHealthSample>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorSubscriberTier {
    pub id: Id,
    pub tier_name: String,
    pub rank: i64,
    pub monthly_price: f64,
    pub subscriber_count: i64,
    pub accent_color: String,
    pub status: String,
    pub retired_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorMembership {
    pub creator_id: Id,
    pub creator_handle: String,
    pub creator_display_name: String,
    pub tier_id: Id,
    pub tier_name: String,
    pub tier_rank: i64,
    pub status: String,
    pub started_at: String,
    pub renews_at: Option<String>,
    pub ends_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserEntitlementReconciliationAction {
    pub action_type: String,
    pub target_id: Id,
    pub previous_state: Option<String>,
    pub next_state: Option<String>,
    pub reason: String,
    pub occurred_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorMembershipReconciliationReport {
    pub creator_id: Id,
    pub user_id: Id,
    pub reconciled_at: String,
    pub actions: Vec<UserEntitlementReconciliationAction>,
    pub membership: CreatorMembership,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentPurchase {
    pub id: Id,
    pub creator_id: Id,
    pub creator_handle: String,
    pub creator_display_name: String,
    pub upload_id: Id,
    pub title: String,
    pub access_policy: String,
    pub amount_cents: i64,
    pub currency: String,
    pub status: String,
    pub purchased_at: String,
    pub expires_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentPurchaseReconciliationReport {
    pub purchase_id: Id,
    pub user_id: Id,
    pub reconciled_at: String,
    pub actions: Vec<UserEntitlementReconciliationAction>,
    pub purchase: ContentPurchase,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserEntitlements {
    pub memberships: Vec<CreatorMembership>,
    pub purchases: Vec<ContentPurchase>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerPreview {
    pub total_viewers: i64,
    pub sample_users: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveNotifyPreference {
    pub streamer_id: Id,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorModerator {
    pub creator_id: Id,
    pub user_id: Id,
    pub role: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveModerationAction {
    pub id: Id,
    pub stream_id: Id,
    pub creator_id: Id,
    pub subject_user_id: Id,
    pub actor_user_id: Id,
    pub action_type: String,
    pub reason: String,
    pub state: String,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub revoked_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveModerationReconciliationAction {
    pub action_type: String,
    pub target_id: Id,
    pub previous_state: Option<String>,
    pub next_state: Option<String>,
    pub reason: String,
    pub occurred_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveModerationReconciliationReport {
    pub action_id: Id,
    pub reconciled_at: String,
    pub actions: Vec<LiveModerationReconciliationAction>,
    pub action: LiveModerationAction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveStreamReportRecord {
    pub id: Id,
    pub stream_id: Id,
    pub user_id: Id,
    pub reason: String,
    pub details: Option<String>,
    pub status: String,
    pub resolved_by_user_id: Option<Id>,
    pub resolution_note: Option<String>,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModerationAuditEntry {
    pub id: Id,
    pub creator_id: Id,
    pub stream_id: Option<Id>,
    pub actor_user_id: Id,
    pub subject_user_id: Option<Id>,
    pub event_type: String,
    pub payload: Value,
    pub created_at: String,
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthDependencyStatus {
    pub ready: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthDependencies {
    pub media_root: HealthDependencyStatus,
    pub ffmpeg: HealthDependencyStatus,
    pub ffprobe: HealthDependencyStatus,
    pub background_worker: HealthDependencyStatus,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub ready: bool,
    pub database: bool,
    pub dependencies: HealthDependencies,
    pub uptime_sec: u64,
    pub timestamp: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSession {
    pub id: Id,
    pub label: String,
    pub scopes: Vec<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub revoked_at: Option<String>,
    pub last_used_at: Option<String>,
    pub is_current: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTokenResponse {
    pub session: AuthSession,
    pub access_token: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeResponse {
    pub trending_series: Vec<Series>,
    pub trending_films: Vec<Film>,
    pub featured_live: Vec<LiveStream>,
    pub categories: Vec<Category>,
    pub continue_watching: Vec<ContinueWatchingEntry>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowingFeedResponse {
    pub total_followed_streamers: i64,
    pub live_now_count: i64,
    pub followed_streamers: Vec<Streamer>,
    pub live_streams: Vec<LiveStream>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchlistResponse {
    pub total_titles: i64,
    pub series: Vec<Series>,
    pub films: Vec<Film>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveDiscoveryResponse {
    pub streams: Vec<LiveStream>,
    pub categories: Vec<Category>,
    pub total_viewers: i64,
    pub total_channels: i64,
    pub active_category: Option<String>,
    pub active_sort: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryBrowseResponse {
    pub category: Category,
    pub live_streams: Vec<LiveStream>,
    pub series: Vec<Series>,
    pub films: Vec<Film>,
    pub total_vod_titles: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorAnalyticsSummary {
    pub window_days: i64,
    pub total_viewers: i64,
    pub total_watch_minutes: i64,
    pub total_revenue: f64,
    pub total_new_followers: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorRevenueBreakdownEntry {
    pub source: String,
    pub amount: f64,
    pub share: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorRevenueSummary {
    pub total_earnings_30d: f64,
    pub total_subscribers: i64,
    pub blended_monthly_price: f64,
    pub estimated_next_payout: f64,
    pub breakdown: Vec<CreatorRevenueBreakdownEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorOperationalChecklistItem {
    pub key: String,
    pub label: String,
    pub complete: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorOperationalState {
    pub creator_id: Id,
    pub legal_name: String,
    pub support_email: String,
    pub business_type: String,
    pub payout_country: String,
    pub payout_provider: String,
    pub onboarding_status: String,
    pub identity_status: String,
    pub tax_status: String,
    pub payout_status: String,
    pub hold_reasons: Vec<String>,
    pub active_enforcement_actions: Vec<CreatorEnforcementAction>,
    pub live_streaming_enabled: bool,
    pub upload_ingest_enabled: bool,
    pub collaboration_enabled: bool,
    pub monetization_enabled: bool,
    pub payouts_enabled: bool,
    pub can_receive_payouts: bool,
    pub can_monetize: bool,
    pub can_publish_paid_content: bool,
    pub requires_action: bool,
    pub checklist: Vec<CreatorOperationalChecklistItem>,
    pub created_at: String,
    pub updated_at: String,
    pub last_reviewed_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorEnforcementAction {
    pub id: Id,
    pub creator_id: Id,
    pub scope: String,
    pub state: String,
    pub reason: String,
    pub resolution_note: Option<String>,
    pub created_by_user_id: Id,
    pub released_by_user_id: Option<Id>,
    pub created_at: String,
    pub released_at: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorEnforcementReconciliationAction {
    pub action_type: String,
    pub target_id: Id,
    pub previous_state: Option<String>,
    pub next_state: Option<String>,
    pub reason: String,
    pub occurred_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorEnforcementReconciliationReport {
    pub action_id: Id,
    pub reconciled_at: String,
    pub actions: Vec<CreatorEnforcementReconciliationAction>,
    pub action: CreatorEnforcementAction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorEnforcementState {
    pub creator_id: Id,
    pub live_streaming_enabled: bool,
    pub upload_ingest_enabled: bool,
    pub collaboration_enabled: bool,
    pub monetization_enabled: bool,
    pub payouts_enabled: bool,
    pub active_actions: Vec<CreatorEnforcementAction>,
    pub history: Vec<CreatorEnforcementAction>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorDashboard {
    pub profile: CreatorProfile,
    pub current_broadcast: Option<Broadcast>,
    pub scheduled_broadcasts: Vec<Broadcast>,
    pub recent_broadcasts: Vec<Broadcast>,
    pub analytics: Vec<AnalyticsPoint>,
    pub traffic_sources: Vec<TrafficSource>,
    pub top_content: Vec<TopContent>,
    pub revenue: Vec<RevenueEntry>,
    pub analytics_summary: CreatorAnalyticsSummary,
    pub revenue_summary: CreatorRevenueSummary,
    pub subscriber_tiers: Vec<CreatorSubscriberTier>,
    pub operational_state: CreatorOperationalState,
    pub notifications: Vec<CreatorNotification>,
    pub uploads: Vec<Upload>,
}

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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLiveRuntimeResponse {
    pub snapshot: CreatorLiveSnapshot,
    pub health: CreatorLiveHealth,
    pub collaboration: CreatorLiveCollaborationSummary,
    pub active_session: Option<LiveIngestSession>,
    pub recent_sessions: Vec<LiveIngestSession>,
    pub recent_events: Vec<LiveIngestEvent>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorContentSummary {
    pub total_uploads: i64,
    pub published_uploads: i64,
    pub scheduled_uploads: i64,
    pub processing_uploads: i64,
    pub draft_uploads: i64,
    pub archived_uploads: i64,
    pub total_views: i64,
    pub total_watch_hours: i64,
    pub total_storage_bytes: i64,
    pub filtered_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorContentResponse {
    pub summary: CreatorContentSummary,
    pub uploads: Vec<Upload>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorUploadOperationRecord {
    pub upload_job: UploadJob,
    pub ingest_session: Option<UploadIngestSession>,
    pub media_asset: Option<MediaAsset>,
    pub published_upload: Option<Upload>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorUploadOperationsSummary {
    pub total_jobs: i64,
    pub created_jobs: i64,
    pub uploaded_jobs: i64,
    pub processing_jobs: i64,
    pub ready_jobs: i64,
    pub failed_jobs: i64,
    pub published_jobs: i64,
    pub active_ingest_sessions: i64,
    pub completed_ingest_sessions: i64,
    pub ready_assets: i64,
    pub processing_assets: i64,
    pub failed_assets: i64,
    pub published_assets: i64,
    pub total_bytes_expected: i64,
    pub total_bytes_received: i64,
    pub total_asset_bytes: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorUploadOperationsResponse {
    pub summary: CreatorUploadOperationsSummary,
    pub records: Vec<CreatorUploadOperationRecord>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorAppState {
    pub dashboard: CreatorDashboard,
    pub live_control: CreatorLiveControlResponse,
    pub live_runtime: CreatorLiveRuntimeResponse,
    pub content: CreatorContentResponse,
    pub upload_operations: CreatorUploadOperationsResponse,
}

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

#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum WsEvent {
    SessionReady {
        channel: String,
        session_token: String,
        resumed: bool,
        last_seen_at: String,
    },
    SessionInvalidated {
        reason: String,
    },
    ChatReplay {
        after_seq: i64,
        messages: Vec<ChatMessage>,
    },
    ChatHistory {
        messages: Vec<ChatMessage>,
    },
    ChatMessage {
        message: ChatMessage,
    },
    ChatMessageRejected {
        reason: String,
    },
    ViewerCount {
        viewer_count: i64,
    },
    CollaborationSnapshot {
        session: CollaborationSessionView,
        grants: Vec<CollaborationMirrorGrant>,
        pickups: Vec<CollaborationMirrorPickup>,
        events: Vec<CollaborationEvent>,
    },
    CollaborationReplay {
        after_seq: i64,
        events: Vec<CollaborationEvent>,
    },
    CollaborationEvent {
        event: CollaborationEvent,
    },
    CollaborationPresence {
        session_id: Id,
        connected_participants: i64,
    },
    CollaborationHeartbeat {
        session_id: Id,
        received_at: String,
    },
    CollaborationCommandAccepted {
        command_type: String,
        participant_id: Option<Id>,
        state: Option<String>,
    },
    CollaborationCommandRejected {
        command_type: String,
        reason: String,
    },
    CollaborationTopology {
        topology: CollaborationRuntimeTopology,
    },
    CreatorLiveState {
        control: CreatorLiveControlResponse,
        runtime: CreatorLiveRuntimeResponse,
    },
    ModerationAction {
        action: LiveModerationAction,
    },
}
