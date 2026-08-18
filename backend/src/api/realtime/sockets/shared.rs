use super::*;

pub(crate) async fn close_websocket(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
) {
    let _ = sender.send(Message::Close(None)).await;
}

pub(crate) async fn ensure_identity_session_active(
    pool: &SqlitePool,
    identity: &RequestIdentity,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS count
        FROM auth_sessions
        WHERE id = ?
          AND user_id = ?
          AND revoked_at IS NULL
          AND (expires_at IS NULL OR expires_at > ?)
        "#,
    )
    .bind(&identity.session_id)
    .bind(&identity.user_id)
    .bind(&now)
    .fetch_one(pool)
    .await?;
    let count: i64 = row.get("count");
    if count == 0 {
        return Err(AppError::Unauthorized);
    }
    Ok(())
}

pub(crate) fn auth_session_channel_id(session_id: &str) -> String {
    format!("auth-session:{session_id}")
}
