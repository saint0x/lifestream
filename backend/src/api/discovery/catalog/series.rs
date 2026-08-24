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

pub(crate) async fn fetch_series_page(
    pool: &SqlitePool,
    genre: Option<&str>,
    originals_only: bool,
    sort: &str,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<Series>, i64)> {
    let mut filters = Vec::new();
    if genre.is_some() {
        filters.push(
            r#"EXISTS (
                SELECT 1
                FROM json_each(series.genres_json)
                WHERE json_each.value = ?
            )"#,
        );
    }
    if originals_only {
        filters.push("is_original = 1");
    }
    let where_clause = if filters.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", filters.join(" AND "))
    };
    let order_clause = match sort {
        "newest" => "year DESC, score DESC",
        "score" => "score DESC, year DESC",
        "title" => "title COLLATE NOCASE ASC",
        _ => "trending DESC, score DESC, year DESC",
    };
    let select_query = format!(
        r#"
        SELECT id, slug, title, tagline, synopsis, year, rating, genres_json,
               images_json, credits_json, score, is_original, trending, hero_color,
               status, total_episodes
        FROM series
        {where_clause}
        ORDER BY {order_clause}
        LIMIT ? OFFSET ?
        "#
    );
    let count_query = format!("SELECT COUNT(*) AS total FROM series {where_clause}");

    let mut count_statement = sqlx::query(&count_query);
    let mut select_statement = sqlx::query(&select_query);
    if let Some(genre) = genre {
        count_statement = count_statement.bind(genre);
        select_statement = select_statement.bind(genre);
    }
    let total: i64 = count_statement.fetch_one(pool).await?.get("total");
    let rows = select_statement
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(series_from_row(pool, row, None).await?);
    }
    Ok((items, total))
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

pub(crate) async fn fetch_series_by_episode_id(
    pool: &SqlitePool,
    episode_id: &str,
    progress: Option<&ContinueWatchingEntry>,
) -> AppResult<Series> {
    let row = sqlx::query(
        r#"
        SELECT s.id, s.slug, s.title, s.tagline, s.synopsis, s.year, s.rating,
               s.genres_json, s.images_json, s.credits_json, s.score, s.is_original,
               s.trending, s.hero_color, s.status, s.total_episodes
        FROM series s
        INNER JOIN episodes e ON e.series_id = s.id
        WHERE e.id = ?
        "#,
    )
    .bind(episode_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    series_from_row(pool, row, progress).await
}

pub(crate) async fn fetch_series_previews_by_ids(
    pool: &SqlitePool,
    ids: &[String],
) -> AppResult<Vec<Series>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = vec!["?"; ids.len()].join(", ");
    let query = format!(
        "SELECT id, slug, title, tagline, synopsis, year, rating, genres_json, images_json, credits_json, score, is_original, trending, hero_color, status, total_episodes FROM series WHERE id IN ({placeholders})"
    );
    let mut statement = sqlx::query(&query);
    for id in ids {
        statement = statement.bind(id);
    }
    let rows = statement.fetch_all(pool).await?;

    let mut by_id = std::collections::HashMap::with_capacity(rows.len());
    for row in rows {
        let series = series_preview_from_row(pool, row).await?;
        by_id.insert(series.id.clone(), series);
    }

    let mut ordered = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(series) = by_id.remove(id) {
            ordered.push(series);
        }
    }
    Ok(ordered)
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

async fn series_preview_from_row(
    pool: &SqlitePool,
    row: sqlx::sqlite::SqliteRow,
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
        seasons: fetch_season_previews(pool, &series_id).await?,
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

async fn fetch_season_previews(pool: &SqlitePool, series_id: &str) -> AppResult<Vec<Season>> {
    Ok(sqlx::query(
        "SELECT season_number, title FROM seasons WHERE series_id = ? ORDER BY season_number ASC",
    )
    .bind(series_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|season_row| Season {
        season_number: season_row.get("season_number"),
        title: season_row.get("title"),
        episodes: Vec::new(),
    })
    .collect())
}
