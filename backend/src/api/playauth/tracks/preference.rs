use super::*;

pub(crate) async fn fetch_user_subtitle_preference(
    pool: &SqlitePool,
    user_id: Option<&str>,
) -> AppResult<Option<String>> {
    let Some(user_id) = user_id else {
        return Ok(None);
    };
    let row = sqlx::query("SELECT subtitle_language FROM user_playback_settings WHERE user_id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| row.get("subtitle_language")))
}

pub(crate) async fn fetch_user_audio_preferences(
    pool: &SqlitePool,
    user_id: Option<&str>,
) -> AppResult<(Option<String>, bool)> {
    let Some(user_id) = user_id else {
        return Ok((None, false));
    };
    let row = sqlx::query(
        "SELECT audio_language, prefer_dubbed FROM user_playback_settings WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row
        .map(|row| {
            (
                Some(row.get::<String, _>("audio_language")),
                row.get::<i64, _>("prefer_dubbed") == 1,
            )
        })
        .unwrap_or((None, false)))
}
