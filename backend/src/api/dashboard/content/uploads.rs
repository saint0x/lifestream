use super::*;

pub(crate) async fn fetch_creator_content_summary(
    pool: &SqlitePool,
    creator_id: &str,
    query: &CreatorContentQuery,
) -> AppResult<CreatorContentSummary> {
    let totals = sqlx::query(
        r#"
        SELECT
            COUNT(*) AS total_uploads,
            SUM(CASE WHEN status = 'published' THEN 1 ELSE 0 END) AS published_uploads,
            SUM(CASE WHEN status = 'scheduled' THEN 1 ELSE 0 END) AS scheduled_uploads,
            SUM(CASE WHEN status = 'processing' THEN 1 ELSE 0 END) AS processing_uploads,
            SUM(CASE WHEN status = 'draft' THEN 1 ELSE 0 END) AS draft_uploads,
            SUM(CASE WHEN status = 'archived' THEN 1 ELSE 0 END) AS archived_uploads,
            COALESCE(SUM(views), 0) AS total_views,
            COALESCE(SUM(watch_hours), 0) AS total_watch_hours,
            COALESCE(SUM(size_bytes), 0) AS total_storage_bytes
        FROM uploads
        WHERE creator_id = ?
        "#,
    )
    .bind(creator_id)
    .fetch_one(pool)
    .await?;

    let filtered_count = fetch_filtered_upload_count(pool, creator_id, query).await?;

    Ok(CreatorContentSummary {
        total_uploads: totals.get("total_uploads"),
        published_uploads: totals
            .get::<Option<i64>, _>("published_uploads")
            .unwrap_or(0),
        scheduled_uploads: totals
            .get::<Option<i64>, _>("scheduled_uploads")
            .unwrap_or(0),
        processing_uploads: totals
            .get::<Option<i64>, _>("processing_uploads")
            .unwrap_or(0),
        draft_uploads: totals.get::<Option<i64>, _>("draft_uploads").unwrap_or(0),
        archived_uploads: totals
            .get::<Option<i64>, _>("archived_uploads")
            .unwrap_or(0),
        total_views: totals.get("total_views"),
        total_watch_hours: totals.get("total_watch_hours"),
        total_storage_bytes: totals.get("total_storage_bytes"),
        filtered_count,
    })
}

pub(crate) async fn fetch_uploads(pool: &SqlitePool, creator_id: &str) -> AppResult<Vec<Upload>> {
    publish_due_scheduled_upload_releases(pool, Some(creator_id), None).await?;
    fetch_uploads_unreconciled(pool, creator_id).await
}

pub(crate) async fn fetch_uploads_for_database(
    database: &crate::db::Database,
    creator_id: &str,
) -> AppResult<Vec<Upload>> {
    if let Ok(pool) = database.try_postgres_adapter() {
        publish_due_postgres_scheduled_upload_releases(pool, Some(creator_id), None).await?;
        return fetch_postgres_uploads_unreconciled(pool, creator_id).await;
    }
    fetch_uploads(database.try_sqlite_adapter()?, creator_id).await
}

