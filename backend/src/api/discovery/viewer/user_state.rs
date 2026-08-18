use super::*;

pub(crate) async fn fetch_user(pool: &SqlitePool, user_id: &str) -> AppResult<User> {
    let row = sqlx::query(
        "SELECT id, handle, display_name, avatar, tier, joined_at FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let watchlist = sqlx::query("SELECT content_id FROM user_watchlist WHERE user_id = ?")
        .bind(user_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|item| item.get("content_id"))
        .collect();

    let following = sqlx::query("SELECT streamer_id FROM user_following WHERE user_id = ?")
        .bind(user_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|item| item.get("streamer_id"))
        .collect();

    let continue_watching = sqlx::query(
        "SELECT content_id, kind, episode_id, progress_sec, duration_sec, last_watched_at FROM continue_watching WHERE user_id = ? ORDER BY last_watched_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|item| ContinueWatchingEntry {
        content_id: item.get("content_id"),
        kind: item.get("kind"),
        episode_id: item.get("episode_id"),
        progress_sec: item.get("progress_sec"),
        duration_sec: item.get("duration_sec"),
        last_watched_at: item.get("last_watched_at"),
    })
    .collect();

    Ok(User {
        id: row.get("id"),
        handle: row.get("handle"),
        display_name: row.get("display_name"),
        avatar: row.get("avatar"),
        tier: row.get("tier"),
        joined_at: row.get("joined_at"),
        watchlist,
        following,
        continue_watching,
    })
}

pub(crate) async fn fetch_watch_history(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<Vec<WatchHistoryEntry>> {
    Ok(sqlx::query(
        r#"
        SELECT content_id, kind, episode_id, progress_sec, duration_sec,
               completed, completed_at, last_watched_at
        FROM user_watch_history
        WHERE user_id = ?
        ORDER BY last_watched_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|item| WatchHistoryEntry {
        content_id: item.get("content_id"),
        kind: item.get("kind"),
        episode_id: item.get("episode_id"),
        progress_sec: item.get("progress_sec"),
        duration_sec: item.get("duration_sec"),
        completed: item.get::<i64, _>("completed") == 1,
        completed_at: item.get("completed_at"),
        last_watched_at: item.get("last_watched_at"),
    })
    .collect())
}

pub(crate) async fn fetch_user_library(pool: &SqlitePool, user_id: &str) -> AppResult<UserLibrary> {
    let user = fetch_user(pool, user_id).await?;
    let entitlements = fetch_user_entitlements(pool, user_id).await?;
    Ok(UserLibrary {
        continue_watching: user.continue_watching,
        history: fetch_watch_history(pool, user_id).await?,
        memberships: entitlements.memberships,
        purchases: entitlements.purchases,
    })
}

pub(crate) async fn fetch_watchlist_response(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<WatchlistResponse> {
    let watchlist_ids: Vec<String> = sqlx::query(
        "SELECT content_id FROM user_watchlist WHERE user_id = ? ORDER BY content_id ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|item| item.get("content_id"))
    .collect();

    let mut series = Vec::new();
    let mut films = Vec::new();
    for content_id in watchlist_ids {
        if let Ok(item) = fetch_series_by_id(pool, &content_id, None).await {
            series.push(item);
            continue;
        }
        if let Ok(item) = fetch_film_by_id(pool, &content_id, None).await {
            films.push(item);
        }
    }

    Ok(WatchlistResponse {
        total_titles: (series.len() + films.len()) as i64,
        series,
        films,
    })
}

pub(crate) async fn fetch_followed_streamer_ids(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<Vec<String>> {
    Ok(
        sqlx::query(
            "SELECT streamer_id FROM user_following WHERE user_id = ? ORDER BY streamer_id",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|item| item.get("streamer_id"))
        .collect(),
    )
}

pub(crate) async fn fetch_creator_id_for_user(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<Option<String>> {
    let row = sqlx::query("SELECT id FROM creator_profiles WHERE user_id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| row.get("id")))
}
