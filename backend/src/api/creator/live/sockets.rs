use super::*;

pub(crate) async fn fetch_creator_live_socket_presence_by_id_raw(
    pool: &SqlitePool,
    creator_id: &str,
    socket_id: &str,
) -> AppResult<CreatorLiveSocketPresence> {
    let cutoff = active_presence_cutoff();
    let row = sqlx::query(
        r#"
        SELECT id, creator_id, user_id, connected_at, last_seen_at, disconnected_at
        FROM creator_live_socket_sessions
        WHERE creator_id = ? AND id = ?
        "#,
    )
    .bind(creator_id)
    .bind(socket_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let last_seen_at: String = row.get("last_seen_at");
    let disconnected_at: Option<String> = row.get("disconnected_at");
    Ok(CreatorLiveSocketPresence {
        id: row.get("id"),
        creator_id: row.get("creator_id"),
        user_id: row.get("user_id"),
        connected_at: row.get("connected_at"),
        last_seen_at: last_seen_at.clone(),
        disconnected_at,
        is_stale: last_seen_at < cutoff,
    })
}
