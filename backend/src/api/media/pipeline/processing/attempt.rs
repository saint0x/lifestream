use super::*;

pub(crate) async fn begin_media_processing_attempt(
    state: &SharedState,
    creator_id: &str,
    job_id: &str,
) -> AppResult<Option<MediaProcessingAttempt>> {
    if let Ok(pool) = state.db.try_postgres_adapter() {
        return begin_postgres_media_processing_attempt(state, pool, creator_id, job_id).await;
    }
    let job = fetch_upload_job_by_id(state.db.sqlite_adapter(), creator_id, job_id).await?;
    if job.status != "uploaded" {
        return Ok(None);
    }

    let now = Utc::now().to_rfc3339();
    let claimed = sqlx::query(
        "UPDATE upload_jobs SET status = 'processing', updated_at = ?, processing_attempt_count = processing_attempt_count + 1 WHERE id = ? AND creator_id = ? AND status = 'uploaded'",
    )
    .bind(&now)
    .bind(job_id)
    .bind(creator_id)
    .execute(state.db.sqlite_adapter())
    .await?;
    if claimed.rows_affected() == 0 {
        return Ok(None);
    }

    let job = fetch_upload_job_by_id(state.db.sqlite_adapter(), creator_id, job_id).await?;
    let session =
        fetch_upload_ingest_session(state.db.sqlite_adapter(), creator_id, job_id).await?;
    let asset = ensure_media_asset_shell(
        state.db.sqlite_adapter(),
        creator_id,
        &job,
        &session.relative_path,
    )
    .await?;
    let source_path = media_path_for_relative(state, &session.relative_path);
    state
        .storage
        .restore_file_if_missing(&session.relative_path, &source_path)
        .await?;

    sqlx::query(
        "UPDATE media_assets SET status = 'processing', updated_at = ? WHERE upload_job_id = ? AND creator_id = ?",
    )
    .bind(&now)
    .bind(job_id)
    .bind(creator_id)
    .execute(state.db.sqlite_adapter())
    .await?;

    Ok(Some(MediaProcessingAttempt {
        job,
        session,
        asset,
        source_path,
        lease_updated_at: now,
    }))
}

async fn begin_postgres_media_processing_attempt(
    state: &SharedState,
    pool: &sqlx::PgPool,
    creator_id: &str,
    job_id: &str,
) -> AppResult<Option<MediaProcessingAttempt>> {
    let job = fetch_postgres_upload_job_by_id_raw(pool, creator_id, job_id).await?;
    if job.status != "uploaded" {
        return Ok(None);
    }

    let now = Utc::now().to_rfc3339();
    let claimed = sqlx::query(
        "UPDATE upload_jobs SET status = 'processing', updated_at = $1, processing_attempt_count = processing_attempt_count + 1 WHERE id = $2 AND creator_id = $3 AND status = 'uploaded'",
    )
    .bind(&now)
    .bind(job_id)
    .bind(creator_id)
    .execute(pool)
    .await?;
    if claimed.rows_affected() == 0 {
        return Ok(None);
    }

    let job = fetch_postgres_upload_job_by_id_raw(pool, creator_id, job_id).await?;
    let session = fetch_postgres_upload_ingest_session(pool, creator_id, job_id).await?;
    let asset =
        ensure_media_asset_shell_for_database(&state.db, creator_id, &job, &session.relative_path)
            .await?;
    let source_path = media_path_for_relative(state, &session.relative_path);
    state
        .storage
        .restore_file_if_missing(&session.relative_path, &source_path)
        .await?;

    sqlx::query(
        "UPDATE media_assets SET status = 'processing', updated_at = $1 WHERE upload_job_id = $2 AND creator_id = $3",
    )
    .bind(&now)
    .bind(job_id)
    .bind(creator_id)
    .execute(pool)
    .await?;

    Ok(Some(MediaProcessingAttempt {
        job,
        session,
        asset,
        source_path,
        lease_updated_at: now,
    }))
}

async fn fetch_postgres_upload_job_by_id_raw(
    pool: &sqlx::PgPool,
    creator_id: &str,
    id: &str,
) -> AppResult<UploadJob> {
    let row = sqlx::query(
        r#"
        SELECT id, upload_id, series_id, kind, source_type, status, title, intended_visibility,
               bytes_expected::BIGINT AS bytes_expected,
               bytes_received::BIGINT AS bytes_received,
               storage_key, created_at, updated_at, published_content_id,
               mime_type, checksum_sha256, completed_at,
               processing_attempt_count::BIGINT AS processing_attempt_count,
               last_processing_error, last_failed_at
        FROM upload_jobs
        WHERE creator_id = $1 AND id = $2
        "#,
    )
    .bind(creator_id)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(UploadJob {
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
    })
}

async fn fetch_postgres_upload_ingest_session(
    pool: &sqlx::PgPool,
    creator_id: &str,
    job_id: &str,
) -> AppResult<UploadIngestSession> {
    let row = sqlx::query(
        r#"
        SELECT job_id, relative_path, status, mime_type,
               bytes_received::BIGINT AS bytes_received, created_at, updated_at, completed_at
        FROM upload_job_ingest_sessions
        WHERE creator_id = $1 AND job_id = $2
        "#,
    )
    .bind(creator_id)
    .bind(job_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(UploadIngestSession {
        job_id: row.get("job_id"),
        relative_path: row.get("relative_path"),
        status: row.get("status"),
        mime_type: row.get("mime_type"),
        bytes_received: row.get("bytes_received"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        completed_at: row.get("completed_at"),
    })
}