pub(crate) async fn fetch_uploads_unreconciled(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<Upload>> {
    fetch_filtered_uploads_unreconciled(
        pool,
        creator_id,
        &CreatorContentQuery {
            kind: None,
            status: None,
            q: None,
            sort: None,
        },
        None,
    )
    .await
}

pub(crate) async fn fetch_filtered_uploads_for_database(
    database: &crate::db::Database,
    creator_id: &str,
    query: &CreatorContentQuery,
    limit: Option<usize>,
) -> AppResult<Vec<Upload>> {
    if let Ok(pool) = database.try_postgres_adapter() {
        publish_due_postgres_scheduled_upload_releases(pool, Some(creator_id), None).await?;
        return fetch_postgres_filtered_uploads_unreconciled(pool, creator_id, query, limit).await;
    }
    fetch_filtered_uploads_unreconciled(database.try_sqlite_adapter()?, creator_id, query, limit)
        .await
}

pub(crate) async fn fetch_filtered_uploads_unreconciled(
    pool: &SqlitePool,
    creator_id: &str,
    query: &CreatorContentQuery,
    limit: Option<usize>,
) -> AppResult<Vec<Upload>> {
    let mut builder = sqlx::QueryBuilder::new(
        "SELECT id, slug, title, description, kind, duration_sec, uploaded_at, published_at, release_at, status, visibility, access_policy, access_tier_id, price_cents, currency, rental_window_hours, views, likes, comments, watch_hours, thumbnail, series_title, season_number, episode_number, size_bytes, resolution, transcode_progress FROM uploads WHERE creator_id = ",
    );
    builder.push_bind(creator_id);
    apply_upload_filters(&mut builder, query);
    apply_upload_sort(&mut builder, query)?;
    if let Some(limit) = limit {
        builder.push(" LIMIT ");
        builder.push_bind(limit.max(1) as i64);
    }
    let rows = builder.build().fetch_all(pool).await?;
    Ok(rows.into_iter().map(upload_from_row).collect())
}

pub(crate) async fn fetch_upload_by_id(
    pool: &SqlitePool,
    creator_id: &str,
    id: &str,
) -> AppResult<Upload> {
    publish_due_scheduled_upload_releases(pool, Some(creator_id), Some(id)).await?;
    fetch_upload_by_id_unreconciled(pool, creator_id, id).await
}

pub(crate) async fn fetch_upload_by_id_for_database(
    database: &crate::db::Database,
    creator_id: &str,
    id: &str,
) -> AppResult<Upload> {
    if let Ok(pool) = database.try_postgres_adapter() {
        publish_due_postgres_scheduled_upload_releases(pool, Some(creator_id), Some(id)).await?;
        return fetch_postgres_upload_by_id_unreconciled(pool, creator_id, id).await;
    }
    fetch_upload_by_id(database.try_sqlite_adapter()?, creator_id, id).await
}

pub(crate) async fn fetch_upload_by_id_unreconciled(
    pool: &SqlitePool,
    creator_id: &str,
    id: &str,
) -> AppResult<Upload> {
    let row = sqlx::query(
        "SELECT id, slug, title, description, kind, duration_sec, uploaded_at, published_at, release_at, status, visibility, access_policy, access_tier_id, price_cents, currency, rental_window_hours, views, likes, comments, watch_hours, thumbnail, series_title, season_number, episode_number, size_bytes, resolution, transcode_progress FROM uploads WHERE creator_id = ? AND id = ?",
    )
    .bind(creator_id)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(upload_from_row(row))
}

async fn publish_due_postgres_scheduled_upload_releases(
    pool: &sqlx::PgPool,
    creator_id: Option<&str>,
    upload_id: Option<&str>,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let mut builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "UPDATE uploads SET status = 'published', published_at = COALESCE(published_at, ",
    );
    builder.push_bind(now.clone());
    builder.push(") WHERE status = 'scheduled' AND visibility = 'public' AND release_at IS NOT NULL AND release_at <= ");
    builder.push_bind(now);
    if let Some(creator_id) = creator_id {
        builder.push(" AND creator_id = ");
        builder.push_bind(creator_id);
    }
    if let Some(upload_id) = upload_id {
        builder.push(" AND id = ");
        builder.push_bind(upload_id);
    }
    builder.build().execute(pool).await?;
    Ok(())
}

async fn fetch_postgres_uploads_unreconciled(
    pool: &sqlx::PgPool,
    creator_id: &str,
) -> AppResult<Vec<Upload>> {
    fetch_postgres_filtered_uploads_unreconciled(
        pool,
        creator_id,
        &CreatorContentQuery {
            kind: None,
            status: None,
            q: None,
            sort: None,
        },
        None,
    )
    .await
}

