use super::*;

pub(crate) async fn expire_playback_session_by_id(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<()> {
    expire_playback_sessions_where(pool, "id = ? AND expires_at > ?", &[session_id.to_string()])
        .await
}

pub(crate) async fn expire_playback_sessions_for_upload(
    pool: &SqlitePool,
    upload_id: &str,
) -> AppResult<()> {
    expire_playback_sessions_where(
        pool,
        "content_id = ? AND expires_at > ?",
        &[upload_id.to_string()],
    )
    .await
}

async fn expire_playback_sessions_where(
    pool: &SqlitePool,
    predicate: &str,
    values: &[String],
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let query =
        format!("UPDATE playback_sessions SET expires_at = ?, last_used_at = ? WHERE {predicate}");
    let mut sql = sqlx::query(&query).bind(&now).bind(&now);
    for value in values {
        sql = sql.bind(value);
    }
    sql.bind(&now).execute(pool).await?;
    Ok(())
}
