use super::*;

pub(super) async fn fetch_creator_series(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<CreatorSeriesProject>> {
    let rows = sqlx::query(
        r#"
        SELECT id, slug, title, synopsis, rating, genres_json, hero_color,
               poster_url, backdrop_url, status, created_at, updated_at
        FROM creator_series_projects
        WHERE creator_id = ?
        ORDER BY updated_at DESC
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| CreatorSeriesProject {
            id: row.get("id"),
            slug: row.get("slug"),
            title: row.get("title"),
            synopsis: row.get("synopsis"),
            rating: row.get("rating"),
            genres: from_json(row.get::<String, _>("genres_json")).unwrap_or_default(),
            hero_color: row.get("hero_color"),
            poster_url: row.get("poster_url"),
            backdrop_url: row.get("backdrop_url"),
            status: row.get("status"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .collect())
}

pub(super) async fn fetch_creator_catalog_series(
    pool: &SqlitePool,
    public_only: bool,
) -> AppResult<Vec<CreatorCatalogSeries>> {
    publish_due_scheduled_upload_releases(pool, None, None).await?;
    let now = Utc::now().to_rfc3339();
    let visibility_filter = if public_only {
        "u.visibility = 'public'"
    } else {
        "u.visibility IN ('public', 'unlisted')"
    };
    let query = format!(
        r#"
        SELECT csp.id, csp.slug, csp.title, csp.synopsis, csp.rating, csp.genres_json,
               csp.hero_color, csp.poster_url, csp.backdrop_url, csp.status,
               cp.handle, cp.display_name, COUNT(u.id) AS published_episode_count
        FROM creator_series_projects csp
        JOIN creator_profiles cp ON cp.id = csp.creator_id
        JOIN uploads u ON u.series_id = csp.id
        WHERE u.kind = 'episode'
          AND u.status = 'published'
          AND {visibility_filter}
          AND COALESCE(u.release_at, u.published_at) <= ?
        GROUP BY csp.id
        ORDER BY MAX(COALESCE(u.release_at, u.published_at)) DESC
        "#
    );
    let rows = sqlx::query(&query).bind(&now).fetch_all(pool).await?;
    let mut series = Vec::with_capacity(rows.len());
    for row in rows {
        let id: String = row.get("id");
        series.push(CreatorCatalogSeries {
            id: id.clone(),
            slug: row.get("slug"),
            title: row.get("title"),
            synopsis: row.get("synopsis"),
            rating: row.get("rating"),
            genres: from_json(row.get::<String, _>("genres_json")).unwrap_or_default(),
            hero_color: row.get("hero_color"),
            poster_url: row.get("poster_url"),
            backdrop_url: row.get("backdrop_url"),
            status: row.get("status"),
            creator_handle: row.get("handle"),
            creator_display_name: row.get("display_name"),
            published_episode_count: row.get("published_episode_count"),
            seasons: fetch_creator_catalog_seasons(pool, &id, public_only).await?,
        });
    }
    Ok(series)
}

pub(super) async fn fetch_creator_catalog_series_by_slug(
    pool: &SqlitePool,
    slug: &str,
    public_only: bool,
) -> AppResult<CreatorCatalogSeries> {
    publish_due_scheduled_upload_releases(pool, None, None).await?;
    let now = Utc::now().to_rfc3339();
    let visibility_filter = if public_only {
        "u.visibility = 'public'"
    } else {
        "u.visibility IN ('public', 'unlisted')"
    };
    let query = format!(
        r#"
        SELECT csp.id, csp.slug, csp.title, csp.synopsis, csp.rating, csp.genres_json,
               csp.hero_color, csp.poster_url, csp.backdrop_url, csp.status,
               cp.handle, cp.display_name, COUNT(u.id) AS published_episode_count
        FROM creator_series_projects csp
        JOIN creator_profiles cp ON cp.id = csp.creator_id
        JOIN uploads u ON u.series_id = csp.id
        WHERE csp.slug = ?
          AND u.kind = 'episode'
          AND u.status = 'published'
          AND {visibility_filter}
          AND COALESCE(u.release_at, u.published_at) <= ?
        GROUP BY csp.id
        "#
    );
    let row = sqlx::query(&query)
        .bind(slug)
        .bind(&now)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    let id: String = row.get("id");
    Ok(CreatorCatalogSeries {
        id: id.clone(),
        slug: row.get("slug"),
        title: row.get("title"),
        synopsis: row.get("synopsis"),
        rating: row.get("rating"),
        genres: from_json(row.get::<String, _>("genres_json")).unwrap_or_default(),
        hero_color: row.get("hero_color"),
        poster_url: row.get("poster_url"),
        backdrop_url: row.get("backdrop_url"),
        status: row.get("status"),
        creator_handle: row.get("handle"),
        creator_display_name: row.get("display_name"),
        published_episode_count: row.get("published_episode_count"),
        seasons: fetch_creator_catalog_seasons(pool, &id, public_only).await?,
    })
}

pub(super) async fn fetch_creator_catalog_series_by_id(
    pool: &SqlitePool,
    id: &str,
    public_only: bool,
) -> AppResult<CreatorCatalogSeries> {
    publish_due_scheduled_upload_releases(pool, None, None).await?;
    let now = Utc::now().to_rfc3339();
    let visibility_filter = if public_only {
        "u.visibility = 'public'"
    } else {
        "u.visibility IN ('public', 'unlisted')"
    };
    let query = format!(
        r#"
        SELECT csp.id, csp.slug, csp.title, csp.synopsis, csp.rating, csp.genres_json,
               csp.hero_color, csp.poster_url, csp.backdrop_url, csp.status,
               cp.handle, cp.display_name, COUNT(u.id) AS published_episode_count
        FROM creator_series_projects csp
        JOIN creator_profiles cp ON cp.id = csp.creator_id
        JOIN uploads u ON u.series_id = csp.id
        WHERE csp.id = ?
          AND u.kind = 'episode'
          AND u.status = 'published'
          AND {visibility_filter}
          AND COALESCE(u.release_at, u.published_at) <= ?
        GROUP BY csp.id
        "#
    );
    let row = sqlx::query(&query)
        .bind(id)
        .bind(&now)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    let id: String = row.get("id");
    Ok(CreatorCatalogSeries {
        id: id.clone(),
        slug: row.get("slug"),
        title: row.get("title"),
        synopsis: row.get("synopsis"),
        rating: row.get("rating"),
        genres: from_json(row.get::<String, _>("genres_json")).unwrap_or_default(),
        hero_color: row.get("hero_color"),
        poster_url: row.get("poster_url"),
        backdrop_url: row.get("backdrop_url"),
        status: row.get("status"),
        creator_handle: row.get("handle"),
        creator_display_name: row.get("display_name"),
        published_episode_count: row.get("published_episode_count"),
        seasons: fetch_creator_catalog_seasons(pool, &id, public_only).await?,
    })
}

async fn fetch_creator_catalog_seasons(
    pool: &SqlitePool,
    series_id: &str,
    public_only: bool,
) -> AppResult<Vec<CreatorCatalogSeason>> {
    let now = Utc::now().to_rfc3339();
    let visibility_filter = if public_only {
        "u.visibility = 'public'"
    } else {
        "u.visibility IN ('public', 'unlisted')"
    };
    let query = format!(
        r#"
        SELECT
            COALESCE(css.id, printf('%s-season-%d', u.series_id, u.season_number)) AS id,
            u.season_number,
            COALESCE(css.title, printf('Season %d', u.season_number)) AS title,
            COALESCE(css.synopsis, '') AS synopsis
        FROM uploads u
        LEFT JOIN creator_series_seasons css
          ON css.series_id = u.series_id AND css.season_number = u.season_number
        WHERE u.series_id = ?
          AND u.kind = 'episode'
          AND u.status = 'published'
          AND {visibility_filter}
          AND COALESCE(u.release_at, u.published_at) <= ?
        GROUP BY u.series_id, u.season_number
        ORDER BY u.season_number ASC
        "#
    );
    let rows = sqlx::query(&query)
        .bind(series_id)
        .bind(&now)
        .fetch_all(pool)
        .await?;
    let mut seasons = Vec::with_capacity(rows.len());
    for row in rows {
        let season_number: i64 = row.get("season_number");
        seasons.push(CreatorCatalogSeason {
            id: row.get("id"),
            season_number,
            title: row.get("title"),
            synopsis: row.get("synopsis"),
            episodes: fetch_creator_catalog_episodes(pool, series_id, season_number, public_only)
                .await?,
        });
    }
    Ok(seasons)
}

async fn fetch_creator_catalog_episodes(
    pool: &SqlitePool,
    series_id: &str,
    season_number: i64,
    public_only: bool,
) -> AppResult<Vec<CreatorCatalogEpisode>> {
    let now = Utc::now().to_rfc3339();
    let visibility_filter = if public_only {
        "u.visibility = 'public'"
    } else {
        "u.visibility IN ('public', 'unlisted')"
    };
    let query = format!(
        r#"
        SELECT u.id, u.slug, u.series_id, csp.slug AS series_slug, u.season_number, u.episode_number,
               u.title, u.description, u.duration_sec, COALESCE(u.release_at, u.published_at) AS release_at,
               u.thumbnail, u.access_policy, u.access_tier_id, u.price_cents, u.currency, u.rental_window_hours,
               CASE WHEN ma.status IN ('ready', 'published') THEN 1 ELSE 0 END AS playback_ready
        FROM uploads u
        JOIN creator_series_projects csp ON csp.id = u.series_id
        LEFT JOIN media_assets ma ON ma.upload_id = u.id
        WHERE u.series_id = ?
          AND u.season_number = ?
          AND u.kind = 'episode'
          AND u.status = 'published'
          AND {visibility_filter}
          AND COALESCE(u.release_at, u.published_at) <= ?
        ORDER BY u.episode_number ASC, COALESCE(u.release_at, u.published_at) ASC
        "#
    );
    let rows = sqlx::query(&query)
        .bind(series_id)
        .bind(season_number)
        .bind(&now)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let id: String = row.get("id");
            let playback_ready = row.get::<i64, _>("playback_ready") == 1;
            CreatorCatalogEpisode {
                id: id.clone(),
                upload_id: id.clone(),
                playback_session_url: playback_ready.then(|| playback_content_session_api_url(&id)),
                slug: row.get("slug"),
                series_id: row.get("series_id"),
                series_slug: row.get("series_slug"),
                season_number: row.get("season_number"),
                episode_number: row.get("episode_number"),
                title: row.get("title"),
                synopsis: row.get("description"),
                duration_sec: row.get("duration_sec"),
                release_at: row.get("release_at"),
                thumbnail: row.get("thumbnail"),
                access_policy: row.get("access_policy"),
                access_tier_id: row.get("access_tier_id"),
                price_cents: row.get("price_cents"),
                currency: row.get("currency"),
                rental_window_hours: row.get("rental_window_hours"),
                playback_ready,
            }
        })
        .collect())
}