async fn fetch_postgres_filtered_uploads_unreconciled(
    pool: &sqlx::PgPool,
    creator_id: &str,
    query: &CreatorContentQuery,
    limit: Option<usize>,
) -> AppResult<Vec<Upload>> {
    let mut builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT id, slug, title, description, kind, duration_sec::BIGINT AS duration_sec, uploaded_at, published_at, release_at, status, visibility, access_policy, access_tier_id, price_cents::BIGINT AS price_cents, currency, rental_window_hours::BIGINT AS rental_window_hours, views::BIGINT AS views, likes::BIGINT AS likes, comments::BIGINT AS comments, watch_hours::BIGINT AS watch_hours, thumbnail, series_title, season_number::BIGINT AS season_number, episode_number::BIGINT AS episode_number, size_bytes::BIGINT AS size_bytes, resolution, transcode_progress::DOUBLE PRECISION AS transcode_progress FROM uploads WHERE creator_id = ",
    );
    builder.push_bind(creator_id);
    apply_postgres_upload_filters(&mut builder, query);
    apply_postgres_upload_sort(&mut builder, query)?;
    if let Some(limit) = limit {
        builder.push(" LIMIT ");
        builder.push_bind(limit.max(1) as i64);
    }
    let rows = builder.build().fetch_all(pool).await?;
    Ok(rows.into_iter().map(postgres_upload_from_row).collect())
}

async fn fetch_postgres_upload_by_id_unreconciled(
    pool: &sqlx::PgPool,
    creator_id: &str,
    id: &str,
) -> AppResult<Upload> {
    let row = sqlx::query(
        r#"
        SELECT id, slug, title, description, kind, duration_sec::BIGINT AS duration_sec,
               uploaded_at, published_at, release_at, status, visibility, access_policy,
               access_tier_id, price_cents::BIGINT AS price_cents, currency,
               rental_window_hours::BIGINT AS rental_window_hours, views::BIGINT AS views,
               likes::BIGINT AS likes, comments::BIGINT AS comments,
               watch_hours::BIGINT AS watch_hours, thumbnail, series_title,
               season_number::BIGINT AS season_number, episode_number::BIGINT AS episode_number,
               size_bytes::BIGINT AS size_bytes, resolution,
               transcode_progress::DOUBLE PRECISION AS transcode_progress
        FROM uploads
        WHERE creator_id = $1 AND id = $2
        "#,
    )
    .bind(creator_id)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(postgres_upload_from_row(row))
}

fn apply_postgres_upload_filters<'a>(
    builder: &mut sqlx::QueryBuilder<'a, sqlx::Postgres>,
    query: &'a CreatorContentQuery,
) {
    if let Some(kind) = query.kind.as_deref().filter(|kind| *kind != "all") {
        builder.push(" AND kind = ");
        builder.push_bind(kind);
    }
    if let Some(status) = query.status.as_deref().filter(|status| *status != "all") {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    if let Some(q) = query.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        builder.push(" AND lower(title) LIKE ");
        builder.push_bind(format!("%{}%", q.to_lowercase()));
    }
}

fn apply_postgres_upload_sort(
    builder: &mut sqlx::QueryBuilder<'_, sqlx::Postgres>,
    query: &CreatorContentQuery,
) -> AppResult<()> {
    match query.sort.as_deref().unwrap_or("uploaded") {
        "uploaded" => builder.push(" ORDER BY uploaded_at DESC"),
        "views" => builder.push(" ORDER BY views DESC"),
        "hours" => builder.push(" ORDER BY watch_hours DESC"),
        "title" => builder.push(" ORDER BY title ASC"),
        other => {
            return Err(AppError::BadRequest(format!(
                "unsupported creator content sort: {other}"
            )));
        }
    };
    Ok(())
}

