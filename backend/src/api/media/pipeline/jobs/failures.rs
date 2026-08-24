use super::*;

pub(crate) const MAX_MEDIA_PROCESSING_ATTEMPTS: i64 = 3;

pub(crate) async fn media_processing_lease_is_active(
    pool: &SqlitePool,
    creator_id: &str,
    job_id: &str,
    lease_updated_at: &str,
) -> AppResult<bool> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS count
        FROM upload_jobs
        WHERE id = ?
          AND creator_id = ?
          AND status = 'processing'
          AND updated_at = ?
        "#,
    )
    .bind(job_id)
    .bind(creator_id)
    .bind(lease_updated_at)
    .fetch_one(pool)
    .await?;
    Ok(row.get::<i64, _>("count") > 0)
}

pub(crate) async fn media_processing_lease_is_active_for_database(
    database: &crate::db::Database,
    creator_id: &str,
    job_id: &str,
    lease_updated_at: &str,
) -> AppResult<bool> {
    if let Ok(pool) = database.try_postgres_adapter() {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*)::BIGINT AS count
            FROM upload_jobs
            WHERE id = $1
              AND creator_id = $2
              AND status = 'processing'
              AND updated_at = $3
            "#,
        )
        .bind(job_id)
        .bind(creator_id)
        .bind(lease_updated_at)
        .fetch_one(pool)
        .await?;
        return Ok(row.get::<i64, _>("count") > 0);
    }
    media_processing_lease_is_active(
        database.try_sqlite_adapter()?,
        creator_id,
        job_id,
        lease_updated_at,
    )
    .await
}

pub(super) async fn fail_media_job(
    pool: &SqlitePool,
    creator_id: &str,
    job_id: &str,
    message: &str,
    retryable: bool,
) -> AppResult<bool> {
    fail_media_job_for_lease(pool, creator_id, job_id, message, retryable, None).await
}

pub(crate) async fn fail_media_job_for_lease(
    pool: &SqlitePool,
    creator_id: &str,
    job_id: &str,
    message: &str,
    retryable: bool,
    lease_updated_at: Option<&str>,
) -> AppResult<bool> {
    let now = Utc::now().to_rfc3339();
    let job = super::queries::fetch_upload_job_by_id_raw(pool, creator_id, job_id).await?;
    if let Some(lease_updated_at) = lease_updated_at {
        if job.status != "processing" || job.updated_at != lease_updated_at {
            return Ok(false);
        }
    }
    let should_retry = retryable && job.processing_attempt_count < MAX_MEDIA_PROCESSING_ATTEMPTS;
    let next_status = if should_retry { "uploaded" } else { "failed" };
    let mut upload_job_query = String::from(
        "UPDATE upload_jobs SET status = ?, last_processing_error = ?, last_failed_at = ?, updated_at = ? WHERE id = ? AND creator_id = ?",
    );
    if lease_updated_at.is_some() {
        upload_job_query.push_str(" AND status = 'processing' AND updated_at = ?");
    }
    let mut upload_job_stmt = sqlx::query(&upload_job_query)
        .bind(next_status)
        .bind(message)
        .bind(&now)
        .bind(&now)
        .bind(job_id)
        .bind(creator_id);
    if let Some(lease_updated_at) = lease_updated_at {
        upload_job_stmt = upload_job_stmt.bind(lease_updated_at);
    }
    let upload_job_result = upload_job_stmt.execute(pool).await?;
    if upload_job_result.rows_affected() == 0 {
        return Ok(false);
    }

    let mut media_asset_query = String::from(
        "UPDATE media_assets SET status = ?, updated_at = ? WHERE upload_job_id = ? AND creator_id = ?",
    );
    if lease_updated_at.is_some() {
        media_asset_query.push_str(" AND status = 'processing' AND updated_at = ?");
    }
    let mut media_asset_stmt = sqlx::query(&media_asset_query)
        .bind(next_status)
        .bind(&now)
        .bind(job_id)
        .bind(creator_id);
    if let Some(lease_updated_at) = lease_updated_at {
        media_asset_stmt = media_asset_stmt.bind(lease_updated_at);
    }
    media_asset_stmt.execute(pool).await?;

    let details = json!({
        "error": message,
        "retryable": retryable,
        "requeued": should_retry,
        "attempt": job.processing_attempt_count
    });
    if let Some(row) =
        sqlx::query("SELECT id FROM media_assets WHERE upload_job_id = ? AND creator_id = ?")
            .bind(job_id)
            .bind(creator_id)
            .fetch_optional(pool)
            .await?
    {
        let asset_id: String = row.get("id");
        let run_id = start_media_processing_run(
            pool,
            creator_id,
            job_id,
            &asset_id,
            "job_failure",
            details.clone(),
        )
        .await?;
        finish_media_processing_run(
            pool,
            &run_id,
            if should_retry { "retrying" } else { "failed" },
            details,
        )
        .await?;
    }
    Ok(true)
}

