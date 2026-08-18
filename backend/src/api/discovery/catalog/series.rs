use super::*;

pub(crate) async fn fetch_series(
    pool: &SqlitePool,
    extra_where: Option<&str>,
    limit: Option<i64>,
) -> AppResult<Vec<Series>> {
    let mut query = String::from(
        "SELECT id, slug, title, tagline, synopsis, year, rating, genres_json, images_json, credits_json, score, is_original, trending, hero_color, status, total_episodes FROM series",
    );
    if let Some(extra) = extra_where {
        query.push(' ');
        query.push_str(extra);
    }
    query.push_str(" ORDER BY score DESC, year DESC");
    if let Some(limit) = limit {
        query.push_str(&format!(" LIMIT {limit}"));
    }

    let rows = sqlx::query(&query).fetch_all(pool).await?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(series_from_row(pool, row, None).await?);
    }
    Ok(items)
}

pub(crate) async fn fetch_series_by_genre(
    pool: &SqlitePool,
    genre: &str,
) -> AppResult<Vec<Series>> {
    let rows = sqlx::query(
        r#"
        SELECT id, slug, title, tagline, synopsis, year, rating, genres_json, images_json,
               credits_json, score, is_original, trending, hero_color, status, total_episodes
        FROM series
        WHERE EXISTS (
            SELECT 1
            FROM json_each(series.genres_json)
            WHERE json_each.value = ?
        )
        ORDER BY score DESC, year DESC
        "#,
    )
    .bind(genre)
    .fetch_all(pool)
    .await?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(series_from_row(pool, row, None).await?);
    }
    Ok(items)
}

pub(crate) async fn fetch_series_by_slug(
    pool: &SqlitePool,
    slug: &str,
    progress: Option<&ContinueWatchingEntry>,
) -> AppResult<Series> {
    let row = sqlx::query(
        "SELECT id, slug, title, tagline, synopsis, year, rating, genres_json, images_json, credits_json, score, is_original, trending, hero_color, status, total_episodes FROM series WHERE slug = ?",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    series_from_row(pool, row, progress).await
}

pub(crate) async fn fetch_series_by_id(
    pool: &SqlitePool,
    id: &str,
    progress: Option<&ContinueWatchingEntry>,
) -> AppResult<Series> {
    let row = sqlx::query(
        "SELECT id, slug, title, tagline, synopsis, year, rating, genres_json, images_json, credits_json, score, is_original, trending, hero_color, status, total_episodes FROM series WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    series_from_row(pool, row, progress).await
}

pub(crate) async fn fetch_episode_by_id(pool: &SqlitePool, episode_id: &str) -> AppResult<Episode> {
    let row = sqlx::query(
        r#"
        SELECT id, series_id, season_number, episode_number, title, synopsis, duration_sec, aired_at, thumbnail,
               CASE WHEN EXISTS (
                    SELECT 1
                    FROM media_assets ma
                    WHERE ma.upload_id = episodes.id
                      AND ma.status IN ('ready', 'published')
               ) THEN 1 ELSE 0 END AS playback_ready
        FROM episodes
        WHERE id = ?
        "#,
    )
    .bind(episode_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Episode {
        id: row.get("id"),
        series_id: row.get("series_id"),
        season_number: row.get("season_number"),
        episode_number: row.get("episode_number"),
        title: row.get("title"),
        synopsis: row.get("synopsis"),
        duration_sec: row.get("duration_sec"),
        aired_at: row.get("aired_at"),
        thumbnail: row.get("thumbnail"),
        progress_sec: None,
        playback_session_url: (row.get::<i64, _>("playback_ready") == 1)
            .then(|| playback_content_session_api_url(&row.get::<String, _>("id"))),
        playback_ready: row.get::<i64, _>("playback_ready") == 1,
    })
}

async fn series_from_row(
    pool: &SqlitePool,
    row: sqlx::sqlite::SqliteRow,
    progress: Option<&ContinueWatchingEntry>,
) -> AppResult<Series> {
    let series_id: String = row.get("id");
    Ok(Series {
        id: series_id.clone(),
        slug: row.get("slug"),
        kind: "series".to_string(),
        title: row.get("title"),
        tagline: row.get("tagline"),
        synopsis: row.get("synopsis"),
        year: row.get("year"),
        rating: row.get("rating"),
        genres: from_json(row.get::<String, _>("genres_json"))?,
        images: from_json(row.get::<String, _>("images_json"))?,
        credits: from_json(row.get::<String, _>("credits_json"))?,
        score: row.get("score"),
        is_original: row.get::<i64, _>("is_original") == 1,
        trending: row.get::<i64, _>("trending") == 1,
        hero_color: row.get("hero_color"),
        seasons: fetch_seasons(pool, &series_id, progress).await?,
        total_episodes: row.get("total_episodes"),
        status: row.get("status"),
    })
}

async fn fetch_seasons(
    pool: &SqlitePool,
    series_id: &str,
    progress: Option<&ContinueWatchingEntry>,
) -> AppResult<Vec<Season>> {
    let season_rows = sqlx::query(
        "SELECT season_number, title FROM seasons WHERE series_id = ? ORDER BY season_number ASC",
    )
    .bind(series_id)
    .fetch_all(pool)
    .await?;

    let mut seasons = Vec::with_capacity(season_rows.len());
    for season_row in season_rows {
        let season_number: i64 = season_row.get("season_number");
        let episode_rows = sqlx::query(
            r#"
            SELECT id, series_id, season_number, episode_number, title, synopsis, duration_sec, aired_at, thumbnail,
                   CASE WHEN EXISTS (
                        SELECT 1
                        FROM media_assets ma
                        WHERE ma.upload_id = episodes.id
                          AND ma.status IN ('ready', 'published')
                   ) THEN 1 ELSE 0 END AS playback_ready
            FROM episodes
            WHERE series_id = ? AND season_number = ?
            ORDER BY episode_number ASC
            "#,
        )
        .bind(series_id)
        .bind(season_number)
        .fetch_all(pool)
        .await?;

        let episodes = episode_rows
            .into_iter()
            .map(|episode_row| Episode {
                id: episode_row.get("id"),
                series_id: episode_row.get("series_id"),
                season_number: episode_row.get("season_number"),
                episode_number: episode_row.get("episode_number"),
                title: episode_row.get("title"),
                synopsis: episode_row.get("synopsis"),
                duration_sec: episode_row.get("duration_sec"),
                aired_at: episode_row.get("aired_at"),
                thumbnail: episode_row.get("thumbnail"),
                progress_sec: progress.and_then(|entry| {
                    if entry.episode_id.as_deref()
                        == Some(episode_row.get::<String, _>("id").as_str())
                    {
                        Some(entry.progress_sec)
                    } else {
                        None
                    }
                }),
                playback_session_url: (episode_row.get::<i64, _>("playback_ready") == 1)
                    .then(|| playback_content_session_api_url(&episode_row.get::<String, _>("id"))),
                playback_ready: episode_row.get::<i64, _>("playback_ready") == 1,
            })
            .collect();

        seasons.push(Season {
            season_number,
            title: season_row.get("title"),
            episodes,
        });
    }
    Ok(seasons)
}
