use super::*;

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
