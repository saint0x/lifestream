use super::*;

pub(crate) async fn fetch_creator_series_title(
    pool: &SqlitePool,
    creator_id: &str,
    series_id: &str,
) -> AppResult<Option<String>> {
    Ok(
        sqlx::query("SELECT title FROM creator_series_projects WHERE creator_id = ? AND id = ?")
            .bind(creator_id)
            .bind(series_id)
            .fetch_optional(pool)
            .await?
            .map(|row| row.get("title")),
    )
}

pub(crate) async fn ensure_creator_series_season(
    pool: &SqlitePool,
    creator_id: &str,
    series_id: &str,
    season_number: i64,
    title: String,
    synopsis: String,
) -> AppResult<()> {
    let exists =
        sqlx::query("SELECT 1 FROM creator_series_projects WHERE creator_id = ? AND id = ?")
            .bind(creator_id)
            .bind(series_id)
            .fetch_optional(pool)
            .await?
            .is_some();
    if !exists {
        return Err(AppError::BadRequest(
            "seriesId does not belong to creator".to_string(),
        ));
    }

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO creator_series_seasons (
            id, series_id, season_number, title, synopsis, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(series_id, season_number) DO UPDATE SET
            title = excluded.title,
            synopsis = excluded.synopsis,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(format!("season-{series_id}-{season_number}"))
    .bind(series_id)
    .bind(season_number)
    .bind(title)
    .bind(synopsis)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(())
}
