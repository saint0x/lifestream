use super::*;

mod lifecycle;
mod reconciliation;
mod targets;
mod validation;

pub(crate) use lifecycle::{expire_playback_session_by_id, expire_playback_sessions_for_upload};
pub(crate) use reconciliation::{
    reconcile_invalid_playback_sessions, reconcile_playback_sessions_for_read,
    reconcile_playback_sessions_for_user, reconcile_single_playback_session,
};
pub(crate) use targets::{
    fetch_live_stream_playback_target, fetch_upload_playback_target,
    fetch_upload_playback_target_for_database, playback_session_from_record,
};
pub(crate) use validation::{
    fetch_playback_session_record_by_id, validate_existing_playback_session_access,
    validate_playback_session_record, validate_playback_session_record_for_path,
};

fn playback_session_record_from_row(row: sqlx::sqlite::SqliteRow) -> PlaybackSessionRecord {
    PlaybackSessionRecord {
        id: row.get("id"),
        auth_session_id: row.get("auth_session_id"),
        user_id: row.get("user_id"),
        creator_id: row.get("creator_id"),
        asset_id: row.get("asset_id"),
        content_id: row.get("content_id"),
        content_kind: row.get("content_kind"),
        access_scope: row.get("access_scope"),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        last_used_at: row.get("last_used_at"),
    }
}
