use super::*;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatInput {
    pub color: Option<String>,
    pub body: String,
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
pub struct UpdatePersonProfileRequest {
    pub slug: Option<String>,
    pub display_name: Option<String>,
    pub avatar: Option<String>,
    pub hero_image: Option<String>,
    pub headline: Option<String>,
    pub location: Option<String>,
    pub about: Option<String>,
    pub known_for: Option<Vec<String>>,
    pub website_url: Option<String>,
    pub instagram_url: Option<String>,
    pub x_url: Option<String>,
    pub imdb_url: Option<String>,
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
