use super::*;

const VIEWER_APP_STATE_NOTIFICATIONS_LIMIT: usize = 20;
const VIEWER_APP_STATE_SESSIONS_LIMIT: usize = 8;
const VIEWER_APP_STATE_HISTORY_LIMIT: usize = 20;
const VIEWER_APP_STATE_CONTINUE_WATCHING_LIMIT: usize = 12;

pub(crate) async fn fetch_viewer_app_state(
    pool: &SqlitePool,
    user_id: &str,
    current_session_id: &str,
) -> AppResult<ViewerAppState> {
    let user = fetch_user(pool, user_id).await?;
    let mut library = fetch_user_library(pool, user_id).await?;
    library
        .continue_watching
        .truncate(VIEWER_APP_STATE_CONTINUE_WATCHING_LIMIT);
    library.history.truncate(VIEWER_APP_STATE_HISTORY_LIMIT);
    let watchlist = fetch_watchlist_response(pool, user_id).await?;

    let followed_streamer_ids = fetch_followed_streamer_ids(pool, user_id).await?;
    let mut followed_streamers = Vec::with_capacity(followed_streamer_ids.len());
    for streamer_id in &followed_streamer_ids {
        followed_streamers.push(fetch_streamer_by_id(pool, streamer_id).await?);
    }
    let followed_streamer_id_set: std::collections::HashSet<_> =
        followed_streamer_ids.into_iter().collect();
    let live_streams: Vec<LiveStream> = fetch_live_streams(pool, None)
        .await?
        .into_iter()
        .filter(|stream| followed_streamer_id_set.contains(&stream.streamer.id))
        .collect();
    let following = FollowingFeedResponse {
        total_followed_streamers: followed_streamers.len() as i64,
        live_now_count: live_streams.len() as i64,
        followed_streamers,
        live_streams,
    };

    Ok(ViewerAppState {
        user,
        library,
        watchlist,
        following,
        entitlements: fetch_user_entitlements(pool, user_id).await?,
        profile: fetch_user_profile_details(pool, user_id).await?,
        settings: fetch_user_settings_bundle(pool, user_id).await?,
        plan: fetch_billing_plan(pool, user_id).await?,
        notifications: fetch_user_notifications(pool, user_id)
            .await?
            .into_iter()
            .take(VIEWER_APP_STATE_NOTIFICATIONS_LIMIT)
            .collect(),
        sessions: fetch_auth_sessions(pool, user_id, current_session_id)
            .await?
            .into_iter()
            .take(VIEWER_APP_STATE_SESSIONS_LIMIT)
            .collect(),
    })
}
