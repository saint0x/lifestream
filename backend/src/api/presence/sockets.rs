use super::*;

pub(crate) async fn register_creator_live_socket_session(
    pool: &SqlitePool,
    creator_id: &str,
    user_id: &str,
    session_token: Option<&str>,
) -> AppResult<(String, bool, String)> {
    let now = Utc::now().to_rfc3339();
    if let Some(token) = session_token.filter(|value| !value.trim().is_empty()) {
        let token_hash = hash_token(token);
        let result = sqlx::query(
            r#"
            UPDATE creator_live_socket_sessions
            SET connected_at = ?,
                last_seen_at = ?,
                disconnected_at = NULL
            WHERE creator_id = ? AND user_id = ? AND session_token_hash = ?
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(creator_id)
        .bind(user_id)
        .bind(&token_hash)
        .execute(pool)
        .await?;
        if result.rows_affected() > 0 {
            return Ok((token.to_string(), true, now));
        }
    }

    let raw_token = format!(
        "cws_{}_{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    sqlx::query(
        r#"
        INSERT INTO creator_live_socket_sessions (
            id, creator_id, user_id, session_token_hash, connected_at, last_seen_at, disconnected_at
        ) VALUES (?, ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(format!("cls-{}", Uuid::new_v4().simple()))
    .bind(creator_id)
    .bind(user_id)
    .bind(hash_token(&raw_token))
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok((raw_token, false, now))
}

pub(crate) async fn touch_creator_live_socket_session(
    pool: &SqlitePool,
    creator_id: &str,
    session_token: &str,
    lease_connected_at: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE creator_live_socket_sessions SET last_seen_at = ?, disconnected_at = NULL WHERE creator_id = ? AND session_token_hash = ? AND connected_at = ?",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(creator_id)
    .bind(hash_token(session_token))
    .bind(lease_connected_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn disconnect_creator_live_socket_session(
    pool: &SqlitePool,
    creator_id: &str,
    session_token: &str,
    lease_connected_at: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE creator_live_socket_sessions SET last_seen_at = ?, disconnected_at = ? WHERE creator_id = ? AND session_token_hash = ? AND connected_at = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(creator_id)
    .bind(hash_token(session_token))
    .bind(lease_connected_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn register_collaboration_socket_session(
    pool: &SqlitePool,
    session: &CollaborationSessionView,
    identity: &RequestIdentity,
    session_token: Option<&str>,
) -> AppResult<(String, bool, String)> {
    let now = Utc::now().to_rfc3339();
    if let Some(token) = session_token.filter(|value| !value.trim().is_empty()) {
        let token_hash = hash_token(token);
        let result = sqlx::query(
            r#"
            UPDATE collaboration_socket_sessions
            SET participant_id = ?,
                creator_id = COALESCE(?, creator_id),
                connected_at = ?,
                last_seen_at = ?,
                disconnected_at = NULL
            WHERE collaboration_session_id = ? AND user_id = ? AND session_token_hash = ?
            "#,
        )
        .bind(&session.participant.id)
        .bind(identity.creator_id.as_deref())
        .bind(&now)
        .bind(&now)
        .bind(&session.id)
        .bind(&identity.user_id)
        .bind(&token_hash)
        .execute(pool)
        .await?;
        if result.rows_affected() > 0 {
            return Ok((token.to_string(), true, now));
        }
    }

    let raw_token = format!(
        "wsc_{}_{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    sqlx::query(
        r#"
        INSERT INTO collaboration_socket_sessions (
            id, collaboration_session_id, user_id, creator_id, participant_id,
            session_token_hash, connected_at, last_seen_at, disconnected_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(format!("css-{}", Uuid::new_v4().simple()))
    .bind(&session.id)
    .bind(&identity.user_id)
    .bind(identity.creator_id.as_deref())
    .bind(&session.participant.id)
    .bind(hash_token(&raw_token))
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok((raw_token, false, now))
}

pub(crate) async fn touch_collaboration_socket_session(
    pool: &SqlitePool,
    session_id: &str,
    session_token: &str,
    lease_connected_at: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE collaboration_socket_sessions SET last_seen_at = ?, disconnected_at = NULL WHERE collaboration_session_id = ? AND session_token_hash = ? AND connected_at = ?",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(session_id)
    .bind(hash_token(session_token))
    .bind(lease_connected_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn disconnect_collaboration_socket_session(
    pool: &SqlitePool,
    session_id: &str,
    session_token: &str,
    lease_connected_at: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE collaboration_socket_sessions SET last_seen_at = ?, disconnected_at = ? WHERE collaboration_session_id = ? AND session_token_hash = ? AND connected_at = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(session_id)
    .bind(hash_token(session_token))
    .bind(lease_connected_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn count_active_collaboration_socket_sessions(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<i64> {
    let row = sqlx::query(
        "SELECT COUNT(DISTINCT participant_id) AS count FROM collaboration_socket_sessions WHERE collaboration_session_id = ? AND disconnected_at IS NULL AND last_seen_at >= ?",
    )
    .bind(session_id)
    .bind(active_presence_cutoff())
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}

pub(crate) async fn count_all_active_collaboration_socket_sessions(
    pool: &SqlitePool,
) -> AppResult<i64> {
    let row = sqlx::query(
        "SELECT COUNT(DISTINCT collaboration_session_id || ':' || participant_id) AS count FROM collaboration_socket_sessions WHERE disconnected_at IS NULL AND last_seen_at >= ?",
    )
    .bind(active_presence_cutoff())
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}

pub(crate) async fn count_all_active_creator_live_socket_sessions(
    pool: &SqlitePool,
) -> AppResult<i64> {
    let row = sqlx::query(
        "SELECT COUNT(DISTINCT creator_id || ':' || user_id || ':' || session_token_hash) AS count FROM creator_live_socket_sessions WHERE disconnected_at IS NULL AND last_seen_at >= ?",
    )
    .bind(active_presence_cutoff())
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}
