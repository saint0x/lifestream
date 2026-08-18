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
