use super::*;

pub(crate) async fn get_my_following_feed(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<FollowingFeedResponse>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_following_feed_response(&state.pool, &identity.user_id).await?,
    ))
}

pub(crate) async fn add_watchlist(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(content_id): Path<String>,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.pool, &headers).await?;
    validate_watchlist_content(&state.pool, &content_id).await?;
    sqlx::query("INSERT OR IGNORE INTO user_watchlist (user_id, content_id) VALUES (?, ?)")
        .bind(&identity.user_id)
        .bind(content_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(fetch_user(&state.pool, &identity.user_id).await?))
}

pub(crate) async fn remove_watchlist(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(content_id): Path<String>,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.pool, &headers).await?;
    sqlx::query("DELETE FROM user_watchlist WHERE user_id = ? AND content_id = ?")
        .bind(&identity.user_id)
        .bind(content_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(fetch_user(&state.pool, &identity.user_id).await?))
}

pub(crate) async fn add_following(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(streamer_id): Path<String>,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.pool, &headers).await?;
    fetch_streamer_by_id(&state.pool, &streamer_id).await?;
    sqlx::query("INSERT OR IGNORE INTO user_following (user_id, streamer_id) VALUES (?, ?)")
        .bind(&identity.user_id)
        .bind(streamer_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(fetch_user(&state.pool, &identity.user_id).await?))
}

pub(crate) async fn remove_following(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(streamer_id): Path<String>,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.pool, &headers).await?;
    sqlx::query("DELETE FROM user_following WHERE user_id = ? AND streamer_id = ?")
        .bind(&identity.user_id)
        .bind(streamer_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(fetch_user(&state.pool, &identity.user_id).await?))
}

pub(crate) async fn record_progress(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<ProgressInput>,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.pool, &headers).await?;
    if input.progress_sec < 0 {
        return Err(AppError::BadRequest("progressSec must be >= 0".to_string()));
    }
    let progress_target = resolve_progress_target(&state.pool, &input).await?;
    let canonical_duration_sec = progress_target.duration_sec;
    let normalized_progress_sec = input.progress_sec.min(canonical_duration_sec);
    let watched_at = Utc::now().to_rfc3339();
    let progress_kind = progress_target.kind.clone();
    let progress_episode_id = progress_target.episode_id.clone();

    if normalized_progress_sec >= canonical_duration_sec {
        sqlx::query("DELETE FROM continue_watching WHERE user_id = ? AND content_id = ?")
            .bind(&identity.user_id)
            .bind(&input.content_id)
            .execute(&state.pool)
            .await?;
        upsert_watch_history_entry(
            &state.pool,
            &identity.user_id,
            &input.content_id,
            &progress_target.kind,
            progress_target.episode_id.as_deref(),
            canonical_duration_sec,
            canonical_duration_sec,
            true,
            &watched_at,
        )
        .await?;
        return Ok(Json(fetch_user(&state.pool, &identity.user_id).await?));
    }

    sqlx::query(
        r#"
        INSERT INTO continue_watching (user_id, content_id, kind, episode_id, progress_sec, duration_sec, last_watched_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(user_id, content_id) DO UPDATE SET
            kind = excluded.kind,
            episode_id = excluded.episode_id,
            progress_sec = excluded.progress_sec,
            duration_sec = excluded.duration_sec,
            last_watched_at = excluded.last_watched_at
        "#,
    )
    .bind(&identity.user_id)
    .bind(&input.content_id)
    .bind(&progress_kind)
    .bind(&progress_episode_id)
    .bind(normalized_progress_sec)
    .bind(canonical_duration_sec)
    .bind(&watched_at)
    .execute(&state.pool)
    .await?;
    upsert_watch_history_entry(
        &state.pool,
        &identity.user_id,
        &input.content_id,
        &progress_kind,
        progress_episode_id.as_deref(),
        normalized_progress_sec,
        canonical_duration_sec,
        false,
        &watched_at,
    )
    .await?;

    Ok(Json(fetch_user(&state.pool, &identity.user_id).await?))
}

pub(crate) async fn remove_progress(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(content_id): Path<String>,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.pool, &headers).await?;
    sqlx::query("DELETE FROM continue_watching WHERE user_id = ? AND content_id = ?")
        .bind(&identity.user_id)
        .bind(content_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(fetch_user(&state.pool, &identity.user_id).await?))
}

pub(crate) async fn remove_history_entry(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(content_id): Path<String>,
) -> AppResult<Json<UserLibrary>> {
    let identity = require_identity(&state.pool, &headers).await?;
    sqlx::query("DELETE FROM user_watch_history WHERE user_id = ? AND content_id = ?")
        .bind(&identity.user_id)
        .bind(content_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(
        fetch_user_library(&state.pool, &identity.user_id).await?,
    ))
}
