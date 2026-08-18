use super::*;

pub(crate) async fn fetch_creator_catalog_films(
    pool: &SqlitePool,
    public_only: bool,
) -> AppResult<Vec<CreatorCatalogFilm>> {
    publish_due_scheduled_upload_releases(pool, None, None).await?;
    let now = Utc::now().to_rfc3339();
    let query =
        creator_catalog_film_query(film_visibility_filter(public_only), "u.slug IS NOT NULL");
    let rows = sqlx::query(&query).bind(&now).fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(creator_catalog_film_from_row)
        .collect())
}

pub(crate) async fn fetch_creator_catalog_film_by_slug(
    pool: &SqlitePool,
    slug: &str,
    public_only: bool,
) -> AppResult<CreatorCatalogFilm> {
    publish_due_scheduled_upload_releases(pool, None, None).await?;
    let now = Utc::now().to_rfc3339();
    let query = creator_catalog_film_query(film_visibility_filter(public_only), "u.slug = ?");
    let row = sqlx::query(&query)
        .bind(slug)
        .bind(&now)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(creator_catalog_film_from_row(row))
}

pub(crate) async fn fetch_creator_catalog_film_by_id(
    pool: &SqlitePool,
    id: &str,
    public_only: bool,
) -> AppResult<CreatorCatalogFilm> {
    publish_due_scheduled_upload_releases(pool, None, None).await?;
    let now = Utc::now().to_rfc3339();
    let query = creator_catalog_film_query(film_visibility_filter(public_only), "u.id = ?");
    let row = sqlx::query(&query)
        .bind(id)
        .bind(&now)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(creator_catalog_film_from_row(row))
}

fn creator_catalog_film_query(visibility_filter: &str, selector: &str) -> String {
    format!(
        r#"
        SELECT u.id, u.slug, u.title, u.description, u.duration_sec, COALESCE(u.release_at, u.published_at) AS release_at,
               u.thumbnail, u.resolution, cp.handle, cp.display_name,
               u.access_policy, u.access_tier_id, u.price_cents, u.currency, u.rental_window_hours,
               CASE WHEN ma.status IN ('ready', 'published') THEN 1 ELSE 0 END AS playback_ready
        FROM uploads u
        JOIN creator_profiles cp ON cp.id = u.creator_id
        LEFT JOIN media_assets ma ON ma.upload_id = u.id
        WHERE u.kind = 'film'
          AND {selector}
          AND u.status = 'published'
          AND {visibility_filter}
          AND COALESCE(u.release_at, u.published_at) <= ?
        ORDER BY COALESCE(u.release_at, u.published_at) DESC
        "#
    )
}

fn film_visibility_filter(public_only: bool) -> &'static str {
    if public_only {
        "u.visibility = 'public'"
    } else {
        "u.visibility IN ('public', 'unlisted')"
    }
}

fn creator_catalog_film_from_row(row: sqlx::sqlite::SqliteRow) -> CreatorCatalogFilm {
    let id: String = row.get("id");
    let playback_ready = row.get::<i64, _>("playback_ready") == 1;
    CreatorCatalogFilm {
        id: id.clone(),
        upload_id: id.clone(),
        playback_session_url: playback_ready.then(|| playback_content_session_api_url(&id)),
        slug: row.get("slug"),
        title: row.get("title"),
        synopsis: row.get("description"),
        duration_sec: row.get("duration_sec"),
        release_at: row.get("release_at"),
        thumbnail: row.get("thumbnail"),
        resolution: row.get("resolution"),
        creator_handle: row.get("handle"),
        creator_display_name: row.get("display_name"),
        access_policy: row.get("access_policy"),
        access_tier_id: row.get("access_tier_id"),
        price_cents: row.get("price_cents"),
        currency: row.get("currency"),
        rental_window_hours: row.get("rental_window_hours"),
        playback_ready,
    }
}
