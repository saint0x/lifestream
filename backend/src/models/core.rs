use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Credit {
    pub id: Id,
    pub person_id: Option<Id>,
    pub person_slug: Option<String>,
    pub name: String,
    pub role: String,
    pub character: Option<String>,
    pub avatar: Option<String>,
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
pub struct PersonCredit {
    pub content_id: Id,
    pub content_slug: String,
    pub content_kind: String,
    pub title: String,
    pub year: i64,
    pub role: String,
    pub character: Option<String>,
    pub poster: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonProfileLink {
    pub id: Id,
    pub platform: String,
    pub label: String,
    pub url: String,
    pub position: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonProfile {
    pub id: Id,
    pub user_id: Option<Id>,
    pub slug: String,
    pub profile_url_path: String,
    pub display_name: String,
    pub avatar: String,
    pub hero_image: String,
    pub headline: String,
    pub location: String,
    pub about: String,
    pub known_for: Vec<String>,
    pub website_url: Option<String>,
    pub instagram_url: Option<String>,
    pub x_url: Option<String>,
    pub imdb_url: Option<String>,
    pub linkedin_url: Option<String>,
    pub facebook_url: Option<String>,
    pub public_links: Vec<PersonProfileLink>,
    pub created_at: String,
    pub updated_at: String,
    pub credits: Vec<PersonCredit>,
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
