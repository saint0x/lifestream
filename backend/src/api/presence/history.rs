use super::*;

pub(crate) async fn fetch_auth_sessions(
    pool: &SqlitePool,
    user_id: &str,
    current_session_id: &str,
) -> AppResult<Vec<AuthSession>> {
    fetch_auth_sessions_limited(pool, user_id, current_session_id, None).await
}

pub(crate) async fn fetch_auth_sessions_limited(
    pool: &SqlitePool,
    user_id: &str,
    current_session_id: &str,
    limit: Option<usize>,
) -> AppResult<Vec<AuthSession>> {
    let mut query = String::from(
        r#"
        SELECT id, label, scopes_json, created_at, expires_at, revoked_at, last_used_at
        FROM auth_sessions
        WHERE user_id = ?
        ORDER BY
            CASE WHEN id = ? THEN 0 ELSE 1 END,
            created_at DESC
        "#,
    );
    if let Some(limit) = limit {
        query.push_str(&format!(" LIMIT {}", limit.max(1)));
    }
    let rows = sqlx::query(&query)
    .bind(user_id)
    .bind(current_session_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| AuthSession {
            id: row.get("id"),
            label: row.get("label"),
            scopes: from_json(row.get::<String, _>("scopes_json")).unwrap_or_default(),
            created_at: row.get("created_at"),
            expires_at: row.get("expires_at"),
            revoked_at: row.get("revoked_at"),
            last_used_at: row.get("last_used_at"),
            is_current: row.get::<String, _>("id") == current_session_id,
        })
        .collect())
}

pub(crate) async fn fetch_continue_watching_entry(
    pool: &SqlitePool,
    user_id: &str,
    content_id: Option<&str>,
    slug_or_id: &str,
) -> AppResult<Option<ContinueWatchingEntry>> {
    let target_content_id = match content_id {
        Some(id) => id.to_string(),
        None => {
            if let Some(row) = sqlx::query("SELECT id FROM series WHERE slug = ?")
                .bind(slug_or_id)
                .fetch_optional(pool)
                .await?
            {
                row.get("id")
            } else if let Some(row) = sqlx::query("SELECT id FROM films WHERE slug = ?")
                .bind(slug_or_id)
                .fetch_optional(pool)
                .await?
            {
                row.get("id")
            } else {
                return Ok(None);
            }
        }
    };

    let row = sqlx::query(
        "SELECT content_id, kind, episode_id, progress_sec, duration_sec, last_watched_at FROM continue_watching WHERE user_id = ? AND content_id = ?",
    )
    .bind(user_id)
    .bind(target_content_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|item| ContinueWatchingEntry {
        content_id: item.get("content_id"),
        kind: item.get("kind"),
        episode_id: item.get("episode_id"),
        progress_sec: item.get("progress_sec"),
        duration_sec: item.get("duration_sec"),
        last_watched_at: item.get("last_watched_at"),
    }))
}

pub(crate) async fn upsert_watch_history_entry(
    pool: &SqlitePool,
    user_id: &str,
    content_id: &str,
    kind: &str,
    episode_id: Option<&str>,
    progress_sec: i64,
    duration_sec: i64,
    completed: bool,
    watched_at: &str,
) -> AppResult<()> {
    let completed_at = if completed {
        Some(watched_at.to_string())
    } else {
        None
    };
    sqlx::query(
        r#"
        INSERT INTO user_watch_history (
            user_id, content_id, kind, episode_id, progress_sec, duration_sec,
            completed, completed_at, last_watched_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(user_id, content_id) DO UPDATE SET
            kind = excluded.kind,
            episode_id = excluded.episode_id,
            progress_sec = excluded.progress_sec,
            duration_sec = excluded.duration_sec,
            completed = excluded.completed,
            completed_at = excluded.completed_at,
            last_watched_at = excluded.last_watched_at
        "#,
    )
    .bind(user_id)
    .bind(content_id)
    .bind(kind)
    .bind(episode_id)
    .bind(progress_sec)
    .bind(duration_sec)
    .bind(completed as i64)
    .bind(completed_at)
    .bind(watched_at)
    .execute(pool)
    .await?;
    Ok(())
}