pub(crate) async fn fail_media_job_for_lease_in_database(
    database: &crate::db::Database,
    creator_id: &str,
    job_id: &str,
    message: &str,
    retryable: bool,
    lease_updated_at: Option<&str>,
) -> AppResult<bool> {
    if let Ok(pool) = database.try_postgres_adapter() {
        return fail_postgres_media_job_for_lease(
            pool,
            creator_id,
            job_id,
            message,
            retryable,
            lease_updated_at,
        )
        .await;
    }
    fail_media_job_for_lease(
        database.try_sqlite_adapter()?,
        creator_id,
        job_id,
        message,
        retryable,
        lease_updated_at,
    )
    .await
}

async fn fail_postgres_media_job_for_lease(
    pool: &sqlx::PgPool,
    creator_id: &str,
    job_id: &str,
    message: &str,
    retryable: bool,
    lease_updated_at: Option<&str>,
) -> AppResult<bool> {
    let now = Utc::now().to_rfc3339();
    let job = fetch_postgres_upload_job_by_id_raw(pool, creator_id, job_id).await?;
    if let Some(lease_updated_at) = lease_updated_at {
        if job.status != "processing" || job.updated_at != lease_updated_at {
            return Ok(false);
        }
    }
    let should_retry = retryable && job.processing_attempt_count < MAX_MEDIA_PROCESSING_ATTEMPTS;
    let next_status = if should_retry { "uploaded" } else { "failed" };
    let mut upload_job_query = String::from(
        "UPDATE upload_jobs SET status = $1, last_processing_error = $2, last_failed_at = $3, updated_at = $4 WHERE id = $5 AND creator_id = $6",
    );
    if lease_updated_at.is_some() {
        upload_job_query.push_str(" AND status = 'processing' AND updated_at = $7");
    }
    let mut upload_job_stmt = sqlx::query(&upload_job_query)
        .bind(next_status)
        .bind(message)
        .bind(&now)
        .bind(&now)
        .bind(job_id)
        .bind(creator_id);
    if let Some(lease_updated_at) = lease_updated_at {
        upload_job_stmt = upload_job_stmt.bind(lease_updated_at);
    }
    let upload_job_result = upload_job_stmt.execute(pool).await?;
    if upload_job_result.rows_affected() == 0 {
        return Ok(false);
    }

    let mut media_asset_query = String::from(
        "UPDATE media_assets SET status = $1, updated_at = $2 WHERE upload_job_id = $3 AND creator_id = $4",
    );
    if lease_updated_at.is_some() {
        media_asset_query.push_str(" AND status = 'processing' AND updated_at = $5");
    }
    let mut media_asset_stmt = sqlx::query(&media_asset_query)
        .bind(next_status)
        .bind(&now)
        .bind(job_id)
        .bind(creator_id);
    if let Some(lease_updated_at) = lease_updated_at {
        media_asset_stmt = media_asset_stmt.bind(lease_updated_at);
    }
    media_asset_stmt.execute(pool).await?;

    let details = json!({
        "error": message,
        "retryable": retryable,
        "requeued": should_retry,
        "attempt": job.processing_attempt_count
    });
    if let Some(row) =
        sqlx::query("SELECT id FROM media_assets WHERE upload_job_id = $1 AND creator_id = $2")
            .bind(job_id)
            .bind(creator_id)
            .fetch_optional(pool)
            .await?
    {
        let asset_id: String = row.get("id");
        let run_id = start_postgres_failure_processing_run(
            pool,
            creator_id,
            job_id,
            &asset_id,
            details.clone(),
        )
        .await?;
        finish_postgres_failure_processing_run(
            pool,
            &run_id,
            if should_retry { "retrying" } else { "failed" },
            details,
        )
        .await?;
    }
    Ok(true)
}

async fn start_postgres_failure_processing_run(
    pool: &sqlx::PgPool,
    creator_id: &str,
    job_id: &str,
    asset_id: &str,
    details: serde_json::Value,
) -> AppResult<String> {
    let run_id = format!("mpr-{}", Uuid::new_v4().simple());
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO media_processing_runs (
            id, creator_id, upload_job_id, asset_id, stage, status, details_json, started_at, completed_at
        ) VALUES ($1, $2, $3, $4, 'job_failure', 'running', $5, $6, NULL)
        "#,
    )
    .bind(&run_id)
    .bind(creator_id)
    .bind(job_id)
    .bind(asset_id)
    .bind(details.to_string())
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(run_id)
}

async fn finish_postgres_failure_processing_run(
    pool: &sqlx::PgPool,
    run_id: &str,
    status: &str,
    details: serde_json::Value,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE media_processing_runs SET status = $1, details_json = $2, completed_at = $3 WHERE id = $4",
    )
    .bind(status)
    .bind(details.to_string())
    .bind(&now)
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
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

pub(crate) async fn requeue_media_job_for_processing(
    pool: &SqlitePool,
    creator_id: &str,
    job_id: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE upload_jobs SET status = 'uploaded', last_processing_error = NULL, last_failed_at = NULL, updated_at = ? WHERE id = ? AND creator_id = ?",
    )
    .bind(&now)
    .bind(job_id)
    .bind(creator_id)
    .execute(pool)
    .await?;
    sqlx::query(
        "UPDATE media_assets SET status = 'uploaded', updated_at = ? WHERE upload_job_id = ? AND creator_id = ?",
    )
    .bind(&now)
    .bind(job_id)
    .bind(creator_id)
    .execute(pool)
    .await?;
    Ok(())
}
