use super::*;

mod auth;
mod catalog;
mod health;
mod live;
pub(super) mod people;

pub(super) use catalog::CatalogRepository;
pub(crate) use catalog::bootstrap;
#[cfg(test)]
pub(crate) use catalog::{
    CatalogPageQuery, SearchQuery, get_series_for_episode, list_films_page, list_series_page,
    search,
};
pub(crate) use catalog::{
    postgres_fetch_film_by_id, postgres_fetch_live_streams, postgres_fetch_series_by_id,
    postgres_fetch_streamer_by_id,
};
#[cfg(test)]
pub(crate) use health::{
    check_binary_available, check_media_root_writable, check_runtime_dependencies_with_binaries,
};
pub(crate) use health::{health, health_live, health_ready, metrics};
#[cfg(test)]
pub(crate) use live::LimitQuery;
pub(crate) use live::PersistedChatMessage;
pub(crate) use live::list_live_streams;
#[cfg(test)]
pub(crate) use live::{
    create_clip_request, create_live_moderation_action, get_live_moderation_action,
    get_live_viewer_preview, list_chat_messages, list_live_moderation_actions,
    reconcile_live_moderation_action, remove_live_stream_moderator, resolve_live_stream_report,
    revoke_live_moderation_action,
};

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/health", get(health))
        .route("/health/live", get(health_live))
        .route("/health/ready", get(health_ready))
        .route("/metrics", get(metrics))
        .route("/api/v1/home", get(catalog::home))
        .route("/api/v1/bootstrap", get(bootstrap))
        .route(
            "/api/auth/sign-in/anonymous",
            post(auth::create_guest_session),
        )
        .route("/api/auth/sign-up/email", post(auth::sign_up_email))
        .route("/api/auth/sign-in/email", post(auth::sign_in_email))
        .route("/api/auth/sign-in/social", post(auth::sign_in_social))
        .route("/api/auth/sign-in/google", get(auth::start_google_auth))
        .route(
            "/api/auth/callback/google",
            get(auth::google_oauth_callback),
        )
        .route("/api/v1/catalog/series", get(catalog::list_series))
        .route(
            "/api/v1/catalog/series/page",
            get(catalog::list_series_page),
        )
        .route(
            "/api/v1/catalog/episodes/:id/series",
            get(catalog::get_series_for_episode),
        )
        .route("/api/v1/catalog/series/:slug", get(catalog::get_series))
        .route("/api/v1/catalog/films", get(catalog::list_films))
        .route("/api/v1/catalog/films/page", get(catalog::list_films_page))
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
        .route(
            "/api/v1/live/streams/:stream_id/chat",
            get(live::list_chat_messages).post(live::post_chat_message),
        )
        .route(
            "/api/v1/live/streams/:stream_id/notify",
            post(live::enable_live_notify),
        )
        .route(
            "/api/v1/live/streams/:stream_id/clip-requests",
            post(live::create_clip_request),
        )
        .route(
            "/api/v1/live/streams/:stream_id/reports",
            post(live::report_live_stream).get(live::list_live_stream_reports),
        )
        .route(
            "/api/v1/live/streams/:stream_id/reports/:report_id/resolve",
            patch(live::resolve_live_stream_report),
        )
        .route(
            "/api/v1/live/streams/:stream_id/viewer-preview",
            get(live::get_live_viewer_preview),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderators",
            get(live::list_live_stream_moderators).post(live::add_live_stream_moderator),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderators/:user_id",
            delete(live::remove_live_stream_moderator),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/actions",
            get(live::list_live_moderation_actions).post(live::create_live_moderation_action),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/actions/:action_id",
            get(live::get_live_moderation_action),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/actions/:action_id/revoke",
            post(live::revoke_live_moderation_action),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/actions/:action_id/reconcile",
            post(live::reconcile_live_moderation_action),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/audit",
            get(live::list_live_moderation_audit_log),
        )
        .route("/api/v1/live/discovery", get(live::get_live_discovery))
        .route("/api/v1/categories", get(catalog::list_categories))
        .route("/api/v1/categories/:slug", get(catalog::get_category))
        .route(
            "/api/v1/categories/:slug/browse",
            get(catalog::get_category_browse),
        )
        .route("/api/v1/streamers", get(catalog::list_streamers))
        .route("/api/v1/streamers/:id", get(catalog::get_streamer))
        .route("/api/v1/search", get(catalog::search))
        .route("/api/v1/people/:slug", get(people::get_person_profile))
}
