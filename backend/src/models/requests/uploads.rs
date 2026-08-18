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
    #[serde(rename = "_HLS_msn")]
    pub hls_msn: Option<i64>,
    #[serde(rename = "_HLS_part")]
    pub hls_part: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendUploadChunkQuery {
    pub offset: i64,
}
