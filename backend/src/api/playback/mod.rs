use super::discovery::fetch_user;
use super::*;

mod admin;
mod grants;
mod purchase;
mod sessions;

pub(crate) use admin::{get_admin_playback_session, reconcile_admin_playback_session};
pub(crate) use grants::{get_playback_manifest, get_playback_session, refresh_playback_session};
pub(crate) use sessions::{create_content_playback_session, create_live_playback_session};

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/api/v1/admin/playback/sessions",
            get(admin::list_admin_playback_sessions),
        )
        .route(
            "/api/v1/admin/playback/sessions/:session_id",
            get(admin::get_admin_playback_session),
        )
        .route(
            "/api/v1/admin/playback/sessions/:session_id/reconcile",
            post(admin::reconcile_admin_playback_session),
        )
        .route(
            "/api/v1/admin/playback/sessions/:session_id/revoke",
            post(admin::revoke_admin_playback_session),
        )
        .route(
            "/api/v1/playback/uploads/:upload_id/session",
            post(sessions::create_upload_playback_session),
        )
        .route(
            "/api/v1/playback/content/:content_id/session",
            post(sessions::create_content_playback_session),
        )
        .route(
            "/api/v1/playback/live/:stream_id/session",
            post(sessions::create_live_playback_session),
        )
        .route(
            "/api/v1/uploads/:upload_id/purchase",
            post(purchase::purchase_upload_access),
        )
        .route(
            "/api/v1/content/:content_id/purchase",
            post(purchase::purchase_content_access),
        )
        .route(
            "/api/v1/playback/sessions/:session_id",
            get(grants::get_playback_session),
        )
        .route(
            "/api/v1/playback/sessions/:session_id/refresh",
            post(grants::refresh_playback_session),
        )
        .route(
            "/api/v1/playback/sessions/:session_id/manifest",
            get(grants::get_playback_manifest),
        )
}
