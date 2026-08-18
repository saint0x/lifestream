use super::creator::get_creator_live;
use super::*;

mod admin;
mod creator;
mod live;
mod repair;

#[cfg(test)]
pub(crate) use admin::{
    get_admin_live_ingest_overview, get_admin_live_ingest_session,
    reconcile_admin_live_ingest_session, repair_admin_live_runtime_output,
};
pub(crate) use creator::update_creator_live;
#[cfg(test)]
pub(crate) use creator::{
    end_broadcast, get_creator_live_ingest_session_by_id, list_creator_live_ingest_events,
    reconcile_creator_live_ingest_session, repair_creator_live_runtime_output,
    terminate_creator_live_ingest,
};
#[cfg(test)]
pub(crate) use live::{
    connect_live_ingest, disconnect_live_ingest, heartbeat_live_ingest, report_live_runtime,
    terminate_live_ingest,
};

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/api/v1/admin/live/ingest/sessions",
            get(admin::list_admin_live_ingest_sessions),
        )
        .route(
            "/api/v1/admin/live/ingest/overview",
            get(admin::get_admin_live_ingest_overview),
        )
        .route(
            "/api/v1/admin/live/ingest/sessions/:session_id",
            get(admin::get_admin_live_ingest_session),
        )
        .route(
            "/api/v1/admin/live/ingest/sessions/:session_id/reconcile",
            post(admin::reconcile_admin_live_ingest_session),
        )
        .route(
            "/api/v1/admin/live/ingest/sessions/:session_id/terminate",
            post(admin::terminate_admin_live_ingest_session),
        )
        .route(
            "/api/v1/admin/live/ingest/sessions/:session_id/runtime/repair",
            post(admin::repair_admin_live_runtime_output),
        )
        .route(
            "/api/v1/creator/me/broadcasts/start",
            post(creator::start_broadcast),
        )
        .route(
            "/api/v1/creator/me/broadcasts/:id/end",
            post(creator::end_broadcast),
        )
        .route(
            "/api/v1/creator/me/stream-key/rotate",
            post(creator::rotate_stream_key),
        )
        .route(
            "/api/v1/creator/me/live/ingest",
            get(creator::get_creator_live_ingest_session),
        )
        .route(
            "/api/v1/creator/me/live/ingest/:session_id",
            get(creator::get_creator_live_ingest_session_by_id),
        )
        .route(
            "/api/v1/creator/me/live/ingest/:session_id/events",
            get(creator::list_creator_live_ingest_events),
        )
        .route(
            "/api/v1/creator/me/live/ingest/:session_id/reconcile",
            post(creator::reconcile_creator_live_ingest_session),
        )
        .route(
            "/api/v1/creator/me/live/ingest/:session_id/terminate",
            post(creator::terminate_creator_live_ingest),
        )
        .route(
            "/api/v1/creator/me/live/ingest/:session_id/runtime/repair",
            post(creator::repair_creator_live_runtime_output),
        )
        .route(
            "/api/v1/ingest/live/connect",
            post(live::connect_live_ingest),
        )
        .route(
            "/api/v1/ingest/live/:session_id/heartbeat",
            post(live::heartbeat_live_ingest),
        )
        .route(
            "/api/v1/ingest/live/:session_id/disconnect",
            post(live::disconnect_live_ingest),
        )
        .route(
            "/api/v1/ingest/live/:session_id/terminate",
            post(live::terminate_live_ingest),
        )
        .route(
            "/api/v1/ingest/live/:session_id/runtime",
            post(live::report_live_runtime),
        )
}
