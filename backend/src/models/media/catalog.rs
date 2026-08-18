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
