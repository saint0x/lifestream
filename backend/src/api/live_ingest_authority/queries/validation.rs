use super::*;

pub(crate) async fn validate_live_ingest_session(
    pool: &SqlitePool,
    session_id: &str,
    ingest_token: &str,
) -> AppResult<LiveIngestSession> {
    let token_hash = crate::auth::hash_token(ingest_token);
    let row = sqlx::query(
        r#"
        SELECT id, creator_id, broadcast_id, protocol, ingest_server, status, bitrate_kbps, viewers,
               dropped_frames, connected_at, last_heartbeat_at, disconnected_at
        FROM live_ingest_sessions
        WHERE id = ? AND ingest_token_hash = ? AND status = 'connected'
        "#,
    )
    .bind(session_id)
    .bind(token_hash)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    live_ingest_session_from_row(row)
}
