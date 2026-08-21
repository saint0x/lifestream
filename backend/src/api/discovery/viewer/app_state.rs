use super::*;
use crate::api::discovery::viewer::fetch_connected_accounts;
use crate::api::notifications::fetch_user_notifications_limited;
use crate::api::presence::fetch_auth_sessions_limited;

const VIEWER_APP_STATE_NOTIFICATIONS_LIMIT: usize = 20;
const VIEWER_APP_STATE_SESSIONS_LIMIT: usize = 8;
const VIEWER_APP_STATE_HISTORY_LIMIT: usize = 20;
const VIEWER_APP_STATE_CONTINUE_WATCHING_LIMIT: usize = 12;

pub(crate) async fn fetch_viewer_app_state(
    pool: &SqlitePool,
    user_id: &str,
    current_session_id: &str,
) -> AppResult<ViewerAppState> {
    let (
        user_record,
        continue_watching,
        history,
        watchlist,
        following,
        entitlements,
        account_bundle,
    ) = tokio::try_join!(
        fetch_user_record(pool, user_id),
        fetch_continue_watching_entries_limited(pool, user_id, Some(VIEWER_APP_STATE_CONTINUE_WATCHING_LIMIT)),
        fetch_watch_history_limited(pool, user_id, Some(VIEWER_APP_STATE_HISTORY_LIMIT)),
        fetch_watchlist_response(pool, user_id),
        fetch_following_feed_response(pool, user_id),
        fetch_user_entitlements(pool, user_id),
        fetch_viewer_account_bundle(pool, user_id),
    )?;
    let (notifications, sessions, connected_accounts): (
        Vec<UserNotification>,
        Vec<AuthSession>,
        Vec<ConnectedAccount>,
    ) = tokio::try_join!(
        fetch_user_notifications_limited(pool, user_id, Some(VIEWER_APP_STATE_NOTIFICATIONS_LIMIT)),
        fetch_auth_sessions_limited(pool, user_id, current_session_id, Some(VIEWER_APP_STATE_SESSIONS_LIMIT)),
        fetch_connected_accounts(pool, user_id),
    )?;
    let user = build_user_from_parts(
        user_record,
        watchlist_ids_from_response(&watchlist),
        followed_streamer_ids_from_response(&following),
        continue_watching.clone(),
    );
    let profile =
        user_profile_details_from_bundle(user.clone(), account_bundle.profile.clone(), connected_accounts);
    let settings = user_settings_bundle_from_account_bundle(account_bundle.clone());
    let plan = account_bundle.plan.clone();
    let library = UserLibrary {
        continue_watching,
        history,
        memberships: entitlements.memberships.clone(),
        purchases: entitlements.purchases.clone(),
    };

    Ok(ViewerAppState {
        user,
        library,
        watchlist,
        following,
        entitlements,
        profile,
        settings,
        plan,
        notifications,
        sessions,
    })
}
