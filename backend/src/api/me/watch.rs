use super::*;

pub(crate) async fn get_my_following_feed(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<FollowingFeedResponse>> {
    let identity = require_identity(&state.db, &headers).await?;
    if state.database_kind == crate::config::DatabaseKind::Postgres {
        return Ok(Json(
            super::state::build_postgres_viewer_app_state(
                &state.db,
                &identity.user_id,
                &identity.session_id,
            )
            .await?
            .following,
        ));
    }
    Ok(Json(
        fetch_following_feed_response(state.db.sqlite_adapter(), &identity.user_id).await?,
    ))
}

pub(crate) async fn add_watchlist(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(content_id): Path<String>,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.db, &headers).await?;
    state
        .db
        .add_watchlist_item(&identity.user_id, &content_id)
        .await?;
    Ok(Json(state.db.fetch_user(&identity.user_id).await?))
}

pub(crate) async fn remove_watchlist(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(content_id): Path<String>,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.db, &headers).await?;
    state
        .db
        .remove_watchlist_item(&identity.user_id, &content_id)
        .await?;
    Ok(Json(state.db.fetch_user(&identity.user_id).await?))
}

pub(crate) async fn add_following(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(streamer_id): Path<String>,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.db, &headers).await?;
    state
        .db
        .add_following(&identity.user_id, &streamer_id)
        .await?;
    Ok(Json(state.db.fetch_user(&identity.user_id).await?))
}

pub(crate) async fn remove_following(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(streamer_id): Path<String>,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.db, &headers).await?;
    state
        .db
        .remove_following(&identity.user_id, &streamer_id)
        .await?;
    Ok(Json(state.db.fetch_user(&identity.user_id).await?))
}

pub(crate) async fn record_progress(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<ProgressInput>,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.db, &headers).await?;
    let watched_at = Utc::now().to_rfc3339();
    state
        .db
        .record_progress(&identity.user_id, &input, &watched_at)
        .await?;

    Ok(Json(state.db.fetch_user(&identity.user_id).await?))
}

pub(crate) async fn remove_progress(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(content_id): Path<String>,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.db, &headers).await?;
    state
        .db
        .remove_progress(&identity.user_id, &content_id)
        .await?;
    Ok(Json(state.db.fetch_user(&identity.user_id).await?))
}

pub(crate) async fn remove_history_entry(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(content_id): Path<String>,
) -> AppResult<Json<UserLibrary>> {
    let identity = require_identity(&state.db, &headers).await?;
    state
        .db
        .remove_history_entry(&identity.user_id, &content_id)
        .await?;
    if state.database_kind == crate::config::DatabaseKind::Postgres {
        return Ok(Json(
            super::state::build_postgres_viewer_app_state(
                &state.db,
                &identity.user_id,
                &identity.session_id,
            )
            .await?
            .library,
        ));
    }
    Ok(Json(
        fetch_user_library(state.db.sqlite_adapter(), &identity.user_id).await?,
    ))
}
