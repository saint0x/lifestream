use super::*;

pub(crate) const MAX_MEDIA_PROCESSING_ATTEMPTS: i64 = 3;

pub(crate) async fn reconcile_stale_media_processing_jobs(state: SharedState) -> AppResult<()> {
    let cutoff = stale_media_processing_cutoff();
    let rows = sqlx::query(
        r#"
        SELECT id, creator_id
        FROM upload_jobs
        WHERE status = 'processing'
          AND updated_at < ?
        ORDER BY updated_at ASC
        LIMIT 25
        "#,
    )
    .bind(&cutoff)
    .fetch_all(&state.pool)
    .await?;

    for row in rows {
        let job_id: String = row.get("id");
        let creator_id: String = row.get("creator_id");
        state.media_processing_jobs.release(&job_id).await;
        let _ = fail_media_job(
            &state.pool,
            &creator_id,
            &job_id,
            "media processing watchdog timed out and requeued the job",
            true,
        )
        .await?;
    }

    Ok(())
}

pub(crate) async fn reconcile_stale_media_processing_jobs_for_read(
    pool: &SqlitePool,
    creator_filter: Option<&str>,
    job_filter: Option<&str>,
) -> AppResult<()> {
    let cutoff = stale_media_processing_cutoff();
    let rows = sqlx::query(
        r#"
        SELECT id, creator_id, updated_at
        FROM upload_jobs
        WHERE status = 'processing'
          AND updated_at < ?
          AND (? IS NULL OR creator_id = ?)
          AND (? IS NULL OR id = ?)
        ORDER BY updated_at ASC
        LIMIT 100
        "#,
    )
    .bind(&cutoff)
    .bind(creator_filter)
    .bind(creator_filter)
    .bind(job_filter)
    .bind(job_filter)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let job_id: String = row.get("id");
        let creator_id: String = row.get("creator_id");
        let lease_updated_at: String = row.get("updated_at");
        let _ = fail_media_job_for_lease(
            pool,
            &creator_id,
            &job_id,
            "media processing watchdog timed out and requeued the job",
            true,
            Some(&lease_updated_at),
        )
        .await?;
    }

    Ok(())
}

pub(crate) async fn reconcile_single_media_job(
    state: SharedState,
    job_id: &str,
) -> AppResult<MediaJobReconciliationReport> {
    let creator_id = fetch_upload_job_creator_id(&state.pool, job_id).await?;
    let before = fetch_upload_job_by_id_raw(&state.pool, &creator_id, job_id).await?;
    let now = Utc::now().to_rfc3339();
    let mut actions = Vec::new();

    if before.status == "processing" && is_upload_job_stale(&before) {
        state.media_processing_jobs.release(job_id).await;
        let transitioned = fail_media_job_for_lease(
            &state.pool,
            &creator_id,
            job_id,
            "media processing watchdog timed out and requeued the job",
            true,
            Some(&before.updated_at),
        )
        .await?;
        if transitioned {
            let after = fetch_upload_job_by_id_raw(&state.pool, &creator_id, job_id).await?;
            actions.push(MediaJobReconciliationAction {
                action_type: "job_reconciled".to_string(),
                target_id: job_id.to_string(),
                previous_status: Some(before.status.clone()),
                next_status: Some(after.status.clone()),
                reason: "media processing watchdog timed out and reconciled the stale job"
                    .to_string(),
                occurred_at: now.clone(),
            });
        }
    }

    let record = fetch_admin_media_job_record(&state.pool, &creator_id, job_id).await?;
    Ok(MediaJobReconciliationReport {
        job_id: job_id.to_string(),
        reconciled_at: now,
        actions,
        record,
    })
}

pub(crate) async fn publish_due_scheduled_upload_releases(
    pool: &SqlitePool,
    creator_filter: Option<&str>,
    upload_filter: Option<&str>,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"
        SELECT id, creator_id, title, visibility, release_at
        FROM uploads
        WHERE status = 'scheduled'
          AND visibility IN ('public', 'unlisted')
          AND release_at IS NOT NULL
          AND release_at <= ?
          AND (? IS NULL OR creator_id = ?)
          AND (? IS NULL OR id = ?)
        "#,
    )
    .bind(&now)
    .bind(creator_filter)
    .bind(creator_filter)
    .bind(upload_filter)
    .bind(upload_filter)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let upload_id: String = row.get("id");
        let creator_id: String = row.get("creator_id");
        let title: String = row.get("title");
        let visibility: String = row.get("visibility");
        let updated = sqlx::query(
            "UPDATE uploads SET status = 'published', published_at = COALESCE(published_at, ?) WHERE id = ? AND status = 'scheduled'",
        )
        .bind(&now)
        .bind(&upload_id)
        .execute(pool)
        .await?;
        if updated.rows_affected() == 0 {
            continue;
        }
        sqlx::query(
            "UPDATE media_assets SET status = 'published', visibility = ?, updated_at = ? WHERE upload_id = ? AND creator_id = ?",
        )
        .bind(&visibility)
        .bind(&now)
        .bind(&upload_id)
        .bind(&creator_id)
        .execute(pool)
        .await?;
        let _ = enqueue_notification_event(
            pool,
            "scheduled_release_published",
            &format!("{title} is now live."),
            None,
            Some("scheduler"),
            Some(&creator_id),
            None,
            None,
            json!({
                "uploadId": upload_id,
                "publishedAt": now,
            }),
            &[],
            std::slice::from_ref(&creator_id),
        )
        .await;
    }

    Ok(())
}

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

    Ok(rows
        .into_iter()
        .map(|row| UploadJob {
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
        .collect())
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
        .map(|row| UploadIngestSession {
            job_id: row.get("job_id"),
            relative_path: row.get("relative_path"),
            status: row.get("status"),
            mime_type: row.get("mime_type"),
            bytes_received: row.get("bytes_received"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            completed_at: row.get("completed_at"),
        })
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

pub(crate) async fn schedule_media_processing(
    state: SharedState,
    creator_id: String,
    job_id: String,
) {
    if !state.media_processing_jobs.try_acquire(&job_id).await {
        return;
    }

    tokio::spawn(async move {
        let result = process_media_job(state.clone(), &creator_id, &job_id).await;
        if let Err((error, lease_updated_at)) = result {
            let (message, retryable) = classify_media_processing_error(&error);
            let _ = fail_media_job_for_lease(
                &state.pool,
                &creator_id,
                &job_id,
                &message,
                retryable,
                Some(&lease_updated_at),
            )
            .await;
        }
        state.media_processing_jobs.release(&job_id).await;
    });
}

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

async fn fail_media_job(
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
    let job = fetch_upload_job_by_id_raw(pool, creator_id, job_id).await?;
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
