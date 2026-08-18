use super::*;

mod catalog;
mod health;
mod live;

pub(crate) use catalog::bootstrap;
pub(crate) use health::{health, health_live, health_ready, metrics};
#[cfg(test)]
pub(crate) use health::{
    check_binary_available, check_media_root_writable, check_runtime_dependencies_with_binaries,
};
pub(crate) use live::{
    create_clip_request, create_live_moderation_action, get_live_moderation_action,
    get_live_viewer_preview, list_chat_messages, list_live_moderation_actions,
    list_live_streams, reconcile_live_moderation_action, remove_live_stream_moderator,
    resolve_live_stream_report, revoke_live_moderation_action,
};
pub(crate) use live::PersistedChatMessage;
#[cfg(test)]
pub(crate) use live::LimitQuery;

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/health", get(health))
        .route("/health/live", get(health_live))
        .route("/health/ready", get(health_ready))
        .route("/metrics", get(metrics))
        .route("/api/v1/home", get(catalog::home))
        .route("/api/v1/bootstrap", get(bootstrap))
        .route("/api/v1/catalog/series", get(catalog::list_series))
        .route("/api/v1/catalog/series/:slug", get(catalog::get_series))
        .route("/api/v1/catalog/films", get(catalog::list_films))
        .route("/api/v1/catalog/films/:slug", get(catalog::get_film))
        .route("/api/v1/catalog/content/:id", get(catalog::get_content))
        .route(
            "/api/v1/catalog/creator/series",
            get(catalog::list_creator_catalog_series),
        )
        .route(
            "/api/v1/catalog/creator/series/:slug",
            get(catalog::get_creator_catalog_series),
        )
        .route(
            "/api/v1/catalog/creator/films",
            get(catalog::list_creator_catalog_films),
        )
        .route(
            "/api/v1/catalog/creator/films/:slug",
            get(catalog::get_creator_catalog_film),
        )
        .route("/api/v1/live/streams", get(list_live_streams))
        .route("/api/v1/live/streams/:slug", get(live::get_live_stream))
        .route("/api/v1/live/discovery", get(live::get_live_discovery))
        .route(
            "/api/v1/live/streams/:stream_id/notify",
            post(live::enable_live_notify),
        )
        .route(
            "/api/v1/live/streams/:stream_id/clip",
            post(create_clip_request),
        )
        .route(
            "/api/v1/live/streams/:stream_id/report",
            post(live::report_live_stream),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/moderators",
            get(live::list_live_stream_moderators).post(live::add_live_stream_moderator),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/moderators/:user_id",
            delete(remove_live_stream_moderator),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/actions",
            get(list_live_moderation_actions).post(create_live_moderation_action),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/actions/:action_id",
            get(get_live_moderation_action),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/actions/:action_id/reconcile",
            post(reconcile_live_moderation_action),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/actions/:action_id/revoke",
            post(revoke_live_moderation_action),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/reports",
            get(live::list_live_stream_reports),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/reports/:report_id",
            patch(resolve_live_stream_report),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/audit",
            get(live::list_live_moderation_audit_log),
        )
        .route(
            "/api/v1/live/streams/:stream_id/viewers",
            get(get_live_viewer_preview),
        )
        .route(
            "/api/v1/live/streams/:stream_id/chat",
            get(list_chat_messages),
        )
        .route(
            "/api/v1/live/streams/:stream_id/chat/messages",
            post(live::post_chat_message),
        )
        .route("/api/v1/categories", get(catalog::list_categories))
        .route("/api/v1/categories/:slug", get(catalog::get_category))
        .route(
            "/api/v1/categories/:slug/browse",
            get(catalog::get_category_browse),
        )
        .route("/api/v1/streamers", get(catalog::list_streamers))
        .route("/api/v1/streamers/:id", get(catalog::get_streamer))
        .route("/api/v1/search", get(catalog::search))
}
