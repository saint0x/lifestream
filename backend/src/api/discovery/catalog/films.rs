use super::*;

pub(crate) async fn fetch_films(
    pool: &SqlitePool,
    extra_where: Option<&str>,
    limit: Option<i64>,
) -> AppResult<Vec<Film>> {
    let mut query = String::from(
        r#"
        SELECT id, slug, title, tagline, synopsis, year, rating, genres_json, images_json, credits_json, score,
               is_original, trending, hero_color, duration_sec,
               CASE WHEN EXISTS (
                    SELECT 1
                    FROM media_assets ma
                    WHERE ma.upload_id = films.id
                      AND ma.status IN ('ready', 'published')
               ) THEN 1 ELSE 0 END AS playback_ready
        FROM films
        "#,
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
        items.push(film_from_row(row, None)?);
    }
    Ok(items)
}

pub(crate) async fn fetch_films_by_genre(pool: &SqlitePool, genre: &str) -> AppResult<Vec<Film>> {
    let rows = sqlx::query(
        r#"
        SELECT id, slug, title, tagline, synopsis, year, rating, genres_json, images_json,
               credits_json, score, is_original, trending, hero_color, duration_sec,
               CASE WHEN EXISTS (
                    SELECT 1
                    FROM media_assets ma
                    WHERE ma.upload_id = films.id
                      AND ma.status IN ('ready', 'published')
               ) THEN 1 ELSE 0 END AS playback_ready
        FROM films
        WHERE EXISTS (
            SELECT 1
            FROM json_each(films.genres_json)
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
        items.push(film_from_row(row, None)?);
    }
    Ok(items)
}

pub(crate) async fn fetch_film_by_slug(
    pool: &SqlitePool,
    slug: &str,
    progress: Option<&ContinueWatchingEntry>,
) -> AppResult<Film> {
    let row = sqlx::query(
        r#"
        SELECT id, slug, title, tagline, synopsis, year, rating, genres_json, images_json, credits_json, score,
               is_original, trending, hero_color, duration_sec,
               CASE WHEN EXISTS (
                    SELECT 1
                    FROM media_assets ma
                    WHERE ma.upload_id = films.id
                      AND ma.status IN ('ready', 'published')
               ) THEN 1 ELSE 0 END AS playback_ready
        FROM films
        WHERE slug = ?
        "#,
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    film_from_row(row, progress)
}

pub(crate) async fn fetch_film_by_id(
    pool: &SqlitePool,
    id: &str,
    progress: Option<&ContinueWatchingEntry>,
) -> AppResult<Film> {
    let row = sqlx::query(
        r#"
        SELECT id, slug, title, tagline, synopsis, year, rating, genres_json, images_json, credits_json, score,
               is_original, trending, hero_color, duration_sec,
               CASE WHEN EXISTS (
                    SELECT 1
                    FROM media_assets ma
                    WHERE ma.upload_id = films.id
                      AND ma.status IN ('ready', 'published')
               ) THEN 1 ELSE 0 END AS playback_ready
        FROM films
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    film_from_row(row, progress)
}

fn film_from_row(
    row: sqlx::sqlite::SqliteRow,
    progress: Option<&ContinueWatchingEntry>,
) -> AppResult<Film> {
    Ok(Film {
        id: row.get("id"),
        slug: row.get("slug"),
        kind: "film".to_string(),
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
        duration_sec: row.get("duration_sec"),
        progress_sec: progress.map(|entry| entry.progress_sec),
        playback_session_url: (row.get::<i64, _>("playback_ready") == 1)
            .then(|| playback_content_session_api_url(&row.get::<String, _>("id"))),
        playback_ready: row.get::<i64, _>("playback_ready") == 1,
    })
}