fn postgres_upload_from_row(row: sqlx::postgres::PgRow) -> Upload {
    Upload {
        id: row.get("id"),
        slug: row.get("slug"),
        title: row.get("title"),
        description: row.get("description"),
        kind: row.get("kind"),
        duration_sec: row.get("duration_sec"),
        uploaded_at: row.get("uploaded_at"),
        published_at: row.get("published_at"),
        release_at: row.get("release_at"),
        status: row.get("status"),
        visibility: row.get("visibility"),
        access_policy: row.get("access_policy"),
        access_tier_id: row.get("access_tier_id"),
        price_cents: row.get("price_cents"),
        currency: row.get("currency"),
        rental_window_hours: row.get("rental_window_hours"),
        views: row.get("views"),
        likes: row.get("likes"),
        comments: row.get("comments"),
        watch_hours: row.get("watch_hours"),
        thumbnail: row.get("thumbnail"),
        series_title: row.get("series_title"),
        season_number: row.get("season_number"),
        episode_number: row.get("episode_number"),
        size_bytes: row.get("size_bytes"),
        resolution: row.get("resolution"),
        transcode_progress: row.get("transcode_progress"),
    }
}

async fn fetch_filtered_upload_count(
    pool: &SqlitePool,
    creator_id: &str,
    query: &CreatorContentQuery,
) -> AppResult<i64> {
    let mut builder =
        sqlx::QueryBuilder::new("SELECT COUNT(*) AS count FROM uploads WHERE creator_id = ");
    builder.push_bind(creator_id);
    apply_upload_filters(&mut builder, query);
    let row = builder.build().fetch_one(pool).await?;
    Ok(row.get("count"))
}

fn apply_upload_filters<'a>(
    builder: &mut sqlx::QueryBuilder<'a, sqlx::Sqlite>,
    query: &'a CreatorContentQuery,
) {
    if let Some(kind) = query.kind.as_deref().filter(|kind| *kind != "all") {
        builder.push(" AND kind = ");
        builder.push_bind(kind);
    }
    if let Some(status) = query.status.as_deref().filter(|status| *status != "all") {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    if let Some(q) = query.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        builder.push(" AND lower(title) LIKE ");
        builder.push_bind(format!("%{}%", q.to_lowercase()));
    }
}

fn apply_upload_sort(
    builder: &mut sqlx::QueryBuilder<'_, sqlx::Sqlite>,
    query: &CreatorContentQuery,
) -> AppResult<()> {
    match query.sort.as_deref().unwrap_or("uploaded") {
        "uploaded" => builder.push(" ORDER BY uploaded_at DESC"),
        "views" => builder.push(" ORDER BY views DESC"),
        "hours" => builder.push(" ORDER BY watch_hours DESC"),
        "title" => builder.push(" ORDER BY title ASC"),
        other => {
            return Err(AppError::BadRequest(format!(
                "unsupported creator content sort: {other}"
            )));
        }
    };
    Ok(())
}

fn upload_from_row(row: sqlx::sqlite::SqliteRow) -> Upload {
    Upload {
        id: row.get("id"),
        slug: row.get("slug"),
        title: row.get("title"),
        description: row.get("description"),
        kind: row.get("kind"),
        duration_sec: row.get("duration_sec"),
        uploaded_at: row.get("uploaded_at"),
        published_at: row.get("published_at"),
        release_at: row.get("release_at"),
        status: row.get("status"),
        visibility: row.get("visibility"),
        access_policy: row.get("access_policy"),
        access_tier_id: row.get("access_tier_id"),
        price_cents: row.get("price_cents"),
        currency: row.get("currency"),
        rental_window_hours: row.get("rental_window_hours"),
        views: row.get("views"),
        likes: row.get("likes"),
        comments: row.get("comments"),
        watch_hours: row.get("watch_hours"),
        thumbnail: row.get("thumbnail"),
        series_title: row.get("series_title"),
        season_number: row.get("season_number"),
        episode_number: row.get("episode_number"),
        size_bytes: row.get("size_bytes"),
        resolution: row.get("resolution"),
        transcode_progress: row.get("transcode_progress"),
    }
}
