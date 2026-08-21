use super::*;

pub(crate) struct UserPlaybackPreferences {
    pub(crate) subtitle_language: Option<String>,
    pub(crate) audio_language: Option<String>,
    pub(crate) prefer_dubbed: bool,
}

pub(crate) async fn fetch_user_playback_preferences(
    pool: &SqlitePool,
    user_id: Option<&str>,
) -> AppResult<UserPlaybackPreferences> {
    let Some(user_id) = user_id else {
        return Ok(UserPlaybackPreferences {
            subtitle_language: None,
            audio_language: None,
            prefer_dubbed: false,
        });
    };
    let row = sqlx::query(
        r#"
        SELECT subtitle_language, audio_language, prefer_dubbed
        FROM user_playback_settings
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row
        .map(|row| UserPlaybackPreferences {
            subtitle_language: row.get("subtitle_language"),
            audio_language: row.get("audio_language"),
            prefer_dubbed: row.get::<i64, _>("prefer_dubbed") == 1,
        })
        .unwrap_or(UserPlaybackPreferences {
            subtitle_language: None,
            audio_language: None,
            prefer_dubbed: false,
        }))
}
