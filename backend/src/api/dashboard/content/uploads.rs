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

pub(crate) fn filter_creator_uploads(
    mut uploads: Vec<Upload>,
    query: &CreatorContentQuery,
) -> AppResult<Vec<Upload>> {
    if let Some(kind) = query.kind.as_deref() {
        if kind != "all" {
            uploads.retain(|upload| upload.kind == kind);
        }
    }
    if let Some(status) = query.status.as_deref() {
        if status != "all" {
            uploads.retain(|upload| upload.status == status);
        }
    }
    if let Some(q) = query.q.as_deref() {
        let normalized = q.trim().to_lowercase();
        if !normalized.is_empty() {
            uploads.retain(|upload| upload.title.to_lowercase().contains(&normalized));
        }
    }

    match query.sort.as_deref().unwrap_or("uploaded") {
        "uploaded" => uploads.sort_by(|left, right| right.uploaded_at.cmp(&left.uploaded_at)),
        "views" => uploads.sort_by(|left, right| right.views.cmp(&left.views)),
        "hours" => uploads.sort_by(|left, right| right.watch_hours.cmp(&left.watch_hours)),
        "title" => uploads.sort_by(|left, right| left.title.cmp(&right.title)),
        other => {
            return Err(AppError::BadRequest(format!(
                "unsupported creator content sort: {other}"
            )));
        }
    }

    Ok(uploads)
}

pub(crate) async fn fetch_uploads(pool: &SqlitePool, creator_id: &str) -> AppResult<Vec<Upload>> {
    publish_due_scheduled_upload_releases(pool, Some(creator_id), None).await?;
    fetch_uploads_unreconciled(pool, creator_id).await
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
