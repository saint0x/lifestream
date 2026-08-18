use super::*;

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
