use super::*;

async fn stream_viewers(pool: &SqlitePool, stream_id: &str) -> AppResult<i64> {
    let row = sqlx::query("SELECT viewers FROM live_streams WHERE id = ?")
        .bind(stream_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(row.get("viewers"))
}

pub(crate) async fn effective_live_viewer_count(
    pool: &SqlitePool,
    stream_id: &str,
) -> AppResult<i64> {
    let reported = stream_viewers(pool, stream_id).await?;
    let connected = count_active_live_viewer_sessions(pool, stream_id).await?;
    Ok(reported.max(connected))
}

pub(crate) async fn count_active_live_viewer_sessions(
    pool: &SqlitePool,
    stream_id: &str,
) -> AppResult<i64> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS count
        FROM (
            SELECT COALESCE('u:' || user_id, 's:' || session_token_hash) AS viewer_key
            FROM live_viewer_sessions
            WHERE stream_id = ?
              AND disconnected_at IS NULL
              AND last_seen_at >= ?
            GROUP BY viewer_key
        ) active_viewers
        "#,
    )
    .bind(stream_id)
    .bind(active_presence_cutoff())
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}

pub(crate) async fn count_all_active_live_viewer_sessions(pool: &SqlitePool) -> AppResult<i64> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS count
        FROM (
            SELECT stream_id, COALESCE('u:' || user_id, 's:' || session_token_hash) AS viewer_key
            FROM live_viewer_sessions
            WHERE disconnected_at IS NULL
              AND last_seen_at >= ?
            GROUP BY stream_id, viewer_key
        ) active_viewers
        "#,
    )
    .bind(active_presence_cutoff())
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}

pub(crate) async fn fetch_live_viewer_sample_users(
    pool: &SqlitePool,
    stream_id: &str,
    limit: i64,
) -> AppResult<Vec<String>> {
    let rows = sqlx::query(
        r#"
        SELECT u.handle
        FROM live_viewer_sessions lvs
        JOIN users u ON u.id = lvs.user_id
        WHERE lvs.stream_id = ?
          AND lvs.user_id IS NOT NULL
          AND lvs.disconnected_at IS NULL
          AND lvs.last_seen_at >= ?
        GROUP BY u.id, u.handle
        ORDER BY MAX(lvs.last_seen_at) DESC
        LIMIT ?
        "#,
    )
    .bind(stream_id)
    .bind(active_presence_cutoff())
    .bind(limit.max(1))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.get("handle")).collect())
}

pub(crate) async fn register_live_viewer_session(
    pool: &SqlitePool,
    stream_id: &str,
    identity: Option<&RequestIdentity>,
    session_token: Option<&str>,
) -> AppResult<(String, bool, String)> {
    let now = Utc::now().to_rfc3339();
    if let Some(token) = session_token.filter(|value| !value.trim().is_empty()) {
        let token_hash = hash_token(token);
        let existing = sqlx::query(
            r#"
            SELECT user_id
            FROM live_viewer_sessions
            WHERE stream_id = ? AND session_token_hash = ?
            ORDER BY connected_at DESC
            LIMIT 1
            "#,
        )
        .bind(&stream_id)
        .bind(&token_hash)
        .fetch_optional(pool)
        .await?;
        if let Some(row) = existing {
            let bound_user_id = row.get::<Option<String>, _>("user_id");
            let requested_user_id = identity.map(|item| item.user_id.as_str());
            if bound_user_id
                .as_deref()
                .is_some_and(|bound| Some(bound) != requested_user_id)
            {
                return Err(AppError::Forbidden);
            }

            let result = sqlx::query(
                r#"
                UPDATE live_viewer_sessions
                SET user_id = COALESCE(?, user_id),
                    connected_at = ?,
                    last_seen_at = ?,
                    disconnected_at = NULL
                WHERE stream_id = ? AND session_token_hash = ?
                "#,
            )
            .bind(requested_user_id)
            .bind(&now)
            .bind(&now)
            .bind(&stream_id)
            .bind(&token_hash)
            .execute(pool)
            .await?;
            if result.rows_affected() > 0 {
                return Ok((token.to_string(), true, now));
            }
        }
    }

    let raw_token = format!(
        "wss_{}_{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    sqlx::query(
        r#"
        INSERT INTO live_viewer_sessions (
            id, stream_id, user_id, session_token_hash, connected_at, last_seen_at, disconnected_at
        ) VALUES (?, ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(format!("lvs-{}", Uuid::new_v4().simple()))
    .bind(stream_id)
    .bind(identity.map(|item| item.user_id.as_str()))
    .bind(hash_token(&raw_token))
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok((raw_token, false, now))
}

pub(crate) async fn touch_live_viewer_session(
    pool: &SqlitePool,
    stream_id: &str,
    session_token: &str,
    lease_connected_at: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE live_viewer_sessions SET last_seen_at = ?, disconnected_at = NULL WHERE stream_id = ? AND session_token_hash = ? AND connected_at = ?",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(stream_id)
    .bind(hash_token(session_token))
    .bind(lease_connected_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn disconnect_live_viewer_session(
    pool: &SqlitePool,
    stream_id: &str,
    session_token: &str,
    lease_connected_at: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE live_viewer_sessions SET last_seen_at = ?, disconnected_at = ? WHERE stream_id = ? AND session_token_hash = ? AND connected_at = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(stream_id)
    .bind(hash_token(session_token))
    .bind(lease_connected_at)
    .execute(pool)
    .await?;
    Ok(())
}
