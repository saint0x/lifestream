use super::*;
#[derive(Clone)]
pub(crate) struct UserRecord {
    pub(crate) id: String,
    pub(crate) handle: String,
    pub(crate) display_name: String,
    pub(crate) avatar: String,
    pub(crate) tier: String,
    pub(crate) joined_at: String,
}

pub(crate) async fn fetch_user(pool: &SqlitePool, user_id: &str) -> AppResult<User> {
    let (record, watchlist, following, continue_watching) = tokio::try_join!(
        fetch_user_record(pool, user_id),
        fetch_watchlist_ids(pool, user_id),
        fetch_followed_streamer_ids(pool, user_id),
        fetch_continue_watching_entries(pool, user_id),
    )?;

    Ok(build_user_from_parts(
        record,
        watchlist,
        following,
        continue_watching,
    ))
}

pub(crate) fn build_user_from_parts(
    record: UserRecord,
    watchlist: Vec<String>,
    following: Vec<String>,
    continue_watching: Vec<ContinueWatchingEntry>,
) -> User {
    User {
        id: record.id,
        handle: record.handle,
        display_name: record.display_name,
        avatar: record.avatar,
        tier: record.tier,
        joined_at: record.joined_at,
        watchlist,
        following,
        continue_watching,
    }
}

pub(crate) async fn fetch_watch_history(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<Vec<WatchHistoryEntry>> {
    fetch_watch_history_limited(pool, user_id, None).await
}

pub(crate) async fn fetch_user_library(pool: &SqlitePool, user_id: &str) -> AppResult<UserLibrary> {
    let (continue_watching, history, entitlements) = tokio::try_join!(
        fetch_continue_watching_entries(pool, user_id),
        fetch_watch_history(pool, user_id),
        fetch_user_entitlements(pool, user_id),
    )?;
    Ok(UserLibrary {
        continue_watching,
        history,
        memberships: entitlements.memberships,
        purchases: entitlements.purchases,
    })
}

pub(crate) async fn fetch_watchlist_response(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<WatchlistResponse> {
    let watchlist_ids = fetch_watchlist_ids(pool, user_id).await?;
    if watchlist_ids.is_empty() {
        return Ok(WatchlistResponse {
            total_titles: 0,
            series: Vec::new(),
            films: Vec::new(),
        });
    }

    let series_ids = fetch_existing_series_watchlist_ids(pool, &watchlist_ids).await?;
    let series_id_set = series_ids.iter().cloned().collect::<std::collections::HashSet<_>>();
    let film_ids = watchlist_ids
        .iter()
        .filter(|id| !series_id_set.contains(*id))
        .cloned()
        .collect::<Vec<_>>();

    let (series, films) = tokio::try_join!(
        fetch_series_previews_by_ids(pool, &series_ids),
        fetch_films_by_ids(pool, &film_ids),
    )?;

    if series.len() + films.len() != watchlist_ids.len() {
        return Err(AppError::NotFound);
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

pub(crate) async fn fetch_following_feed_response(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<FollowingFeedResponse> {
    let (followed_streamers, live_streams) = tokio::try_join!(
        fetch_followed_streamers(pool, user_id),
        fetch_followed_live_streams(pool, user_id),
    )?;

    Ok(FollowingFeedResponse {
        total_followed_streamers: followed_streamers.len() as i64,
        live_now_count: live_streams.len() as i64,
        followed_streamers,
        live_streams,
    })
}

pub(crate) async fn fetch_watch_history_limited(
    pool: &SqlitePool,
    user_id: &str,
    limit: Option<usize>,
) -> AppResult<Vec<WatchHistoryEntry>> {
    let mut query = String::from(
        r#"
        SELECT content_id, kind, episode_id, progress_sec, duration_sec,
               completed, completed_at, last_watched_at
        FROM user_watch_history
        WHERE user_id = ?
        ORDER BY last_watched_at DESC
        "#,
    );
    if let Some(limit) = limit {
        query.push_str(&format!(" LIMIT {}", limit.max(1)));
    }
    Ok(sqlx::query(
        &query,
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

async fn fetch_watchlist_ids(pool: &SqlitePool, user_id: &str) -> AppResult<Vec<String>> {
    Ok(sqlx::query(
        "SELECT content_id FROM user_watchlist WHERE user_id = ? ORDER BY content_id ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|item| item.get("content_id"))
    .collect())
}

pub(crate) async fn fetch_user_record(pool: &SqlitePool, user_id: &str) -> AppResult<UserRecord> {
    let row = sqlx::query(
        "SELECT id, handle, display_name, avatar, tier, joined_at FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(UserRecord {
        id: row.get("id"),
        handle: row.get("handle"),
        display_name: row.get("display_name"),
        avatar: row.get("avatar"),
        tier: row.get("tier"),
        joined_at: row.get("joined_at"),
    })
}

pub(crate) fn watchlist_ids_from_response(response: &WatchlistResponse) -> Vec<String> {
    response
        .series
        .iter()
        .map(|item| item.id.clone())
        .chain(response.films.iter().map(|item| item.id.clone()))
        .collect()
}

pub(crate) fn followed_streamer_ids_from_response(response: &FollowingFeedResponse) -> Vec<String> {
    response
        .followed_streamers
        .iter()
        .map(|item| item.id.clone())
        .collect()
}

pub(crate) async fn fetch_continue_watching_entries(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<Vec<ContinueWatchingEntry>> {
    fetch_continue_watching_entries_limited(pool, user_id, None).await
}

pub(crate) async fn fetch_continue_watching_entries_limited(
    pool: &SqlitePool,
    user_id: &str,
    limit: Option<usize>,
) -> AppResult<Vec<ContinueWatchingEntry>> {
    let mut query = String::from(
        "SELECT content_id, kind, episode_id, progress_sec, duration_sec, last_watched_at FROM continue_watching WHERE user_id = ? ORDER BY last_watched_at DESC",
    );
    if let Some(limit) = limit {
        query.push_str(&format!(" LIMIT {}", limit.max(1)));
    }
    Ok(sqlx::query(&query)
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
    .collect())
}

async fn fetch_followed_streamers(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<Vec<Streamer>> {
    Ok(
        sqlx::query(
            r#"
            SELECT s.id, s.handle, s.display_name, s.avatar, s.bio, s.followers, s.is_partner, s.is_live
            FROM user_following uf
            JOIN streamers s ON s.id = uf.streamer_id
            WHERE uf.user_id = ?
            ORDER BY uf.streamer_id
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(streamer_from_row)
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

async fn fetch_existing_series_watchlist_ids(
    pool: &SqlitePool,
    watchlist_ids: &[String],
) -> AppResult<Vec<String>> {
    if watchlist_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = vec!["?"; watchlist_ids.len()].join(", ");
    let query =
        format!("SELECT id FROM series WHERE id IN ({placeholders}) ORDER BY id ASC");
    let mut statement = sqlx::query(&query);
    for id in watchlist_ids {
        statement = statement.bind(id);
    }
    Ok(statement
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| row.get("id"))
        .collect())
}
