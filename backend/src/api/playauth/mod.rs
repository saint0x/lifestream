use super::*;

pub(super) struct UploadPlaybackTarget {
    pub(super) creator_id: String,
    pub(super) upload: Upload,
    pub(super) asset: MediaAsset,
}

pub(super) struct LivePlaybackTarget {
    pub(super) creator_id: String,
    pub(super) asset_id: String,
    pub(super) title: String,
    pub(super) poster_relative_path: Option<String>,
    pub(super) playback_relative_path: String,
    pub(super) runtime_output: LiveRuntimeOutput,
    pub(super) asset: MediaAsset,
}

pub(crate) struct PlaybackSessionRecord {
    pub(crate) id: String,
    pub(crate) auth_session_id: Option<String>,
    pub(crate) user_id: Option<String>,
    pub(crate) creator_id: Option<String>,
    pub(crate) asset_id: String,
    pub(crate) content_id: String,
    pub(crate) content_kind: String,
    pub(crate) access_scope: String,
    pub(crate) created_at: String,
    pub(crate) expires_at: String,
    pub(crate) last_used_at: String,
}

pub(super) struct UploadAccessTerms {
    pub(super) access_policy: String,
    pub(super) access_tier_id: Option<String>,
    pub(super) price_cents: Option<i64>,
    pub(super) currency: Option<String>,
    pub(super) rental_window_hours: Option<i64>,
}

pub(super) struct PlaybackAccessDecision {
    pub(super) access_scope: String,
}

mod access;
mod sessions;
mod tracks;

pub(super) use access::{
    fetch_active_creator_membership, resolve_upload_access_terms, resolve_upload_playback_access,
};
#[cfg(test)]
pub(super) use sessions::fetch_upload_playback_target;
pub(super) use sessions::{
    expire_playback_session_by_id, expire_playback_sessions_for_upload,
    fetch_live_stream_playback_target, fetch_playback_session_record_by_id,
    fetch_upload_playback_target_for_database, playback_session_from_record,
    reconcile_invalid_playback_sessions, reconcile_playback_sessions_for_read,
    reconcile_playback_sessions_for_user, reconcile_single_playback_session,
    validate_existing_playback_session_access, validate_playback_session_record,
    validate_playback_session_record_for_path,
};
pub(super) use tracks::{
    build_media_audio_tracks, build_media_caption_tracks, build_media_preview_tracks,
    default_audio_track_id, default_caption_track_id, default_preview_track_id,
    fetch_user_playback_preferences_for_database,
};
