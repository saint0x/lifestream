use super::*;

pub(crate) async fn fetch_upload_jobs(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<UploadJob>> {
    reconcile_stale_media_processing_jobs_for_read(pool, Some(creator_id), None).await?;
    let rows = sqlx::query(
        r#"
        SELECT id, upload_id, series_id, kind, source_type, status, title, intended_visibility,
               bytes_expected, bytes_received, storage_key, created_at, updated_at, published_content_id,
               mime_type, checksum_sha256, completed_at, processing_attempt_count,
               last_processing_error, last_failed_at
        FROM upload_jobs
        WHERE creator_id = ?
        ORDER BY created_at DESC
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(upload_job_from_row).collect())
}

pub(crate) async fn fetch_upload_job_by_id(
    pool: &SqlitePool,
    creator_id: &str,
    id: &str,
) -> AppResult<UploadJob> {
    reconcile_stale_media_processing_jobs_for_read(pool, Some(creator_id), Some(id)).await?;
    fetch_upload_job_by_id_raw(pool, creator_id, id).await
}

pub(crate) async fn fetch_upload_job_by_id_raw(
    pool: &SqlitePool,
    creator_id: &str,
    id: &str,
) -> AppResult<UploadJob> {
    let row = sqlx::query(
        r#"
        SELECT id, upload_id, series_id, kind, source_type, status, title, intended_visibility,
               bytes_expected, bytes_received, storage_key, created_at, updated_at, published_content_id,
               mime_type, checksum_sha256, completed_at, processing_attempt_count,
               last_processing_error, last_failed_at
        FROM upload_jobs
        WHERE creator_id = ? AND id = ?
        "#,
    )
    .bind(creator_id)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(upload_job_from_row(row))
}

pub(crate) async fn fetch_admin_media_jobs(
    pool: &SqlitePool,
    status_filter: Option<&str>,
    creator_filter: Option<&str>,
    limit: i64,
) -> AppResult<Vec<AdminMediaJobRecord>> {
    reconcile_stale_media_processing_jobs_for_read(pool, creator_filter, None).await?;
    let limit = limit.clamp(1, 250);
    let rows = match (status_filter, creator_filter) {
        (Some(status), Some(creator_id)) => {
            sqlx::query(
                "SELECT id, creator_id FROM upload_jobs WHERE status = ? AND creator_id = ? ORDER BY updated_at DESC LIMIT ?",
            )
            .bind(status)
            .bind(creator_id)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        (Some(status), None) => {
            sqlx::query(
                "SELECT id, creator_id FROM upload_jobs WHERE status = ? ORDER BY updated_at DESC LIMIT ?",
            )
            .bind(status)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        (None, Some(creator_id)) => {
            sqlx::query(
                "SELECT id, creator_id FROM upload_jobs WHERE creator_id = ? ORDER BY updated_at DESC LIMIT ?",
            )
            .bind(creator_id)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        (None, None) => sqlx::query(
            "SELECT id, creator_id FROM upload_jobs ORDER BY updated_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?,
    };

    let mut records = Vec::with_capacity(rows.len());
    for row in rows {
        let creator_id: String = row.get("creator_id");
        let job_id: String = row.get("id");
        records.push(fetch_admin_media_job_record(pool, &creator_id, &job_id).await?);
    }
    Ok(records)
}

pub(crate) async fn fetch_admin_media_job_record(
    pool: &SqlitePool,
    creator_id: &str,
    job_id: &str,
) -> AppResult<AdminMediaJobRecord> {
    reconcile_stale_media_processing_jobs_for_read(pool, Some(creator_id), Some(job_id)).await?;
    let upload_job = fetch_upload_job_by_id(pool, creator_id, job_id).await?;
    let asset = fetch_media_asset_by_upload_job(pool, creator_id, job_id)
        .await
        .ok();
    let processing_runs = if let Some(asset) = asset.as_ref() {
        fetch_media_processing_runs(pool, creator_id, &asset.id).await?
    } else {
        Vec::new()
    };
    let stale_processing = is_upload_job_stale(&upload_job);
    let repair_required = upload_job.status == "failed" || stale_processing;

    Ok(AdminMediaJobRecord {
        creator_id: creator_id.to_string(),
        asset_status: asset.map(|item| item.status),
        upload_job,
        processing_runs,
        stale_processing,
        repair_required,
    })
}

pub(crate) async fn fetch_upload_job_by_id_global(
    pool: &SqlitePool,
    job_id: &str,
) -> AppResult<AdminMediaJobRecord> {
    let creator_id = fetch_upload_job_creator_id(pool, job_id).await?;
    fetch_admin_media_job_record(pool, &creator_id, job_id).await
}

pub(crate) async fn fetch_upload_job_creator_id(
    pool: &SqlitePool,
    job_id: &str,
) -> AppResult<String> {
    let row = sqlx::query("SELECT creator_id FROM upload_jobs WHERE id = ?")
        .bind(job_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(row.get("creator_id"))
}

pub(crate) async fn fetch_upload_ingest_session(
    pool: &SqlitePool,
    creator_id: &str,
    job_id: &str,
) -> AppResult<UploadIngestSession> {
    let row = sqlx::query(
        r#"
        SELECT job_id, relative_path, status, mime_type, bytes_received, created_at, updated_at, completed_at
        FROM upload_job_ingest_sessions
        WHERE creator_id = ? AND job_id = ?
        "#,
    )
    .bind(creator_id)
    .bind(job_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(upload_ingest_session_from_row(row))
}

pub(crate) async fn fetch_upload_ingest_sessions(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<UploadIngestSession>> {
    let rows = sqlx::query(
        r#"
        SELECT job_id, relative_path, status, mime_type, bytes_received, created_at, updated_at, completed_at
        FROM upload_job_ingest_sessions
        WHERE creator_id = ?
        ORDER BY created_at DESC
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(upload_ingest_session_from_row)
        .collect())
}

pub(crate) async fn fetch_pending_media_jobs(
    pool: &SqlitePool,
) -> AppResult<Vec<(String, String)>> {
    let rows = sqlx::query(
        "SELECT creator_id, id FROM upload_jobs WHERE status = 'uploaded' ORDER BY updated_at ASC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| (row.get("creator_id"), row.get("id")))
        .collect())
}

fn upload_job_from_row(row: sqlx::sqlite::SqliteRow) -> UploadJob {
    UploadJob {
        id: row.get("id"),
        upload_id: row.get("upload_id"),
        series_id: row.get("series_id"),
        kind: row.get("kind"),
        source_type: row.get("source_type"),
        status: row.get("status"),
        title: row.get("title"),
        intended_visibility: row.get("intended_visibility"),
        bytes_expected: row.get("bytes_expected"),
        bytes_received: row.get("bytes_received"),
        storage_key: row.get("storage_key"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        published_content_id: row.get("published_content_id"),
        mime_type: row.get("mime_type"),
        checksum_sha256: row.get("checksum_sha256"),
        completed_at: row.get("completed_at"),
        processing_attempt_count: row.get("processing_attempt_count"),
        last_processing_error: row.get("last_processing_error"),
        last_failed_at: row.get("last_failed_at"),
    }
}

fn upload_ingest_session_from_row(row: sqlx::sqlite::SqliteRow) -> UploadIngestSession {
    UploadIngestSession {
        job_id: row.get("job_id"),
        relative_path: row.get("relative_path"),
        status: row.get("status"),
        mime_type: row.get("mime_type"),
        bytes_received: row.get("bytes_received"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        completed_at: row.get("completed_at"),
    }
}
