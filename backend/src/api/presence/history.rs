use super::*;

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