pub(super) async fn fetch_creator_catalog_films(
    pool: &SqlitePool,
    public_only: bool,
) -> AppResult<Vec<CreatorCatalogFilm>> {
    publish_due_scheduled_upload_releases(pool, None, None).await?;
    let now = Utc::now().to_rfc3339();
    let visibility_filter = if public_only {
        "u.visibility = 'public'"
    } else {
        "u.visibility IN ('public', 'unlisted')"
    };
    let query = format!(
        r#"
        SELECT u.id, u.slug, u.title, u.description, u.duration_sec, COALESCE(u.release_at, u.published_at) AS release_at,
               u.thumbnail, u.resolution, cp.handle, cp.display_name,
               u.access_policy, u.access_tier_id, u.price_cents, u.currency, u.rental_window_hours,
               CASE WHEN ma.status IN ('ready', 'published') THEN 1 ELSE 0 END AS playback_ready
        FROM uploads u
        JOIN creator_profiles cp ON cp.id = u.creator_id
        LEFT JOIN media_assets ma ON ma.upload_id = u.id
        WHERE u.kind = 'film'
          AND u.status = 'published'
          AND u.slug IS NOT NULL
          AND {visibility_filter}
          AND COALESCE(u.release_at, u.published_at) <= ?
        ORDER BY COALESCE(u.release_at, u.published_at) DESC
        "#
    );
    let rows = sqlx::query(&query).bind(&now).fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(|row| {
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
        })
        .collect())
}

