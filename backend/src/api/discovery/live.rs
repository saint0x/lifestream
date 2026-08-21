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
                CASE
                    WHEN ls.playback_asset_id IS NOT NULL
                     AND ls.playback_relative_path IS NOT NULL
                     AND (
                        EXISTS (
                            SELECT 1
                            FROM live_ingest_sessions lis_ready
                            WHERE lis_ready.creator_id = cp.id
                              AND lis_ready.status = 'connected'
                              AND lis_ready.last_heartbeat_at >= ?
                              AND lis_ready.last_source_probe_at IS NOT NULL
                        )
                        OR EXISTS (
                            SELECT 1
                            FROM collaboration_mirror_pickups cmp_ready
                            JOIN live_ingest_sessions lis_ready
                              ON lis_ready.creator_id = cmp_ready.host_creator_id
                             AND lis_ready.broadcast_id = cmp_ready.source_broadcast_id
                            WHERE cmp_ready.guest_creator_id = cp.id
                              AND cmp_ready.guest_broadcast_id = cp.current_broadcast_id
                              AND cmp_ready.state = 'active'
                              AND lis_ready.status = 'connected'
                              AND lis_ready.last_heartbeat_at >= ?
                              AND lis_ready.last_source_probe_at IS NOT NULL
                        )
                     )
                    THEN 1 ELSE 0
                END AS playback_ready,
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
        .bind(&fresh_cutoff)
        .bind(&fresh_cutoff)
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
                CASE
                    WHEN ls.playback_asset_id IS NOT NULL
                     AND ls.playback_relative_path IS NOT NULL
                     AND (
                        EXISTS (
                            SELECT 1
                            FROM live_ingest_sessions lis_ready
                            WHERE lis_ready.creator_id = cp.id
                              AND lis_ready.status = 'connected'
                              AND lis_ready.last_heartbeat_at >= ?
                              AND lis_ready.last_source_probe_at IS NOT NULL
                        )
                        OR EXISTS (
                            SELECT 1
                            FROM collaboration_mirror_pickups cmp_ready
                            JOIN live_ingest_sessions lis_ready
                              ON lis_ready.creator_id = cmp_ready.host_creator_id
                             AND lis_ready.broadcast_id = cmp_ready.source_broadcast_id
                            WHERE cmp_ready.guest_creator_id = cp.id
                              AND cmp_ready.guest_broadcast_id = cp.current_broadcast_id
                              AND cmp_ready.state = 'active'
                              AND lis_ready.status = 'connected'
                              AND lis_ready.last_heartbeat_at >= ?
                              AND lis_ready.last_source_probe_at IS NOT NULL
                        )
                     )
                    THEN 1 ELSE 0
                END AS playback_ready,
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
        .bind(&fresh_cutoff)
        .bind(&fresh_cutoff)
        .fetch_all(pool)
        .await?
    };

    let mut streams = rows.into_iter().map(live_stream_from_row).collect::<Vec<_>>();
    apply_effective_live_viewer_counts(pool, &mut streams).await?;
    sort_live_streams(&mut streams, "viewers");

    Ok(streams)
}

