use super::*;

pub(crate) async fn fetch_broadcasts(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<Broadcast>> {
    let rows = sqlx::query(
        "SELECT id, title, category, tags_json, status, started_at, ended_at, duration_sec, peak_viewers, average_viewers, chat_messages, new_followers, new_subscribers, revenue, thumbnail, is_mature FROM broadcasts WHERE creator_id = ? ORDER BY started_at DESC",
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| Broadcast {
            id: row.get("id"),
            title: row.get("title"),
            category: row.get("category"),
            tags: from_json(row.get::<String, _>("tags_json")).unwrap_or_default(),
            status: row.get("status"),
            started_at: row.get("started_at"),
            ended_at: row.get("ended_at"),
            duration_sec: row.get("duration_sec"),
            peak_viewers: row.get("peak_viewers"),
            average_viewers: row.get("average_viewers"),
            chat_messages: row.get("chat_messages"),
            new_followers: row.get("new_followers"),
            new_subscribers: row.get("new_subscribers"),
            revenue: row.get("revenue"),
            thumbnail: row.get("thumbnail"),
            is_mature: row.get::<i64, _>("is_mature") == 1,
        })
        .collect())
}

pub(crate) async fn fetch_broadcast_by_id(
    pool: &SqlitePool,
    creator_id: &str,
    id: &str,
) -> AppResult<Broadcast> {
    let row = sqlx::query(
        "SELECT id, title, category, tags_json, status, started_at, ended_at, duration_sec, peak_viewers, average_viewers, chat_messages, new_followers, new_subscribers, revenue, thumbnail, is_mature FROM broadcasts WHERE creator_id = ? AND id = ?",
    )
    .bind(creator_id)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Broadcast {
        id: row.get("id"),
        title: row.get("title"),
        category: row.get("category"),
        tags: from_json(row.get::<String, _>("tags_json")).unwrap_or_default(),
        status: row.get("status"),
        started_at: row.get("started_at"),
        ended_at: row.get("ended_at"),
        duration_sec: row.get("duration_sec"),
        peak_viewers: row.get("peak_viewers"),
        average_viewers: row.get("average_viewers"),
        chat_messages: row.get("chat_messages"),
        new_followers: row.get("new_followers"),
        new_subscribers: row.get("new_subscribers"),
        revenue: row.get("revenue"),
        thumbnail: row.get("thumbnail"),
        is_mature: row.get::<i64, _>("is_mature") == 1,
    })
}
