use super::discovery::{
    fetch_billing_plan, fetch_followed_streamer_ids, fetch_live_streams, fetch_streamer_by_id,
    fetch_user, fetch_user_library, fetch_user_profile_details, fetch_user_settings_bundle,
    fetch_viewer_app_state, fetch_watchlist_response, resolve_progress_target,
    validate_watchlist_content,
};
use super::notifications::{list_my_notifications, mark_my_notification_read};
use super::realtime::auth_session_channel_id;
use super::*;

mod entitlements;
mod profile;
mod sessions;
mod state;
mod watch;

pub(crate) use entitlements::{
    get_my_membership_entitlement, get_my_purchase_entitlement,
    reconcile_my_membership_entitlement, reconcile_my_purchase_entitlement,
};
pub(crate) use sessions::revoke_session;

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/api/v1/me", get(state::get_me))
        .route("/api/v1/me/state", get(state::get_my_state))
        .route("/api/v1/me/library", get(state::get_my_library))
        .route("/api/v1/me/entitlements", get(state::get_my_entitlements))
        .route(
            "/api/v1/me/entitlements/memberships/:creator_id",
            get(entitlements::get_my_membership_entitlement),
        )
        .route(
            "/api/v1/me/entitlements/memberships/:creator_id/reconcile",
            post(entitlements::reconcile_my_membership_entitlement),
        )
        .route(
            "/api/v1/me/entitlements/purchases/:purchase_id",
            get(entitlements::get_my_purchase_entitlement),
        )
        .route(
            "/api/v1/me/entitlements/purchases/:purchase_id/reconcile",
            post(entitlements::reconcile_my_purchase_entitlement),
        )
        .route("/api/v1/me/watchlist", get(state::get_my_watchlist))
        .route("/api/v1/me/notifications", get(list_my_notifications))
        .route(
            "/api/v1/me/notifications/:notification_id/read",
            post(mark_my_notification_read),
        )
        .route(
            "/api/v1/me/profile",
            get(profile::get_my_profile).patch(profile::update_my_profile),
        )
        .route(
            "/api/v1/me/settings",
            get(profile::get_my_settings).patch(profile::update_my_settings),
        )
        .route("/api/v1/me/plan", get(sessions::get_my_plan))
        .route(
            "/api/v1/me/sessions",
            get(sessions::list_sessions).post(sessions::create_session),
        )
        .route("/api/v1/me/sessions/:id", delete(sessions::revoke_session))
        .route(
            "/api/v1/me/watchlist/:content_id",
            post(watch::add_watchlist).delete(watch::remove_watchlist),
        )
        .route(
            "/api/v1/me/following/:streamer_id",
            post(watch::add_following).delete(watch::remove_following),
        )
        .route("/api/v1/me/following", get(watch::get_my_following_feed))
        .route("/api/v1/me/progress", put(watch::record_progress))
        .route(
            "/api/v1/me/progress/:content_id",
            delete(watch::remove_progress),
        )
        .route(
            "/api/v1/me/history/:content_id",
            delete(watch::remove_history_entry),
        )
}
