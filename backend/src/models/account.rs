use super::*;

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
    pub delivery_class: String,
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
pub struct AdvertiserCompany {
    pub id: Id,
    pub name: String,
    pub industry: String,
    pub website_url: Option<String>,
    pub status: String,
    pub billing_name: String,
    pub billing_email: String,
    pub billing_status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvertiserSeat {
    pub user_id: Id,
    pub email: String,
    pub name: String,
    pub role: String,
    pub permissions: Vec<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvertiserInvite {
    pub id: Id,
    pub email: String,
    pub role: String,
    pub permissions: Vec<String>,
    pub status: String,
    pub invited_by_user_id: Id,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvertiserPermissionPreset {
    pub role: String,
    pub label: String,
    pub permissions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvertiserAccountResponse {
    pub company: AdvertiserCompany,
    pub current_seat: AdvertiserSeat,
    pub seats: Vec<AdvertiserSeat>,
    pub invites: Vec<AdvertiserInvite>,
    pub permission_presets: Vec<AdvertiserPermissionPreset>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAdvertiserCompanyRequest {
    pub name: String,
    pub industry: String,
    pub website_url: Option<String>,
    pub billing_name: String,
    pub billing_email: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAdvertiserInviteRequest {
    pub email: String,
    pub name: Option<String>,
    pub role: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAdvertiserSeatRequest {
    pub role: String,
    pub status: Option<String>,
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
