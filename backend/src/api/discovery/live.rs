use super::*;

pub(crate) async fn fetch_streamers(pool: &SqlitePool) -> AppResult<Vec<Streamer>> {
    let rows = sqlx::query(
        "SELECT id, handle, display_name, avatar, bio, followers, is_partner, is_live FROM streamers ORDER BY followers DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(streamer_from_row).collect())
}

pub(crate) async fn fetch_streamer_by_id(pool: &SqlitePool, id: &str) -> AppResult<Streamer> {
    let row = sqlx::query(
        "SELECT id, handle, display_name, avatar, bio, followers, is_partner, is_live FROM streamers WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(streamer_from_row(row))
}

pub(crate) async fn fetch_streamer_by_handle(
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

pub(crate) async fn fetch_live_streams(
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

pub(crate) async fn fetch_live_streams_by_category(
    pool: &SqlitePool,
    category: &str,
) -> AppResult<Vec<LiveStream>> {
    let mut streams = fetch_live_streams(pool, None).await?;
    streams.retain(|stream| stream.category == category);
    Ok(streams)
}

pub(crate) fn sort_live_streams(streams: &mut [LiveStream], sort: &str) {
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

pub(crate) async fn fetch_live_stream_by_slug(
    pool: &SqlitePool,
    slug: &str,
) -> AppResult<LiveStream> {
    fetch_live_streams(pool, Some(slug))
        .await?
        .into_iter()
        .next()
        .ok_or(AppError::NotFound)
}

pub(crate) async fn fetch_live_stream_by_id(pool: &SqlitePool, id: &str) -> AppResult<LiveStream> {
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

pub(crate) async fn fetch_categories(pool: &SqlitePool) -> AppResult<Vec<Category>> {
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

pub(crate) async fn fetch_category_by_slug(pool: &SqlitePool, slug: &str) -> AppResult<Category> {
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