pub(crate) async fn fetch_followed_live_streams(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<Vec<LiveStream>> {
    let fresh_cutoff = stale_live_ingest_cutoff();
    let rows = sqlx::query(
        r#"
        SELECT
            ls.id, ls.slug, ls.title, ls.category, ls.tags_json, ls.viewers, ls.started_at,
            ls.thumbnail, ls.language, ls.is_mature, ls.playback_asset_id,
            ls.poster_relative_path, ls.playback_relative_path,
            CASE
                WHEN ls.playback_asset_id IS NOT NULL
                 AND ls.playback_relative_path IS NOT NULL
                 AND (
                    EXISTS (
                        SELECT 1
                        FROM live_ingest_sessions lis_ready
                        WHERE lis_ready.creator_id = cp.id
                          AND lis_ready.status = 'connected'
                          AND lis_ready.last_heartbeat_at >= ?
                          AND lis_ready.last_source_probe_at IS NOT NULL
                    )
                    OR EXISTS (
                        SELECT 1
                        FROM collaboration_mirror_pickups cmp_ready
                        JOIN live_ingest_sessions lis_ready
                          ON lis_ready.creator_id = cmp_ready.host_creator_id
                         AND lis_ready.broadcast_id = cmp_ready.source_broadcast_id
                        WHERE cmp_ready.guest_creator_id = cp.id
                          AND cmp_ready.guest_broadcast_id = cp.current_broadcast_id
                          AND cmp_ready.state = 'active'
                          AND lis_ready.status = 'connected'
                          AND lis_ready.last_heartbeat_at >= ?
                          AND lis_ready.last_source_probe_at IS NOT NULL
                    )
                 )
                THEN 1 ELSE 0
            END AS playback_ready,
            s.id AS streamer_id, s.handle, s.display_name, s.avatar, s.bio, s.followers,
            s.is_partner, s.is_live
        FROM user_following uf
        JOIN streamers s ON s.id = uf.streamer_id
        JOIN live_streams ls ON ls.streamer_id = s.id
        JOIN creator_profiles cp ON cp.handle = s.handle
        WHERE uf.user_id = ?
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
    .bind(&fresh_cutoff)
    .bind(&fresh_cutoff)
    .bind(user_id)
    .bind(&fresh_cutoff)
    .bind(&fresh_cutoff)
    .fetch_all(pool)
    .await?;

    let mut streams = rows.into_iter().map(live_stream_from_row).collect::<Vec<_>>();
    apply_effective_live_viewer_counts(pool, &mut streams).await?;
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
            CASE
                WHEN ls.playback_asset_id IS NOT NULL
                 AND ls.playback_relative_path IS NOT NULL
                 AND (
                    EXISTS (
                        SELECT 1
                        FROM live_ingest_sessions lis_ready
                        WHERE lis_ready.creator_id = cp.id
                          AND lis_ready.status = 'connected'
                          AND lis_ready.last_heartbeat_at >= ?
                          AND lis_ready.last_source_probe_at IS NOT NULL
                    )
                    OR EXISTS (
                        SELECT 1
                        FROM collaboration_mirror_pickups cmp_ready
                        JOIN live_ingest_sessions lis_ready
                          ON lis_ready.creator_id = cmp_ready.host_creator_id
                         AND lis_ready.broadcast_id = cmp_ready.source_broadcast_id
                        WHERE cmp_ready.guest_creator_id = cp.id
                          AND cmp_ready.guest_broadcast_id = cp.current_broadcast_id
                          AND cmp_ready.state = 'active'
                          AND lis_ready.status = 'connected'
                          AND lis_ready.last_heartbeat_at >= ?
                          AND lis_ready.last_source_probe_at IS NOT NULL
                    )
                 )
                THEN 1 ELSE 0
            END AS playback_ready,
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
    .bind(&fresh_cutoff)
    .bind(&fresh_cutoff)
    .bind(id)
    .bind(&fresh_cutoff)
    .bind(&fresh_cutoff)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let mut stream = live_stream_from_row(row);
    apply_effective_live_viewer_counts(pool, std::slice::from_mut(&mut stream)).await?;
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
        .map(category_from_row)
        .collect();

    categories_with_live_totals(pool, categories).await
}

pub(crate) async fn fetch_categories_for_live_streams(
    pool: &SqlitePool,
    live_streams: &[LiveStream],
) -> AppResult<Vec<Category>> {
    let rows = sqlx::query(
        "SELECT slug, name, cover_image, live_viewers, live_channels, tags_json FROM categories ORDER BY live_viewers DESC",
    )
    .fetch_all(pool)
    .await?;

    apply_category_live_totals(rows.into_iter().map(category_from_row).collect(), live_streams)
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
        vec![category_from_row(row)],
    )
    .await?;

    categories.pop().ok_or(AppError::NotFound)
}

async fn categories_with_live_totals(
    pool: &SqlitePool,
    categories: Vec<Category>,
) -> AppResult<Vec<Category>> {
    let live_streams = fetch_live_streams(pool, None).await?;
    apply_category_live_totals(categories, &live_streams)
}

async fn apply_effective_live_viewer_counts(
    pool: &SqlitePool,
    streams: &mut [LiveStream],
) -> AppResult<()> {
    if streams.is_empty() {
        return Ok(());
    }

    let active_cutoff = active_presence_cutoff();
    let mut query = sqlx::QueryBuilder::new(
        r#"
        SELECT stream_id, COUNT(*) AS count
        FROM (
            SELECT stream_id, COALESCE('u:' || user_id, 's:' || session_token_hash) AS viewer_key
            FROM live_viewer_sessions
            WHERE disconnected_at IS NULL
              AND last_seen_at >= 
        "#,
    );
    query.push_bind(active_cutoff);
    query.push(" AND stream_id IN (");
    {
        let mut separated = query.separated(", ");
        for stream in streams.iter() {
            separated.push_bind(stream.id.as_str());
        }
    }
    query.push(
        r#")
            GROUP BY stream_id, viewer_key
        ) active_viewers
        GROUP BY stream_id
        "#,
    );

    let rows = query.build().fetch_all(pool).await?;
    let connected_counts = rows
        .into_iter()
        .map(|row| (row.get::<String, _>("stream_id"), row.get::<i64, _>("count")))
        .collect::<HashMap<_, _>>();

    for stream in streams {
        if let Some(connected) = connected_counts.get(&stream.id) {
            stream.viewers = stream.viewers.max(*connected);
        }
    }

    Ok(())
}

fn category_from_row(row: sqlx::sqlite::SqliteRow) -> Category {
    Category {
        slug: row.get("slug"),
        name: row.get("name"),
        cover_image: row.get("cover_image"),
        live_viewers: row.get("live_viewers"),
        live_channels: row.get("live_channels"),
        tags: from_json(row.get::<String, _>("tags_json")).unwrap_or_default(),
    }
}

fn apply_category_live_totals(
    mut categories: Vec<Category>,
    live_streams: &[LiveStream],
) -> AppResult<Vec<Category>> {
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