pub(super) async fn fetch_creator_catalog_film_by_slug(
    pool: &SqlitePool,
    slug: &str,
    public_only: bool,
) -> AppResult<CreatorCatalogFilm> {
    publish_due_scheduled_upload_releases(pool, None, None).await?;
    let now = Utc::now().to_rfc3339();
    let visibility_filter = if public_only {
        "u.visibility = 'public'"
    } else {
        "u.visibility IN ('public', 'unlisted')"
    };
    let query = format!(
        r#"
        SELECT u.id, u.slug, u.title, u.description, u.duration_sec, COALESCE(u.release_at, u.published_at) AS release_at,
               u.thumbnail, u.resolution, cp.handle, cp.display_name,
               u.access_policy, u.access_tier_id, u.price_cents, u.currency, u.rental_window_hours,
               CASE WHEN ma.status IN ('ready', 'published') THEN 1 ELSE 0 END AS playback_ready
        FROM uploads u
        JOIN creator_profiles cp ON cp.id = u.creator_id
        LEFT JOIN media_assets ma ON ma.upload_id = u.id
        WHERE u.kind = 'film'
          AND u.slug = ?
          AND u.status = 'published'
          AND {visibility_filter}
          AND COALESCE(u.release_at, u.published_at) <= ?
        "#,
    );
    let row = sqlx::query(&query)
        .bind(slug)
        .bind(&now)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(CreatorCatalogFilm {
        id: row.get("id"),
        upload_id: row.get("id"),
        playback_session_url: (row.get::<i64, _>("playback_ready") == 1)
            .then(|| playback_content_session_api_url(&row.get::<String, _>("id"))),
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
        playback_ready: row.get::<i64, _>("playback_ready") == 1,
    })
}

