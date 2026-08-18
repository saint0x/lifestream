use super::*;

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
    let rows = sqlx::query(
        "SELECT id, slug, title, description, kind, duration_sec, uploaded_at, published_at, release_at, status, visibility, access_policy, access_tier_id, price_cents, currency, rental_window_hours, views, likes, comments, watch_hours, thumbnail, series_title, season_number, episode_number, size_bytes, resolution, transcode_progress FROM uploads WHERE creator_id = ? ORDER BY uploaded_at DESC",
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| Upload {
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
        })
        .collect())
}

pub(crate) async fn fetch_upload_by_id(
    pool: &SqlitePool,
    creator_id: &str,
    id: &str,
) -> AppResult<Upload> {
    publish_due_scheduled_upload_releases(pool, Some(creator_id), Some(id)).await?;
    let row = sqlx::query(
        "SELECT id, slug, title, description, kind, duration_sec, uploaded_at, published_at, release_at, status, visibility, access_policy, access_tier_id, price_cents, currency, rental_window_hours, views, likes, comments, watch_hours, thumbnail, series_title, season_number, episode_number, size_bytes, resolution, transcode_progress FROM uploads WHERE creator_id = ? AND id = ?",
    )
    .bind(creator_id)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Upload {
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
    })
}
