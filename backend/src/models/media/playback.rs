use super::*;

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
    pub access_tier_id: Option<Id>,
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