pub(super) async fn fetch_creator_catalog_film_by_id(
    pool: &SqlitePool,
    id: &str,
    public_only: bool,
) -> AppResult<CreatorCatalogFilm> {
    publish_due_scheduled_upload_releases(pool, None, None).await?;
    let now = Utc::now().to_rfc3339();
    let visibility_filter = if public_only {
        "u.visibility = 'public'"
    } else {
        "u.visibility IN ('public', 'unlisted')"
    };
    let query = format!(
        r#"
        SELECT u.id, u.slug, u.title, u.description, u.duration_sec, COALESCE(u.release_at, u.published_at) AS release_at,
               u.thumbnail, u.resolution, cp.handle, cp.display_name,
               u.access_policy, u.access_tier_id, u.price_cents, u.currency, u.rental_window_hours,
               CASE WHEN ma.status IN ('ready', 'published') THEN 1 ELSE 0 END AS playback_ready
        FROM uploads u
        JOIN creator_profiles cp ON cp.id = u.creator_id
        LEFT JOIN media_assets ma ON ma.upload_id = u.id
        WHERE u.kind = 'film'
          AND u.id = ?
          AND u.status = 'published'
          AND {visibility_filter}
          AND COALESCE(u.release_at, u.published_at) <= ?
        "#,
    );
    let row = sqlx::query(&query)
        .bind(id)
        .bind(&now)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(CreatorCatalogFilm {
        id: row.get("id"),
        upload_id: row.get("id"),
        playback_session_url: (row.get::<i64, _>("playback_ready") == 1)
            .then(|| playback_content_session_api_url(&row.get::<String, _>("id"))),
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
        playback_ready: row.get::<i64, _>("playback_ready") == 1,
    })
}

pub(super) async fn fetch_creator_series_by_id(
    pool: &SqlitePool,
    creator_id: &str,
    id: &str,
) -> AppResult<CreatorSeriesProject> {
    let row = sqlx::query(
        r#"
        SELECT id, slug, title, synopsis, rating, genres_json, hero_color,
               poster_url, backdrop_url, status, created_at, updated_at
        FROM creator_series_projects
        WHERE creator_id = ? AND id = ?
        "#,
    )
    .bind(creator_id)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(CreatorSeriesProject {
        id: row.get("id"),
        slug: row.get("slug"),
        title: row.get("title"),
        synopsis: row.get("synopsis"),
        rating: row.get("rating"),
        genres: from_json(row.get::<String, _>("genres_json")).unwrap_or_default(),
        hero_color: row.get("hero_color"),
        poster_url: row.get("poster_url"),
        backdrop_url: row.get("backdrop_url"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}
