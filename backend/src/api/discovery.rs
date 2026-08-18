use super::*;

pub(super) async fn fetch_series(
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

pub(super) async fn fetch_series_by_genre(
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

pub(super) async fn fetch_series_by_slug(
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

pub(super) async fn fetch_series_by_id(
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

pub(super) async fn fetch_episode_by_id(pool: &SqlitePool, episode_id: &str) -> AppResult<Episode> {
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

pub(super) async fn fetch_films(
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

pub(super) async fn fetch_films_by_genre(pool: &SqlitePool, genre: &str) -> AppResult<Vec<Film>> {
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

pub(super) async fn fetch_film_by_slug(
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

pub(super) async fn fetch_film_by_id(
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

pub(super) async fn fetch_streamers(pool: &SqlitePool) -> AppResult<Vec<Streamer>> {
    let rows = sqlx::query(
        "SELECT id, handle, display_name, avatar, bio, followers, is_partner, is_live FROM streamers ORDER BY followers DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| Streamer {
            id: row.get("id"),
            handle: row.get("handle"),
            display_name: row.get("display_name"),
            avatar: row.get("avatar"),
            bio: row.get("bio"),
            followers: row.get("followers"),
            is_partner: row.get::<i64, _>("is_partner") == 1,
            is_live: row.get::<i64, _>("is_live") == 1,
        })
        .collect())
}

pub(super) async fn fetch_streamer_by_id(pool: &SqlitePool, id: &str) -> AppResult<Streamer> {
    let row = sqlx::query(
        "SELECT id, handle, display_name, avatar, bio, followers, is_partner, is_live FROM streamers WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(streamer_from_row(row))
}

pub(super) async fn validate_watchlist_content(
    pool: &SqlitePool,
    content_id: &str,
) -> AppResult<()> {
    if fetch_series_by_id(pool, content_id, None).await.is_ok()
        || fetch_film_by_id(pool, content_id, None).await.is_ok()
    {
        return Ok(());
    }

    if fetch_live_stream_by_id(pool, content_id).await.is_ok() {
        return Err(AppError::BadRequest(
            "watchlist only supports series and films".to_string(),
        ));
    }

    Err(AppError::NotFound)
}

pub(super) struct ProgressTarget {
    pub(super) kind: String,
    pub(super) episode_id: Option<String>,
    pub(super) duration_sec: i64,
}

pub(super) async fn resolve_progress_target(
    pool: &SqlitePool,
    input: &ProgressInput,
) -> AppResult<ProgressTarget> {
    match input.kind.as_str() {
        "film" => {
            if input.episode_id.is_some() {
                return Err(AppError::BadRequest(
                    "film progress cannot include an episodeId".to_string(),
                ));
            }

            let film = fetch_film_by_id(pool, &input.content_id, None).await?;
            Ok(ProgressTarget {
                kind: "film".to_string(),
                episode_id: None,
                duration_sec: film.duration_sec,
            })
        }
        "series" => {
            let episode_id = input.episode_id.clone().ok_or_else(|| {
                AppError::BadRequest("series progress requires an episodeId".to_string())
            })?;
            fetch_series_by_id(pool, &input.content_id, None).await?;
            let episode = fetch_episode_by_id(pool, &episode_id).await?;
            if episode.series_id != input.content_id {
                return Err(AppError::BadRequest(
                    "episodeId does not belong to the requested series".to_string(),
                ));
            }

            Ok(ProgressTarget {
                kind: "series".to_string(),
                episode_id: Some(episode_id),
                duration_sec: episode.duration_sec,
            })
        }
        _ => Err(AppError::BadRequest(
            "kind must be either 'film' or 'series'".to_string(),
        )),
    }
}

pub(super) async fn fetch_streamer_by_handle(
    pool: &SqlitePool,
    handle: &str,
) -> AppResult<Streamer> {
    let row = sqlx::query(
        "SELECT id, handle, display_name, avatar, bio, followers, is_partner, is_live FROM streamers WHERE handle = ?",
    )
    .bind(handle)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(streamer_from_row(row))
}

pub(super) async fn fetch_live_streams(
    pool: &SqlitePool,
    filter_slug: Option<&str>,
) -> AppResult<Vec<LiveStream>> {
    let fresh_cutoff = stale_live_ingest_cutoff();
    let rows = if let Some(slug) = filter_slug {
        sqlx::query(
            r#"
            SELECT
                ls.id, ls.slug, ls.title, ls.category, ls.tags_json, ls.viewers, ls.started_at,
                ls.thumbnail, ls.language, ls.is_mature, ls.playback_asset_id,
                ls.poster_relative_path, ls.playback_relative_path,
                s.id AS streamer_id, s.handle, s.display_name, s.avatar, s.bio, s.followers,
                s.is_partner, s.is_live
            FROM live_streams ls
            JOIN streamers s ON s.id = ls.streamer_id
            JOIN creator_profiles cp ON cp.handle = s.handle
            WHERE ls.slug = ?
              AND (
                EXISTS (
                    SELECT 1
                    FROM live_ingest_sessions lis
                    WHERE lis.creator_id = cp.id
                      AND lis.status = 'connected'
                      AND lis.last_heartbeat_at >= ?
                )
                OR EXISTS (
                    SELECT 1
                    FROM collaboration_mirror_pickups cmp
                    JOIN live_ingest_sessions lis
                      ON lis.creator_id = cmp.host_creator_id
                     AND lis.broadcast_id = cmp.source_broadcast_id
                    WHERE cmp.guest_creator_id = cp.id
                      AND cmp.guest_broadcast_id = cp.current_broadcast_id
                      AND cmp.state = 'active'
                      AND lis.status = 'connected'
                      AND lis.last_heartbeat_at >= ?
                )
              )
            ORDER BY ls.viewers DESC
            "#,
        )
        .bind(slug)
        .bind(&fresh_cutoff)
        .bind(&fresh_cutoff)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT
                ls.id, ls.slug, ls.title, ls.category, ls.tags_json, ls.viewers, ls.started_at,
                ls.thumbnail, ls.language, ls.is_mature, ls.playback_asset_id,
                ls.poster_relative_path, ls.playback_relative_path,
                s.id AS streamer_id, s.handle, s.display_name, s.avatar, s.bio, s.followers,
                s.is_partner, s.is_live
            FROM live_streams ls
            JOIN streamers s ON s.id = ls.streamer_id
            JOIN creator_profiles cp ON cp.handle = s.handle
            WHERE (
                EXISTS (
                    SELECT 1
                    FROM live_ingest_sessions lis
                    WHERE lis.creator_id = cp.id
                      AND lis.status = 'connected'
                      AND lis.last_heartbeat_at >= ?
                )
                OR EXISTS (
                    SELECT 1
                    FROM collaboration_mirror_pickups cmp
                    JOIN live_ingest_sessions lis
                      ON lis.creator_id = cmp.host_creator_id
                     AND lis.broadcast_id = cmp.source_broadcast_id
                    WHERE cmp.guest_creator_id = cp.id
                      AND cmp.guest_broadcast_id = cp.current_broadcast_id
                      AND cmp.state = 'active'
                      AND lis.status = 'connected'
                      AND lis.last_heartbeat_at >= ?
                )
            )
            ORDER BY ls.viewers DESC
            "#,
        )
        .bind(&fresh_cutoff)
        .bind(&fresh_cutoff)
        .fetch_all(pool)
        .await?
    };

    let mut streams = Vec::with_capacity(rows.len());
    for row in rows {
        let mut stream = live_stream_from_row(row);
        stream.viewers = effective_live_viewer_count(pool, &stream.id).await?;
        streams.push(stream);
    }

    sort_live_streams(&mut streams, "viewers");

    Ok(streams)
}

pub(super) async fn fetch_live_streams_by_category(
    pool: &SqlitePool,
    category: &str,
) -> AppResult<Vec<LiveStream>> {
    let mut streams = fetch_live_streams(pool, None).await?;
    streams.retain(|stream| stream.category == category);
    Ok(streams)
}

pub(super) fn sort_live_streams(streams: &mut [LiveStream], sort: &str) {
    match sort {
        "newest" => streams.sort_by(|left, right| right.started_at.cmp(&left.started_at)),
        _ => streams.sort_by(|left, right| {
            right
                .viewers
                .cmp(&left.viewers)
                .then_with(|| right.started_at.cmp(&left.started_at))
        }),
    }
}

pub(super) async fn fetch_live_stream_by_slug(
    pool: &SqlitePool,
    slug: &str,
) -> AppResult<LiveStream> {
    fetch_live_streams(pool, Some(slug))
        .await?
        .into_iter()
        .next()
        .ok_or(AppError::NotFound)
}

pub(super) async fn fetch_live_stream_by_id(pool: &SqlitePool, id: &str) -> AppResult<LiveStream> {
    let fresh_cutoff = stale_live_ingest_cutoff();
    let row = sqlx::query(
        r#"
        SELECT
            ls.id, ls.slug, ls.title, ls.category, ls.tags_json, ls.viewers, ls.started_at,
            ls.thumbnail, ls.language, ls.is_mature, ls.playback_asset_id,
            ls.poster_relative_path, ls.playback_relative_path,
            s.id AS streamer_id, s.handle, s.display_name, s.avatar, s.bio, s.followers,
            s.is_partner, s.is_live
        FROM live_streams ls
        JOIN streamers s ON s.id = ls.streamer_id
        JOIN creator_profiles cp ON cp.handle = s.handle
        WHERE ls.id = ?
          AND (
            EXISTS (
                SELECT 1
                FROM live_ingest_sessions lis
                WHERE lis.creator_id = cp.id
                  AND lis.status = 'connected'
                  AND lis.last_heartbeat_at >= ?
            )
            OR EXISTS (
                SELECT 1
                FROM collaboration_mirror_pickups cmp
                JOIN live_ingest_sessions lis
                  ON lis.creator_id = cmp.host_creator_id
                 AND lis.broadcast_id = cmp.source_broadcast_id
                WHERE cmp.guest_creator_id = cp.id
                  AND cmp.guest_broadcast_id = cp.current_broadcast_id
                  AND cmp.state = 'active'
                  AND lis.status = 'connected'
                  AND lis.last_heartbeat_at >= ?
            )
          )
        "#,
    )
    .bind(id)
    .bind(&fresh_cutoff)
    .bind(&fresh_cutoff)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let mut stream = live_stream_from_row(row);
    stream.viewers = effective_live_viewer_count(pool, &stream.id).await?;
    Ok(stream)
}

pub(super) async fn fetch_categories(pool: &SqlitePool) -> AppResult<Vec<Category>> {
    let rows = sqlx::query(
        "SELECT slug, name, cover_image, live_viewers, live_channels, tags_json FROM categories ORDER BY live_viewers DESC",
    )
    .fetch_all(pool)
    .await?;

    let categories: Vec<Category> = rows
        .into_iter()
        .map(|row| Category {
            slug: row.get("slug"),
            name: row.get("name"),
            cover_image: row.get("cover_image"),
            live_viewers: row.get("live_viewers"),
            live_channels: row.get("live_channels"),
            tags: from_json(row.get::<String, _>("tags_json")).unwrap_or_default(),
        })
        .collect();

    categories_with_live_totals(pool, categories).await
}

pub(super) async fn fetch_category_by_slug(pool: &SqlitePool, slug: &str) -> AppResult<Category> {
    let row = sqlx::query(
        "SELECT slug, name, cover_image, live_viewers, live_channels, tags_json FROM categories WHERE slug = ?",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let mut categories = categories_with_live_totals(
        pool,
        vec![Category {
            slug: row.get("slug"),
            name: row.get("name"),
            cover_image: row.get("cover_image"),
            live_viewers: row.get("live_viewers"),
            live_channels: row.get("live_channels"),
            tags: from_json(row.get::<String, _>("tags_json"))?,
        }],
    )
    .await?;

    categories.pop().ok_or(AppError::NotFound)
}

async fn categories_with_live_totals(
    pool: &SqlitePool,
    mut categories: Vec<Category>,
) -> AppResult<Vec<Category>> {
    let live_streams = fetch_live_streams(pool, None).await?;
    let mut totals_by_category: HashMap<String, (i64, i64)> = HashMap::new();
    for stream in live_streams {
        let entry = totals_by_category
            .entry(stream.category.clone())
            .or_insert((0, 0));
        entry.0 += stream.viewers;
        entry.1 += 1;
    }

    for category in &mut categories {
        let (live_viewers, live_channels) = totals_by_category
            .get(&category.name)
            .copied()
            .unwrap_or((0, 0));
        category.live_viewers = live_viewers;
        category.live_channels = live_channels;
    }

    categories.sort_by(|left, right| {
        right
            .live_viewers
            .cmp(&left.live_viewers)
            .then_with(|| right.live_channels.cmp(&left.live_channels))
            .then_with(|| left.name.cmp(&right.name))
    });

    Ok(categories)
}

pub(super) async fn fetch_user(pool: &SqlitePool, user_id: &str) -> AppResult<User> {
    let row = sqlx::query(
        "SELECT id, handle, display_name, avatar, tier, joined_at FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let watchlist = sqlx::query("SELECT content_id FROM user_watchlist WHERE user_id = ?")
        .bind(user_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|item| item.get("content_id"))
        .collect();

    let following = sqlx::query("SELECT streamer_id FROM user_following WHERE user_id = ?")
        .bind(user_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|item| item.get("streamer_id"))
        .collect();

    let continue_watching = sqlx::query(
        "SELECT content_id, kind, episode_id, progress_sec, duration_sec, last_watched_at FROM continue_watching WHERE user_id = ? ORDER BY last_watched_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|item| ContinueWatchingEntry {
        content_id: item.get("content_id"),
        kind: item.get("kind"),
        episode_id: item.get("episode_id"),
        progress_sec: item.get("progress_sec"),
        duration_sec: item.get("duration_sec"),
        last_watched_at: item.get("last_watched_at"),
    })
    .collect();

    Ok(User {
        id: row.get("id"),
        handle: row.get("handle"),
        display_name: row.get("display_name"),
        avatar: row.get("avatar"),
        tier: row.get("tier"),
        joined_at: row.get("joined_at"),
        watchlist,
        following,
        continue_watching,
    })
}

pub(super) async fn fetch_watch_history(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<Vec<WatchHistoryEntry>> {
    Ok(sqlx::query(
        r#"
        SELECT content_id, kind, episode_id, progress_sec, duration_sec,
               completed, completed_at, last_watched_at
        FROM user_watch_history
        WHERE user_id = ?
        ORDER BY last_watched_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|item| WatchHistoryEntry {
        content_id: item.get("content_id"),
        kind: item.get("kind"),
        episode_id: item.get("episode_id"),
        progress_sec: item.get("progress_sec"),
        duration_sec: item.get("duration_sec"),
        completed: item.get::<i64, _>("completed") == 1,
        completed_at: item.get("completed_at"),
        last_watched_at: item.get("last_watched_at"),
    })
    .collect())
}

pub(super) async fn fetch_user_library(pool: &SqlitePool, user_id: &str) -> AppResult<UserLibrary> {
    let user = fetch_user(pool, user_id).await?;
    let entitlements = fetch_user_entitlements(pool, user_id).await?;
    Ok(UserLibrary {
        continue_watching: user.continue_watching,
        history: fetch_watch_history(pool, user_id).await?,
        memberships: entitlements.memberships,
        purchases: entitlements.purchases,
    })
}

pub(super) async fn fetch_watchlist_response(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<WatchlistResponse> {
    let watchlist_ids: Vec<String> = sqlx::query(
        "SELECT content_id FROM user_watchlist WHERE user_id = ? ORDER BY content_id ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|item| item.get("content_id"))
    .collect();

    let mut series = Vec::new();
    let mut films = Vec::new();
    for content_id in watchlist_ids {
        if let Ok(item) = fetch_series_by_id(pool, &content_id, None).await {
            series.push(item);
            continue;
        }
        if let Ok(item) = fetch_film_by_id(pool, &content_id, None).await {
            films.push(item);
        }
    }

    Ok(WatchlistResponse {
        total_titles: (series.len() + films.len()) as i64,
        series,
        films,
    })
}

pub(super) async fn fetch_viewer_app_state(
    pool: &SqlitePool,
    user_id: &str,
    current_session_id: &str,
) -> AppResult<ViewerAppState> {
    let user = fetch_user(pool, user_id).await?;
    let library = fetch_user_library(pool, user_id).await?;
    let watchlist = fetch_watchlist_response(pool, user_id).await?;

    let followed_streamer_ids = fetch_followed_streamer_ids(pool, user_id).await?;
    let mut followed_streamers = Vec::with_capacity(followed_streamer_ids.len());
    for streamer_id in &followed_streamer_ids {
        followed_streamers.push(fetch_streamer_by_id(pool, streamer_id).await?);
    }
    let followed_streamer_id_set: std::collections::HashSet<_> =
        followed_streamer_ids.into_iter().collect();
    let live_streams: Vec<LiveStream> = fetch_live_streams(pool, None)
        .await?
        .into_iter()
        .filter(|stream| followed_streamer_id_set.contains(&stream.streamer.id))
        .collect();
    let following = FollowingFeedResponse {
        total_followed_streamers: followed_streamers.len() as i64,
        live_now_count: live_streams.len() as i64,
        followed_streamers,
        live_streams,
    };

    Ok(ViewerAppState {
        user,
        library,
        watchlist,
        following,
        entitlements: fetch_user_entitlements(pool, user_id).await?,
        profile: fetch_user_profile_details(pool, user_id).await?,
        settings: fetch_user_settings_bundle(pool, user_id).await?,
        plan: fetch_billing_plan(pool, user_id).await?,
        notifications: fetch_user_notifications(pool, user_id).await?,
        sessions: fetch_auth_sessions(pool, user_id, current_session_id).await?,
    })
}

pub(super) async fn fetch_followed_streamer_ids(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<Vec<String>> {
    Ok(
        sqlx::query(
            "SELECT streamer_id FROM user_following WHERE user_id = ? ORDER BY streamer_id",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|item| item.get("streamer_id"))
        .collect(),
    )
}

pub(super) async fn fetch_creator_id_for_user(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<Option<String>> {
    let row = sqlx::query("SELECT id FROM creator_profiles WHERE user_id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| row.get("id")))
}

pub(super) async fn fetch_connected_accounts(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<Vec<ConnectedAccount>> {
    let rows = sqlx::query(
        "SELECT id, provider, display_name, connected_at FROM connected_accounts WHERE user_id = ? ORDER BY connected_at ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| ConnectedAccount {
            id: row.get("id"),
            provider: row.get("provider"),
            display_name: row.get("display_name"),
            connected_at: row.get("connected_at"),
        })
        .collect())
}

pub(super) async fn fetch_user_profile_details(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<UserProfileDetails> {
    let user = fetch_user(pool, user_id).await?;
    let row = sqlx::query(
        r#"
        SELECT email, email_verified, mature_content_allowed, default_audio,
               subtitle_preset, autoplay_trailers, live_chat_filter, hours_watched
        FROM user_profiles
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(UserProfileDetails {
        user,
        email: row.get("email"),
        email_verified: row.get::<i64, _>("email_verified") == 1,
        mature_content_allowed: row.get::<i64, _>("mature_content_allowed") == 1,
        default_audio: row.get("default_audio"),
        subtitle_preset: row.get("subtitle_preset"),
        autoplay_trailers: row.get::<i64, _>("autoplay_trailers") == 1,
        live_chat_filter: row.get("live_chat_filter"),
        hours_watched: row.get("hours_watched"),
        connected_accounts: fetch_connected_accounts(pool, user_id).await?,
    })
}

pub(super) async fn fetch_user_settings_bundle(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<UserSettingsBundle> {
    let playback_row = sqlx::query(
        r#"
        SELECT default_quality, audio_language, subtitle_language, subtitle_style,
               autoplay_next_episode, autoplay_trailers, reduced_motion, prefer_dubbed, playback_speed
        FROM user_playback_settings
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let notification_row = sqlx::query(
        r#"
        SELECT series_push, series_email, live_push, live_email, originals_push, originals_email,
               watchlist_push, watchlist_email, creator_push, creator_email, security_push, security_email
        FROM user_notification_settings
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let privacy_row = sqlx::query(
        r#"
        SELECT show_friend_activity, improve_recommendations, personalized_ads,
               ab_tests, data_export_size_mb, delete_cooldown_days
        FROM user_privacy_settings
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let parental_row = sqlx::query(
        r#"
        SELECT max_rating, require_pin_for_mature, hide_live_chat_for_kids,
               block_mature_live_streams, pin_set
        FROM user_parental_controls
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let download_row = sqlx::query(
        r#"
        SELECT video_quality, wifi_only, smart_downloads, storage_used_gb,
               storage_limit_gb, device_limit, active_devices
        FROM user_download_settings
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let language_row = sqlx::query(
        r#"
        SELECT interface_language, subtitle_language, catalog_region, date_format, clock_format
        FROM user_language_settings
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(UserSettingsBundle {
        playback: PlaybackSettings {
            default_quality: playback_row.get("default_quality"),
            audio_language: playback_row.get("audio_language"),
            subtitle_language: playback_row.get("subtitle_language"),
            subtitle_style: playback_row.get("subtitle_style"),
            autoplay_next_episode: playback_row.get::<i64, _>("autoplay_next_episode") == 1,
            autoplay_trailers: playback_row.get::<i64, _>("autoplay_trailers") == 1,
            reduced_motion: playback_row.get::<i64, _>("reduced_motion") == 1,
            prefer_dubbed: playback_row.get::<i64, _>("prefer_dubbed") == 1,
            playback_speed: playback_row.get("playback_speed"),
        },
        notifications: NotificationSettings {
            series_releases: NotificationChannelSetting {
                label: "New episodes of series I watch".to_string(),
                push: notification_row.get::<i64, _>("series_push") == 1,
                email: notification_row.get::<i64, _>("series_email") == 1,
                lock: false,
            },
            live_streams: NotificationChannelSetting {
                label: "Followed streamers go live".to_string(),
                push: notification_row.get::<i64, _>("live_push") == 1,
                email: notification_row.get::<i64, _>("live_email") == 1,
                lock: false,
            },
            originals: NotificationChannelSetting {
                label: "LIFESTREAM Originals premieres".to_string(),
                push: notification_row.get::<i64, _>("originals_push") == 1,
                email: notification_row.get::<i64, _>("originals_email") == 1,
                lock: false,
            },
            watchlist_updates: NotificationChannelSetting {
                label: "Watchlist price drops".to_string(),
                push: notification_row.get::<i64, _>("watchlist_push") == 1,
                email: notification_row.get::<i64, _>("watchlist_email") == 1,
                lock: false,
            },
            creator_updates: NotificationChannelSetting {
                label: "Creator tools & product updates".to_string(),
                push: notification_row.get::<i64, _>("creator_push") == 1,
                email: notification_row.get::<i64, _>("creator_email") == 1,
                lock: false,
            },
            security_alerts: NotificationChannelSetting {
                label: "Security alerts".to_string(),
                push: notification_row.get::<i64, _>("security_push") == 1,
                email: notification_row.get::<i64, _>("security_email") == 1,
                lock: true,
            },
        },
        privacy: PrivacySettings {
            show_friend_activity: privacy_row.get::<i64, _>("show_friend_activity") == 1,
            improve_recommendations: privacy_row.get::<i64, _>("improve_recommendations") == 1,
            personalized_ads: privacy_row.get::<i64, _>("personalized_ads") == 1,
            ab_tests: privacy_row.get::<i64, _>("ab_tests") == 1,
            data_export_size_mb: privacy_row.get("data_export_size_mb"),
            delete_cooldown_days: privacy_row.get("delete_cooldown_days"),
        },
        parental: ParentalControls {
            max_rating: parental_row.get("max_rating"),
            require_pin_for_mature: parental_row.get::<i64, _>("require_pin_for_mature") == 1,
            hide_live_chat_for_kids: parental_row.get::<i64, _>("hide_live_chat_for_kids") == 1,
            block_mature_live_streams: parental_row.get::<i64, _>("block_mature_live_streams") == 1,
            pin_set: parental_row.get::<i64, _>("pin_set") == 1,
        },
        downloads: DownloadSettings {
            video_quality: download_row.get("video_quality"),
            wifi_only: download_row.get::<i64, _>("wifi_only") == 1,
            smart_downloads: download_row.get::<i64, _>("smart_downloads") == 1,
            storage_used_gb: download_row.get("storage_used_gb"),
            storage_limit_gb: download_row.get("storage_limit_gb"),
            device_limit: download_row.get("device_limit"),
            active_devices: download_row.get("active_devices"),
        },
        language: LanguageSettings {
            interface_language: language_row.get("interface_language"),
            subtitle_language: language_row.get("subtitle_language"),
            catalog_region: language_row.get("catalog_region"),
            date_format: language_row.get("date_format"),
            clock_format: language_row.get("clock_format"),
        },
    })
}

pub(super) async fn fetch_billing_plan(pool: &SqlitePool, user_id: &str) -> AppResult<BillingPlan> {
    let row = sqlx::query(
        r#"
        SELECT plan_name, monthly_price, next_renewal_date, payment_brand, payment_last4,
               billing_city, billing_region, billing_country, invoices_count, screens,
               features_json, average_revenue_per_user
        FROM billing_profiles
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(BillingPlan {
        plan_name: row.get("plan_name"),
        monthly_price: row.get("monthly_price"),
        next_renewal_date: row.get("next_renewal_date"),
        payment_brand: row.get("payment_brand"),
        payment_last4: row.get("payment_last4"),
        billing_city: row.get("billing_city"),
        billing_region: row.get("billing_region"),
        billing_country: row.get("billing_country"),
        invoices_count: row.get("invoices_count"),
        screens: row.get("screens"),
        features: from_json(row.get::<String, _>("features_json"))?,
        average_revenue_per_user: row.get("average_revenue_per_user"),
    })
}
