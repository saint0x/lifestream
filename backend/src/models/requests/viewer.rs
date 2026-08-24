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
pub struct ViewerEventInput {
    pub visitor_id: String,
    pub event_type: String,
    pub content_id: Option<Id>,
    pub content_kind: Option<String>,
    pub episode_id: Option<Id>,
    pub stream_id: Option<Id>,
    pub session_id: Option<Id>,
    pub path: Option<String>,
    pub url: Option<String>,
    pub referrer_url: Option<String>,
    pub landing_url: Option<String>,
    pub initial_referrer_url: Option<String>,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub utm_term: Option<String>,
    pub utm_content: Option<String>,
    pub progress_sec: Option<i64>,
    pub duration_sec: Option<i64>,
    pub watch_time_ms: Option<i64>,
    pub metadata: Option<Value>,
    pub occurred_at: Option<String>,
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
    #[serde(default)]
    pub website_url: NullablePatch<String>,
    #[serde(default)]
    pub instagram_url: NullablePatch<String>,
    #[serde(default)]
    pub x_url: NullablePatch<String>,
    #[serde(default)]
    pub imdb_url: NullablePatch<String>,
    #[serde(default)]
    pub linkedin_url: NullablePatch<String>,
    #[serde(default)]
    pub facebook_url: NullablePatch<String>,
    pub public_links: Option<Vec<UpdatePersonProfileLinkRequest>>,
}

#[derive(Clone, Debug)]
pub enum NullablePatch<T> {
    Unset,
    Set(Option<T>),
}

impl<T> Default for NullablePatch<T> {
    fn default() -> Self {
        Self::Unset
    }
}

impl<'de, T> Deserialize<'de> for NullablePatch<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer).map(Self::Set)
    }
}

impl<T> NullablePatch<T> {
    pub fn is_set(&self) -> bool {
        matches!(self, Self::Set(_))
    }

    pub fn as_deref(&self) -> Option<Option<&str>>
    where
        T: AsRef<str>,
    {
        match self {
            Self::Unset => None,
            Self::Set(value) => Some(value.as_ref().map(AsRef::as_ref)),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePersonProfileLinkRequest {
    pub platform: Option<String>,
    pub label: String,
    pub url: String,
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
