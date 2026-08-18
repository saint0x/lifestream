use super::*;

pub(super) const MAX_MEDIA_PROCESSING_ATTEMPTS: i64 = 3;

pub(super) async fn reconcile_stale_media_processing_jobs(state: SharedState) -> AppResult<()> {
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

pub(super) async fn reconcile_stale_media_processing_jobs_for_read(
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

pub(super) async fn reconcile_single_media_job(
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

pub(super) async fn publish_due_scheduled_upload_releases(
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

pub(super) async fn fetch_upload_jobs(
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

pub(super) async fn fetch_upload_job_by_id(
    pool: &SqlitePool,
    creator_id: &str,
    id: &str,
) -> AppResult<UploadJob> {
    reconcile_stale_media_processing_jobs_for_read(pool, Some(creator_id), Some(id)).await?;
    fetch_upload_job_by_id_raw(pool, creator_id, id).await
}

pub(super) async fn fetch_upload_job_by_id_raw(
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

pub(super) async fn fetch_admin_media_jobs(
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

pub(super) async fn fetch_admin_media_job_record(
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

pub(super) async fn fetch_upload_job_by_id_global(
    pool: &SqlitePool,
    job_id: &str,
) -> AppResult<AdminMediaJobRecord> {
    let creator_id = fetch_upload_job_creator_id(pool, job_id).await?;
    fetch_admin_media_job_record(pool, &creator_id, job_id).await
}

pub(super) async fn fetch_upload_job_creator_id(
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

pub(super) async fn fetch_upload_ingest_session(
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

pub(super) async fn fetch_upload_ingest_sessions(
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

#[derive(Clone, Debug)]
pub(super) struct ProbedMedia {
    pub(super) container_format: Option<String>,
    pub(super) duration_sec: f64,
    pub(super) width: Option<i64>,
    pub(super) height: Option<i64>,
    pub(super) frame_rate: Option<f64>,
    pub(super) video_codec: Option<String>,
    pub(super) audio_codec: Option<String>,
    pub(super) audio_sample_rate_hz: Option<i64>,
    pub(super) audio_channels: Option<i64>,
    pub(super) has_video: bool,
    pub(super) has_audio: bool,
    pub(super) bitrate_bps: Option<i64>,
    pub(super) audio_streams: Vec<ProbedAudioStream>,
    pub(super) subtitle_streams: Vec<ProbedSubtitleStream>,
}

#[derive(Clone, Debug)]
pub(super) struct ProbedAudioStream {
    pub(super) stream_index: i64,
    pub(super) codec: Option<String>,
    pub(super) language: Option<String>,
    pub(super) sample_rate_hz: Option<i64>,
    pub(super) channels: Option<i64>,
}

#[derive(Clone, Debug)]
pub(super) struct ProbedSubtitleStream {
    pub(super) stream_index: i64,
    pub(super) codec: Option<String>,
    pub(super) language: Option<String>,
}

pub(super) async fn fetch_pending_media_jobs(
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

pub(super) async fn schedule_media_processing(
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

struct MediaProcessingAttempt {
    job: UploadJob,
    session: UploadIngestSession,
    asset: MediaAsset,
    source_path: PathBuf,
    lease_updated_at: String,
}

async fn begin_media_processing_attempt(
    state: &SharedState,
    creator_id: &str,
    job_id: &str,
) -> AppResult<Option<MediaProcessingAttempt>> {
    let job = fetch_upload_job_by_id(&state.pool, creator_id, job_id).await?;
    if job.status != "uploaded" && job.status != "processing" {
        return Ok(None);
    }
    let session = fetch_upload_ingest_session(&state.pool, creator_id, job_id).await?;
    let asset =
        ensure_media_asset_shell(&state.pool, creator_id, &job, &session.relative_path).await?;
    let source_path = media_path_for_relative(state, &session.relative_path);

    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE upload_jobs SET status = 'processing', updated_at = ?, processing_attempt_count = processing_attempt_count + 1 WHERE id = ? AND creator_id = ?")
        .bind(&now)
        .bind(job_id)
        .bind(creator_id)
        .execute(&state.pool)
        .await?;
    sqlx::query("UPDATE media_assets SET status = 'processing', updated_at = ? WHERE upload_job_id = ? AND creator_id = ?")
        .bind(&now)
        .bind(job_id)
        .bind(creator_id)
        .execute(&state.pool)
        .await?;

    Ok(Some(MediaProcessingAttempt {
        job,
        session,
        asset,
        source_path,
        lease_updated_at: now,
    }))
}

async fn process_media_job(
    state: SharedState,
    creator_id: &str,
    job_id: &str,
) -> Result<(), (AppError, String)> {
    let Some(attempt) = begin_media_processing_attempt(&state, creator_id, job_id)
        .await
        .map_err(|error| (error, String::new()))?
    else {
        return Ok(());
    };

    let probe_run_id = start_media_processing_run(
        &state.pool,
        creator_id,
        job_id,
        &attempt.asset.id,
        "probe",
        json!({}),
    )
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    let probed = match probe_media(&attempt.source_path).await {
        Ok(probed) => {
            validate_probed_media(&attempt.job, &probed)
                .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
            finish_media_processing_run(
                &state.pool,
                &probe_run_id,
                "completed",
                json!({
                    "durationSec": probed.duration_sec,
                    "width": probed.width,
                    "height": probed.height,
                    "videoCodec": probed.video_codec,
                    "audioCodec": probed.audio_codec,
                    "audioSampleRateHz": probed.audio_sample_rate_hz,
                    "audioChannels": probed.audio_channels,
                    "bitrateBps": probed.bitrate_bps,
                    "attempt": attempt.job.processing_attempt_count + 1
                }),
            )
            .await
            .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
            probed
        }
        Err(error) => {
            let _ = finish_media_processing_run(
                &state.pool,
                &probe_run_id,
                "failed",
                json!({ "error": error.to_string() }),
            )
            .await;
            return Err((error, attempt.lease_updated_at.clone()));
        }
    };
    let integrity_run_id = start_media_processing_run(
        &state.pool,
        creator_id,
        job_id,
        &attempt.asset.id,
        "integrity",
        json!({
            "sourcePath": attempt.session.relative_path,
        }),
    )
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    match verify_media_integrity(&attempt.source_path, &probed).await {
        Ok(()) => {
            finish_media_processing_run(
                &state.pool,
                &integrity_run_id,
                "completed",
                json!({
                    "sourcePath": attempt.session.relative_path,
                    "hasVideo": probed.has_video,
                    "hasAudio": probed.has_audio,
                }),
            )
            .await
            .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
        }
        Err(error) => {
            let _ = finish_media_processing_run(
                &state.pool,
                &integrity_run_id,
                "failed",
                json!({
                    "sourcePath": attempt.session.relative_path,
                    "error": error.to_string(),
                }),
            )
            .await;
            return Err((error, attempt.lease_updated_at.clone()));
        }
    }

    let processed_root = format!("processed/{creator_id}/{job_id}");
    let poster_relative_path = if probed.has_video {
        let poster_relative_path = format!("{processed_root}/poster.jpg");
        let poster_full_path = media_path_for_relative(&state, &poster_relative_path);
        ensure_parent_dir(&poster_full_path)
            .await
            .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
        let poster_run_id = start_media_processing_run(
            &state.pool,
            creator_id,
            job_id,
            &attempt.asset.id,
            "poster",
            json!({ "target": poster_relative_path }),
        )
        .await
        .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
        match generate_poster(&attempt.source_path, &poster_full_path, probed.duration_sec).await {
            Ok(()) => {
                finish_media_processing_run(
                    &state.pool,
                    &poster_run_id,
                    "completed",
                    json!({ "target": poster_relative_path }),
                )
                .await
                .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
            }
            Err(error) => {
                let _ = finish_media_processing_run(
                    &state.pool,
                    &poster_run_id,
                    "failed",
                    json!({ "target": poster_relative_path, "error": error.to_string() }),
                )
                .await;
                return Err((error, attempt.lease_updated_at.clone()));
            }
        }
        Some(poster_relative_path)
    } else {
        None
    };
    let image_derivative_plans = build_image_derivative_plans(&probed)
        .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    let image_derivatives_relative_paths = if probed.has_video {
        let derivatives_run_id = start_media_processing_run(
            &state.pool,
            creator_id,
            job_id,
            &attempt.asset.id,
            "thumbnails",
            json!({
                "targets": image_derivative_plans.iter().map(|plan| {
                    json!({
                        "label": plan.label,
                        "maxWidth": plan.max_width,
                        "maxHeight": plan.max_height,
                    })
                }).collect::<Vec<_>>(),
            }),
        )
        .await
        .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
        let mut derived = Vec::with_capacity(image_derivative_plans.len());
        for plan in &image_derivative_plans {
            let relative_path = format!("{processed_root}/images/{}.jpg", plan.label);
            let full_path = media_path_for_relative(&state, &relative_path);
            ensure_parent_dir(&full_path)
                .await
                .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
            let (width, height) = scaled_dimensions_for_rung(
                probed.width.unwrap_or(plan.max_width),
                probed.height.unwrap_or(plan.max_height),
                plan.max_width,
                plan.max_height,
            );
            if let Err(error) = generate_thumbnail(
                &attempt.source_path,
                &full_path,
                probed.duration_sec,
                width,
                height,
            )
            .await
            {
                let _ = finish_media_processing_run(
                    &state.pool,
                    &derivatives_run_id,
                    "failed",
                    json!({
                        "target": relative_path,
                        "error": error.to_string(),
                    }),
                )
                .await;
                return Err((error, attempt.lease_updated_at.clone()));
            }
            derived.push((plan.label.to_string(), relative_path, width, height));
        }
        finish_media_processing_run(
            &state.pool,
            &derivatives_run_id,
            "completed",
            json!({
                "targets": derived.iter().map(|(label, relative_path, width, height)| {
                    json!({
                        "label": label,
                        "target": relative_path,
                        "width": width,
                        "height": height,
                    })
                }).collect::<Vec<_>>(),
            }),
        )
        .await
        .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
        derived
    } else {
        Vec::new()
    };
    let timeline_preview_track = if probed.has_video {
        let image_relative_path = format!("{processed_root}/images/timeline_preview.jpg");
        let vtt_relative_path = format!("{processed_root}/images/timeline_preview.vtt");
        let image_full_path = media_path_for_relative(&state, &image_relative_path);
        let vtt_full_path = media_path_for_relative(&state, &vtt_relative_path);
        ensure_parent_dir(&image_full_path)
            .await
            .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
        ensure_parent_dir(&vtt_full_path)
            .await
            .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
        let preview_run_id = start_media_processing_run(
            &state.pool,
            creator_id,
            job_id,
            &attempt.asset.id,
            "timeline_preview",
            json!({
                "imageTarget": image_relative_path,
                "vttTarget": vtt_relative_path,
            }),
        )
        .await
        .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
        match generate_timeline_preview_track(
            &attempt.source_path,
            &image_full_path,
            &vtt_full_path,
            &image_relative_path,
            &vtt_relative_path,
            probed.duration_sec,
            probed.width.unwrap_or(320),
            probed.height.unwrap_or(180),
        )
        .await
        {
            Ok(track) => {
                finish_media_processing_run(
                    &state.pool,
                    &preview_run_id,
                    "completed",
                    json!({
                        "label": track.label,
                        "imageTarget": track.image_relative_path,
                        "vttTarget": track.vtt_relative_path,
                        "tileWidth": track.tile_width,
                        "tileHeight": track.tile_height,
                        "columnsCount": track.columns_count,
                        "rowsCount": track.rows_count,
                        "intervalSec": track.interval_sec,
                        "frameCount": track.frame_count,
                    }),
                )
                .await
                .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
                Some(track)
            }
            Err(error) => {
                let _ = finish_media_processing_run(
                    &state.pool,
                    &preview_run_id,
                    "failed",
                    json!({
                        "imageTarget": image_relative_path,
                        "vttTarget": vtt_relative_path,
                        "error": error.to_string(),
                    }),
                )
                .await;
                return Err((error, attempt.lease_updated_at.clone()));
            }
        }
    } else {
        None
    };
    let subtitle_variants = if probed.subtitle_streams.is_empty() {
        Vec::new()
    } else {
        let subtitles_run_id = start_media_processing_run(
            &state.pool,
            creator_id,
            job_id,
            &attempt.asset.id,
            "captions",
            json!({
                "streams": probed.subtitle_streams.iter().map(|stream| {
                    json!({
                        "streamIndex": stream.stream_index,
                        "codec": stream.codec,
                        "language": stream.language,
                    })
                }).collect::<Vec<_>>(),
            }),
        )
        .await
        .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
        let mut generated = Vec::new();
        let mut skipped = Vec::new();
        for (ordinal, stream) in probed.subtitle_streams.iter().enumerate() {
            if !subtitle_codec_supported_for_normalization(stream.codec.as_deref()) {
                skipped.push(json!({
                    "streamIndex": stream.stream_index,
                    "codec": stream.codec,
                    "language": stream.language,
                    "reason": "unsupported_subtitle_codec",
                }));
                continue;
            }
            let language = stream.language.as_deref().unwrap_or("und");
            let label = if ordinal == 0 {
                format!("captions-{language}")
            } else {
                format!("captions-{language}-{}", ordinal + 1)
            };
            let relative_path = format!("{processed_root}/captions/{label}.vtt");
            let full_path = media_path_for_relative(&state, &relative_path);
            ensure_parent_dir(&full_path)
                .await
                .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
            if let Err(error) =
                extract_subtitle_stream_to_webvtt(&attempt.source_path, stream, &full_path).await
            {
                let _ = finish_media_processing_run(
                    &state.pool,
                    &subtitles_run_id,
                    "failed",
                    json!({
                        "streamIndex": stream.stream_index,
                        "codec": stream.codec,
                        "language": stream.language,
                        "target": relative_path,
                        "error": error.to_string(),
                    }),
                )
                .await;
                return Err((error, attempt.lease_updated_at.clone()));
            }
            let metadata = std::fs::metadata(&full_path)
                .map_err(AppError::from)
                .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
            generated.push((
                label,
                relative_path,
                language.to_string(),
                metadata.len() as i64,
                ordinal == 0,
            ));
        }
        finish_media_processing_run(
            &state.pool,
            &subtitles_run_id,
            "completed",
            json!({
                "generated": generated.iter().map(|(label, relative_path, language, file_size_bytes, is_default)| {
                    json!({
                        "label": label,
                        "target": relative_path,
                        "language": language,
                        "fileSizeBytes": file_size_bytes,
                        "default": is_default,
                    })
                }).collect::<Vec<_>>(),
                "skipped": skipped,
            }),
        )
        .await
        .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
        generated
    };

    let hls_subtitle_tracks = subtitle_variants
        .iter()
        .map(
            |(label, relative_path, language, _file_size_bytes, is_default)| {
                GeneratedHlsSubtitleTrack {
                    relative_path: PathBuf::from(relative_path)
                        .file_name()
                        .map(|name| format!("../captions/{}", name.to_string_lossy()))
                        .unwrap_or_else(|| relative_path.clone()),
                    language: language.clone(),
                    name: label.clone(),
                    is_default: *is_default,
                }
            },
        )
        .collect::<Vec<_>>();

    let hls_relative_path = format!("{processed_root}/hls/master.m3u8");
    let hls_full_path = media_path_for_relative(&state, &hls_relative_path);
    ensure_parent_dir(&hls_full_path)
        .await
        .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    let hls_run_id = start_media_processing_run(
        &state.pool,
        creator_id,
        job_id,
        &attempt.asset.id,
        "package",
        json!({ "target": hls_relative_path }),
    )
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    let generated_package = match generate_hls(
        &attempt.source_path,
        &hls_full_path,
        &probed,
        &hls_subtitle_tracks,
    )
    .await
    {
        Ok(package) => {
            finish_media_processing_run(
                &state.pool,
                &hls_run_id,
                "completed",
                json!({
                    "target": hls_relative_path,
                    "masterRelativePath": package.master_relative_path,
                    "variantCount": package.variants.len(),
                    "audioTrackCount": package.audio_tracks.len(),
                    "variants": package.variants.iter().map(|variant| {
                        json!({
                            "label": variant.plan.label.clone(),
                            "width": variant.plan.width,
                            "height": variant.plan.height,
                            "bandwidthBps": variant.plan.bandwidth_bps,
                            "playlistPath": variant.relative_playlist_path.clone(),
                            "fileSizeBytes": variant.file_size_bytes,
                        })
                    }).collect::<Vec<_>>(),
                    "audioTracks": package.audio_tracks.iter().map(|track| {
                        json!({
                            "label": track.label.clone(),
                            "language": track.language.clone(),
                            "codec": track.codec.clone(),
                            "bitrateBps": track.bitrate_bps,
                            "playlistPath": track.relative_playlist_path.clone(),
                            "fileSizeBytes": track.file_size_bytes,
                            "default": track.is_default,
                            "dubbed": track.is_dubbed,
                        })
                    }).collect::<Vec<_>>(),
                }),
            )
            .await
            .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
            package
        }
        Err(error) => {
            let _ = finish_media_processing_run(
                &state.pool,
                &hls_run_id,
                "failed",
                json!({ "target": hls_relative_path, "error": error.to_string() }),
            )
            .await;
            return Err((error, attempt.lease_updated_at.clone()));
        }
    };

    if !media_processing_lease_is_active(&state.pool, creator_id, job_id, &attempt.lease_updated_at)
        .await
        .map_err(|error| (error, attempt.lease_updated_at.clone()))?
    {
        return Ok(());
    }

    replace_media_variants(&state.pool, &attempt.asset.id, &{
        let mut variants = vec![NewMediaVariant {
            variant_type: "source",
            label: "source".to_string(),
            relative_path: attempt.session.relative_path.clone(),
            mime_type: attempt.job.mime_type.clone(),
            width: probed.width,
            height: probed.height,
            bitrate_bps: probed.bitrate_bps,
            file_size_bytes: attempt.job.bytes_expected,
            is_default: false,
        }];
        for (label, relative_path, width, height) in &image_derivatives_relative_paths {
            let full_path = media_path_for_relative(&state, relative_path);
            let metadata = std::fs::metadata(&full_path)
                .map_err(AppError::from)
                .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
            variants.push(NewMediaVariant {
                variant_type: "thumbnail",
                label: label.clone(),
                relative_path: relative_path.clone(),
                mime_type: "image/jpeg".to_string(),
                width: Some(*width),
                height: Some(*height),
                bitrate_bps: None,
                file_size_bytes: metadata.len() as i64,
                is_default: label == "card_thumbnail",
            });
        }
        for (label, relative_path, language, file_size_bytes, is_default) in &subtitle_variants {
            variants.push(NewMediaVariant {
                variant_type: "caption",
                label: format!("{label}:{language}"),
                relative_path: relative_path.clone(),
                mime_type: "text/vtt".to_string(),
                width: None,
                height: None,
                bitrate_bps: None,
                file_size_bytes: *file_size_bytes,
                is_default: *is_default,
            });
        }
        for track in &generated_package.audio_tracks {
            variants.push(NewMediaVariant {
                variant_type: "audio",
                label: format!(
                    "{}:{}:{}:{}:{}",
                    track.label,
                    track.language,
                    "source-provided",
                    if track.is_dubbed { 1 } else { 0 },
                    track.codec
                ),
                relative_path: format!(
                    "{}/{}",
                    PathBuf::from(&hls_relative_path)
                        .parent()
                        .map(|path| path.to_string_lossy().to_string())
                        .unwrap_or_else(|| "processed".to_string()),
                    track.relative_playlist_path
                ),
                mime_type: "application/vnd.apple.mpegurl".to_string(),
                width: None,
                height: None,
                bitrate_bps: Some(track.bitrate_bps),
                file_size_bytes: track.file_size_bytes,
                is_default: track.is_default,
            });
        }
        let highest_height = generated_package
            .variants
            .iter()
            .map(|variant| variant.plan.height)
            .max()
            .unwrap_or_default();
        for variant in &generated_package.variants {
            variants.push(NewMediaVariant {
                variant_type: "playback",
                label: variant.plan.label.clone(),
                relative_path: format!(
                    "{}/{}",
                    PathBuf::from(&hls_relative_path)
                        .parent()
                        .map(|path| path.to_string_lossy().to_string())
                        .unwrap_or_else(|| "processed".to_string()),
                    variant.relative_playlist_path
                ),
                mime_type: "application/vnd.apple.mpegurl".to_string(),
                width: Some(variant.plan.width),
                height: Some(variant.plan.height),
                bitrate_bps: Some(variant.plan.bandwidth_bps),
                file_size_bytes: variant.file_size_bytes,
                is_default: variant.plan.height == highest_height,
            });
        }
        variants
    })
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    replace_media_preview_tracks(
        &state.pool,
        &attempt.asset.id,
        &timeline_preview_track.into_iter().collect::<Vec<_>>(),
    )
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))?;

    let completed_at = Utc::now().to_rfc3339();
    let asset_update = sqlx::query(
        r#"
        UPDATE media_assets
        SET status = 'ready',
            source_relative_path = ?,
            poster_relative_path = ?,
            playback_relative_path = ?,
            mime_type = ?,
            checksum_sha256 = ?,
            container_format = ?,
            file_size_bytes = ?,
            duration_sec = ?,
            width = ?,
            height = ?,
            frame_rate = ?,
            video_codec = ?,
            audio_codec = ?,
            has_video = ?,
            has_audio = ?,
            updated_at = ?,
            processed_at = ?
        WHERE upload_job_id = ? AND creator_id = ?
          AND status = 'processing'
          AND updated_at = ?
        "#,
    )
    .bind(&attempt.session.relative_path)
    .bind(poster_relative_path)
    .bind(&hls_relative_path)
    .bind(&attempt.job.mime_type)
    .bind(attempt.job.checksum_sha256.clone())
    .bind(probed.container_format)
    .bind(attempt.job.bytes_expected)
    .bind(probed.duration_sec)
    .bind(probed.width)
    .bind(probed.height)
    .bind(probed.frame_rate)
    .bind(probed.video_codec)
    .bind(probed.audio_codec)
    .bind(probed.has_video as i64)
    .bind(probed.has_audio as i64)
    .bind(&completed_at)
    .bind(&completed_at)
    .bind(job_id)
    .bind(creator_id)
    .bind(&attempt.lease_updated_at)
    .execute(&state.pool)
    .await
    .map_err(|error| (AppError::from(error), attempt.lease_updated_at.clone()))?;
    if asset_update.rows_affected() == 0 {
        return Ok(());
    }

    let job_update = sqlx::query(
        "UPDATE upload_jobs SET status = 'ready', updated_at = ?, last_processing_error = NULL, last_failed_at = NULL WHERE id = ? AND creator_id = ? AND status = 'processing' AND updated_at = ?",
    )
    .bind(&completed_at)
    .bind(job_id)
    .bind(creator_id)
    .bind(&attempt.lease_updated_at)
    .execute(&state.pool)
    .await
    .map_err(|error| (AppError::from(error), attempt.lease_updated_at.clone()))?;
    if job_update.rows_affected() == 0 {
        return Ok(());
    }

    Ok(())
}

async fn media_processing_lease_is_active(
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

pub(super) async fn fail_media_job_for_lease(
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

pub(super) async fn requeue_media_job_for_processing(
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

pub(super) async fn ensure_media_asset_shell(
    pool: &SqlitePool,
    creator_id: &str,
    job: &UploadJob,
    source_relative_path: &str,
) -> AppResult<MediaAsset> {
    let now = Utc::now().to_rfc3339();
    let asset_id = format!("ast-{}", Uuid::new_v4().simple());
    sqlx::query(
        r#"
        INSERT INTO media_assets (
            id, creator_id, upload_job_id, upload_id, series_id, kind, title, status, visibility,
            source_relative_path, poster_relative_path, playback_relative_path, mime_type,
            checksum_sha256, container_format, file_size_bytes, duration_sec, width, height,
            frame_rate, video_codec, audio_codec, has_video, has_audio, created_at, updated_at,
            processed_at, published_content_id
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(upload_job_id) DO UPDATE SET
            upload_id = excluded.upload_id,
            series_id = excluded.series_id,
            kind = excluded.kind,
            title = excluded.title,
            visibility = excluded.visibility,
            source_relative_path = excluded.source_relative_path,
            mime_type = excluded.mime_type,
            checksum_sha256 = excluded.checksum_sha256,
            file_size_bytes = excluded.file_size_bytes,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(&asset_id)
    .bind(creator_id)
    .bind(&job.id)
    .bind(job.upload_id.clone())
    .bind(job.series_id.clone())
    .bind(&job.kind)
    .bind(&job.title)
    .bind(&job.status)
    .bind(&job.intended_visibility)
    .bind(source_relative_path)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(&job.mime_type)
    .bind(job.checksum_sha256.clone())
    .bind(Option::<String>::None)
    .bind(job.bytes_expected)
    .bind(0.0_f64)
    .bind(Option::<i64>::None)
    .bind(Option::<i64>::None)
    .bind(Option::<f64>::None)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(0_i64)
    .bind(0_i64)
    .bind(&now)
    .bind(&now)
    .bind(Option::<String>::None)
    .bind(job.published_content_id.clone())
    .execute(pool)
    .await?;

    fetch_media_asset_by_upload_job(pool, creator_id, &job.id).await
}

pub(super) async fn fetch_media_assets(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<MediaAsset>> {
    reconcile_stale_media_processing_jobs_for_read(pool, Some(creator_id), None).await?;
    let rows = sqlx::query(
        r#"
        SELECT id, upload_job_id, upload_id, series_id, kind, title, status, visibility,
               source_relative_path, poster_relative_path, playback_relative_path, mime_type,
               checksum_sha256, container_format, file_size_bytes, duration_sec, width, height,
               frame_rate, video_codec, audio_codec, has_video, has_audio, created_at, updated_at,
               processed_at, published_content_id
        FROM media_assets
        WHERE creator_id = ?
        ORDER BY created_at DESC
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    let mut assets = Vec::with_capacity(rows.len());
    for row in rows {
        assets.push(media_asset_from_row(pool, creator_id, row).await?);
    }
    Ok(assets)
}

pub(super) async fn fetch_media_asset_by_upload_job(
    pool: &SqlitePool,
    creator_id: &str,
    job_id: &str,
) -> AppResult<MediaAsset> {
    reconcile_stale_media_processing_jobs_for_read(pool, Some(creator_id), Some(job_id)).await?;
    let row = sqlx::query(
        r#"
        SELECT id, upload_job_id, upload_id, series_id, kind, title, status, visibility,
               source_relative_path, poster_relative_path, playback_relative_path, mime_type,
               checksum_sha256, container_format, file_size_bytes, duration_sec, width, height,
               frame_rate, video_codec, audio_codec, has_video, has_audio, created_at, updated_at,
               processed_at, published_content_id
        FROM media_assets
        WHERE creator_id = ? AND upload_job_id = ?
        "#,
    )
    .bind(creator_id)
    .bind(job_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    media_asset_from_row(pool, creator_id, row).await
}

pub(super) async fn fetch_media_asset_by_upload_id(
    pool: &SqlitePool,
    creator_id: &str,
    upload_id: &str,
) -> AppResult<MediaAsset> {
    reconcile_stale_media_processing_jobs_for_read(pool, Some(creator_id), None).await?;
    let row = sqlx::query(
        r#"
        SELECT id, upload_job_id, upload_id, series_id, kind, title, status, visibility,
               source_relative_path, poster_relative_path, playback_relative_path, mime_type,
               checksum_sha256, container_format, file_size_bytes, duration_sec, width, height,
               frame_rate, video_codec, audio_codec, has_video, has_audio, created_at, updated_at,
               processed_at, published_content_id
        FROM media_assets
        WHERE creator_id = ? AND upload_id = ?
        "#,
    )
    .bind(creator_id)
    .bind(upload_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    media_asset_from_row(pool, creator_id, row).await
}

pub(super) async fn fetch_media_asset_by_id_any_creator(
    pool: &SqlitePool,
    asset_id: &str,
) -> AppResult<MediaAsset> {
    reconcile_stale_media_processing_jobs_for_read(pool, None, None).await?;
    let row = sqlx::query(
        r#"
        SELECT creator_id, id, upload_job_id, upload_id, series_id, kind, title, status, visibility,
               source_relative_path, poster_relative_path, playback_relative_path, mime_type,
               checksum_sha256, container_format, file_size_bytes, duration_sec, width, height,
               frame_rate, video_codec, audio_codec, has_video, has_audio, created_at, updated_at,
               processed_at, published_content_id
        FROM media_assets
        WHERE id = ?
        "#,
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let creator_id: String = row.get("creator_id");

    media_asset_from_row(pool, &creator_id, row).await
}

#[derive(Clone, Debug)]
pub(super) struct StoredMediaPreviewTrack {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) image_relative_path: String,
    pub(super) vtt_relative_path: String,
    pub(super) tile_width: i64,
    pub(super) tile_height: i64,
    pub(super) columns_count: i64,
    pub(super) rows_count: i64,
    pub(super) interval_sec: f64,
    pub(super) frame_count: i64,
    pub(super) is_default: bool,
}

async fn media_asset_from_row(
    pool: &SqlitePool,
    creator_id: &str,
    row: sqlx::sqlite::SqliteRow,
) -> AppResult<MediaAsset> {
    let asset_id: String = row.get("id");
    let source_path: String = row.get("source_relative_path");
    let poster_path: Option<String> = row.get("poster_relative_path");
    let playback_path: Option<String> = row.get("playback_relative_path");
    let status: String = row.get("status");
    let audio_codec: Option<String> = row.get("audio_codec");
    let variants = fetch_media_asset_variants(pool, &asset_id).await?;
    let preview_track_rows = fetch_media_preview_track_rows(pool, &asset_id).await?;
    let audio_tracks = build_media_audio_tracks(
        &status,
        &asset_id,
        &variants,
        audio_codec.as_deref(),
        None,
        None,
        false,
    );
    let caption_tracks = build_media_caption_tracks(&status, &variants, None, None);
    let preview_tracks = build_media_preview_tracks(&status, &preview_track_rows, None);
    let default_audio_track_id = default_audio_track_id(&audio_tracks);
    let default_caption_track_id = default_caption_track_id(&caption_tracks);
    let default_preview_track_id = default_preview_track_id(&preview_tracks);
    Ok(MediaAsset {
        id: asset_id.clone(),
        upload_job_id: row.get("upload_job_id"),
        upload_id: row.get("upload_id"),
        series_id: row.get("series_id"),
        kind: row.get("kind"),
        title: row.get("title"),
        status,
        visibility: row.get("visibility"),
        source_url: media_api_url(&source_path),
        source_path,
        poster_url: poster_path.as_ref().map(|path| media_api_url(path)),
        poster_path,
        playback_url: playback_path.as_ref().map(|path| media_api_url(path)),
        playback_path,
        mime_type: row.get("mime_type"),
        checksum_sha256: row.get("checksum_sha256"),
        container_format: row.get("container_format"),
        file_size_bytes: row.get("file_size_bytes"),
        duration_sec: row.get("duration_sec"),
        width: row.get("width"),
        height: row.get("height"),
        frame_rate: row.get("frame_rate"),
        video_codec: row.get("video_codec"),
        audio_codec,
        has_video: row.get::<i64, _>("has_video") == 1,
        has_audio: row.get::<i64, _>("has_audio") == 1,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        processed_at: row.get("processed_at"),
        published_content_id: row.get("published_content_id"),
        variants,
        audio_tracks,
        caption_tracks,
        preview_tracks,
        default_audio_track_id,
        default_caption_track_id,
        default_preview_track_id,
        processing_runs: fetch_media_processing_runs(pool, creator_id, &asset_id).await?,
    })
}

pub(super) async fn fetch_media_asset_variants(
    pool: &SqlitePool,
    asset_id: &str,
) -> AppResult<Vec<MediaAssetVariant>> {
    ensure_media_asset_thumbnail_variant(pool, asset_id).await?;
    let rows = sqlx::query(
        r#"
        SELECT id, variant_type, label, relative_path, mime_type, width, height, bitrate_bps,
               file_size_bytes, is_default, created_at
        FROM media_asset_variants
        WHERE asset_id = ?
        ORDER BY created_at ASC
        "#,
    )
    .bind(asset_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let relative_path: String = row.get("relative_path");
            MediaAssetVariant {
                id: row.get("id"),
                variant_type: row.get("variant_type"),
                label: row.get("label"),
                url: media_api_url(&relative_path),
                relative_path,
                mime_type: row.get("mime_type"),
                width: row.get("width"),
                height: row.get("height"),
                bitrate_bps: row.get("bitrate_bps"),
                file_size_bytes: row.get("file_size_bytes"),
                is_default: row.get::<i64, _>("is_default") == 1,
                created_at: row.get("created_at"),
            }
        })
        .collect())
}

async fn ensure_media_asset_thumbnail_variant(pool: &SqlitePool, asset_id: &str) -> AppResult<()> {
    let has_thumbnail = sqlx::query(
        "SELECT 1 FROM media_asset_variants WHERE asset_id = ? AND variant_type = 'thumbnail' LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await?
    .is_some();
    if has_thumbnail {
        return Ok(());
    }

    let row = sqlx::query(
        r#"
        SELECT poster_relative_path, width, height, created_at
        FROM media_assets
        WHERE id = ?
        "#,
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(());
    };
    let Some(poster_relative_path) = row.get::<Option<String>, _>("poster_relative_path") else {
        return Ok(());
    };

    sqlx::query(
        r#"
        INSERT INTO media_asset_variants (
            id, asset_id, variant_type, label, relative_path, mime_type, width, height,
            bitrate_bps, file_size_bytes, is_default, created_at
        ) VALUES (?, ?, 'thumbnail', 'card_thumbnail', ?, 'image/jpeg', ?, ?, NULL, 0, 1, ?)
        "#,
    )
    .bind(format!("var-{}", Uuid::new_v4().simple()))
    .bind(asset_id)
    .bind(poster_relative_path)
    .bind(row.get::<Option<i64>, _>("width"))
    .bind(row.get::<Option<i64>, _>("height"))
    .bind(row.get::<String, _>("created_at"))
    .execute(pool)
    .await?;

    Ok(())
}

pub(super) async fn fetch_media_processing_runs(
    pool: &SqlitePool,
    creator_id: &str,
    asset_id: &str,
) -> AppResult<Vec<MediaProcessingRun>> {
    let rows = sqlx::query(
        r#"
        SELECT id, stage, status, details_json, started_at, completed_at
        FROM media_processing_runs
        WHERE creator_id = ? AND asset_id = ?
        ORDER BY started_at DESC
        "#,
    )
    .bind(creator_id)
    .bind(asset_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| MediaProcessingRun {
            id: row.get("id"),
            stage: row.get("stage"),
            status: row.get("status"),
            details: serde_json::from_str(&row.get::<String, _>("details_json"))
                .unwrap_or(json!({})),
            started_at: row.get("started_at"),
            completed_at: row.get("completed_at"),
        })
        .collect())
}

pub(super) async fn fetch_media_preview_track_rows(
    pool: &SqlitePool,
    asset_id: &str,
) -> AppResult<Vec<StoredMediaPreviewTrack>> {
    let rows = sqlx::query(
        r#"
        SELECT id, label, image_relative_path, vtt_relative_path, tile_width, tile_height,
               columns_count, rows_count, interval_sec, frame_count, is_default, created_at
        FROM media_timeline_previews
        WHERE asset_id = ?
        ORDER BY created_at ASC
        "#,
    )
    .bind(asset_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| StoredMediaPreviewTrack {
            id: row.get("id"),
            label: row.get("label"),
            image_relative_path: row.get("image_relative_path"),
            vtt_relative_path: row.get("vtt_relative_path"),
            tile_width: row.get("tile_width"),
            tile_height: row.get("tile_height"),
            columns_count: row.get("columns_count"),
            rows_count: row.get("rows_count"),
            interval_sec: row.get("interval_sec"),
            frame_count: row.get("frame_count"),
            is_default: row.get::<i64, _>("is_default") == 1,
        })
        .collect())
}

struct NewMediaVariant {
    variant_type: &'static str,
    label: String,
    relative_path: String,
    mime_type: String,
    width: Option<i64>,
    height: Option<i64>,
    bitrate_bps: Option<i64>,
    file_size_bytes: i64,
    is_default: bool,
}

#[derive(Clone, Debug)]
struct ImageDerivativePlan {
    label: &'static str,
    max_width: i64,
    max_height: i64,
}

#[derive(Clone, Debug)]
struct NewMediaPreviewTrack {
    label: String,
    image_relative_path: String,
    vtt_relative_path: String,
    tile_width: i64,
    tile_height: i64,
    columns_count: i64,
    rows_count: i64,
    interval_sec: f64,
    frame_count: i64,
    is_default: bool,
}

#[derive(Clone, Debug)]
pub(super) struct HlsVariantPlan {
    pub(super) label: String,
    pub(super) width: i64,
    pub(super) height: i64,
    pub(super) video_bitrate_bps: i64,
    pub(super) bandwidth_bps: i64,
}

#[derive(Clone, Debug)]
pub(super) struct GeneratedHlsVariant {
    pub(super) plan: HlsVariantPlan,
    pub(super) relative_playlist_path: String,
    pub(super) file_size_bytes: i64,
}

#[derive(Clone, Debug)]
pub(super) struct GeneratedHlsSubtitleTrack {
    pub(super) relative_path: String,
    pub(super) language: String,
    pub(super) name: String,
    pub(super) is_default: bool,
}

#[derive(Clone, Debug)]
pub(super) struct GeneratedHlsAudioTrack {
    pub(super) label: String,
    pub(super) language: String,
    pub(super) codec: String,
    pub(super) bitrate_bps: i64,
    pub(super) relative_playlist_path: String,
    pub(super) file_size_bytes: i64,
    pub(super) is_default: bool,
    pub(super) is_dubbed: bool,
}

#[derive(Clone, Debug)]
pub(super) struct GeneratedHlsPackage {
    pub(super) master_relative_path: String,
    pub(super) variants: Vec<GeneratedHlsVariant>,
    pub(super) audio_tracks: Vec<GeneratedHlsAudioTrack>,
    pub(super) subtitle_tracks: Vec<GeneratedHlsSubtitleTrack>,
}

async fn replace_media_variants(
    pool: &SqlitePool,
    asset_id: &str,
    variants: &[NewMediaVariant],
) -> AppResult<()> {
    sqlx::query("DELETE FROM media_asset_variants WHERE asset_id = ?")
        .bind(asset_id)
        .execute(pool)
        .await?;

    let now = Utc::now().to_rfc3339();
    for variant in variants {
        sqlx::query(
            r#"
            INSERT INTO media_asset_variants (
                id, asset_id, variant_type, label, relative_path, mime_type, width, height,
                bitrate_bps, file_size_bytes, is_default, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(format!("var-{}", Uuid::new_v4().simple()))
        .bind(asset_id)
        .bind(variant.variant_type)
        .bind(&variant.label)
        .bind(&variant.relative_path)
        .bind(&variant.mime_type)
        .bind(variant.width)
        .bind(variant.height)
        .bind(variant.bitrate_bps)
        .bind(variant.file_size_bytes)
        .bind(variant.is_default as i64)
        .bind(&now)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn replace_media_preview_tracks(
    pool: &SqlitePool,
    asset_id: &str,
    tracks: &[NewMediaPreviewTrack],
) -> AppResult<()> {
    sqlx::query("DELETE FROM media_timeline_previews WHERE asset_id = ?")
        .bind(asset_id)
        .execute(pool)
        .await?;

    let now = Utc::now().to_rfc3339();
    for track in tracks {
        sqlx::query(
            r#"
            INSERT INTO media_timeline_previews (
                id, asset_id, label, image_relative_path, vtt_relative_path, tile_width,
                tile_height, columns_count, rows_count, interval_sec, frame_count, is_default,
                created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(format!("mtp-{}", Uuid::new_v4().simple()))
        .bind(asset_id)
        .bind(&track.label)
        .bind(&track.image_relative_path)
        .bind(&track.vtt_relative_path)
        .bind(track.tile_width)
        .bind(track.tile_height)
        .bind(track.columns_count)
        .bind(track.rows_count)
        .bind(track.interval_sec)
        .bind(track.frame_count)
        .bind(track.is_default as i64)
        .bind(&now)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn start_media_processing_run(
    pool: &SqlitePool,
    creator_id: &str,
    job_id: &str,
    asset_id: &str,
    stage: &str,
    details: Value,
) -> AppResult<String> {
    let id = format!("mpr-{}", Uuid::new_v4().simple());
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO media_processing_runs (
            id, creator_id, upload_job_id, asset_id, stage, status, details_json, started_at, completed_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(creator_id)
    .bind(job_id)
    .bind(asset_id)
    .bind(stage)
    .bind("running")
    .bind(details.to_string())
    .bind(&now)
    .bind(Option::<String>::None)
    .execute(pool)
    .await?;
    Ok(id)
}

async fn finish_media_processing_run(
    pool: &SqlitePool,
    run_id: &str,
    status: &str,
    details: Value,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE media_processing_runs SET status = ?, details_json = ?, completed_at = ? WHERE id = ?",
    )
    .bind(status)
    .bind(details.to_string())
    .bind(&now)
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn probe_media(path: &FsPath) -> AppResult<ProbedMedia> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-print_format")
        .arg("json")
        .arg("-show_format")
        .arg("-show_streams")
        .arg(path)
        .output()
        .await?;

    if !output.status.success() {
        return Err(AppError::MediaPipeline(format!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let payload: Value = serde_json::from_slice(&output.stdout)?;
    let format = payload.get("format").cloned().unwrap_or_else(|| json!({}));
    let streams = payload
        .get("streams")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let video_stream = streams.iter().find(|stream| {
        stream
            .get("codec_type")
            .and_then(Value::as_str)
            .map(|codec_type| codec_type == "video")
            .unwrap_or(false)
    });
    let audio_streams = streams
        .iter()
        .filter(|stream| {
            stream
                .get("codec_type")
                .and_then(Value::as_str)
                .map(|codec_type| codec_type == "audio")
                .unwrap_or(false)
        })
        .filter_map(|stream| {
            let stream_index = stream.get("index").and_then(Value::as_i64)?;
            Some(ProbedAudioStream {
                stream_index,
                codec: stream
                    .get("codec_name")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                language: stream
                    .get("tags")
                    .and_then(|tags| tags.get("language"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                sample_rate_hz: stream
                    .get("sample_rate")
                    .and_then(Value::as_str)
                    .and_then(|value| value.parse::<i64>().ok()),
                channels: stream.get("channels").and_then(Value::as_i64),
            })
        })
        .collect::<Vec<_>>();
    let audio_stream = audio_streams.first();
    let subtitle_streams = streams
        .iter()
        .filter(|stream| {
            stream
                .get("codec_type")
                .and_then(Value::as_str)
                .map(|codec_type| codec_type == "subtitle")
                .unwrap_or(false)
        })
        .filter_map(|stream| {
            let stream_index = stream.get("index").and_then(Value::as_i64)?;
            Some(ProbedSubtitleStream {
                stream_index,
                codec: stream
                    .get("codec_name")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                language: stream
                    .get("tags")
                    .and_then(|tags| tags.get("language"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
            })
        })
        .collect::<Vec<_>>();

    Ok(ProbedMedia {
        container_format: format
            .get("format_name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        duration_sec: format
            .get("duration")
            .and_then(Value::as_str)
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(0.0),
        width: video_stream
            .and_then(|stream| stream.get("width"))
            .and_then(Value::as_i64),
        height: video_stream
            .and_then(|stream| stream.get("height"))
            .and_then(Value::as_i64),
        frame_rate: video_stream
            .and_then(|stream| stream.get("avg_frame_rate"))
            .and_then(Value::as_str)
            .and_then(parse_ffprobe_ratio),
        video_codec: video_stream
            .and_then(|stream| stream.get("codec_name"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        audio_codec: audio_stream.and_then(|stream| stream.codec.clone()),
        audio_sample_rate_hz: audio_stream.and_then(|stream| stream.sample_rate_hz),
        audio_channels: audio_stream.and_then(|stream| stream.channels),
        has_video: video_stream.is_some(),
        has_audio: audio_stream.is_some(),
        bitrate_bps: format
            .get("bit_rate")
            .and_then(Value::as_str)
            .and_then(|value| value.parse::<i64>().ok()),
        audio_streams,
        subtitle_streams,
    })
}

pub(super) fn validate_probed_media(job: &UploadJob, media: &ProbedMedia) -> AppResult<()> {
    let container = media.container_format.as_deref().ok_or_else(|| {
        AppError::BadRequest("media container could not be identified".to_string())
    })?;
    let container_allowed = container.split(',').map(|value| value.trim()).any(|value| {
        matches!(
            value,
            "mov" | "mp4" | "m4a" | "3gp" | "3g2" | "mj2" | "matroska" | "webm" | "mpegts"
        )
    });
    if !container_allowed {
        return Err(AppError::BadRequest(format!(
            "unsupported media container: {container}"
        )));
    }

    if !media.has_video {
        return Err(AppError::BadRequest(
            "upload must contain at least one video stream".to_string(),
        ));
    }
    if !media.has_audio {
        return Err(AppError::BadRequest(
            "upload must contain at least one audio stream".to_string(),
        ));
    }

    let width = media
        .width
        .ok_or_else(|| AppError::BadRequest("video width could not be determined".to_string()))?;
    let height = media
        .height
        .ok_or_else(|| AppError::BadRequest("video height could not be determined".to_string()))?;
    if width < 144 || height < 144 {
        return Err(AppError::BadRequest(
            "video resolution is below the supported minimum of 144p".to_string(),
        ));
    }
    if width > 7680 || height > 4320 {
        return Err(AppError::BadRequest(
            "video resolution exceeds the supported maximum of 8k".to_string(),
        ));
    }

    let frame_rate = media.frame_rate.ok_or_else(|| {
        AppError::BadRequest("video frame rate could not be determined".to_string())
    })?;
    if !(1.0..=120.0).contains(&frame_rate) {
        return Err(AppError::BadRequest(format!(
            "video frame rate {frame_rate:.2}fps is outside the supported range"
        )));
    }

    if media.duration_sec <= 0.0 {
        return Err(AppError::BadRequest(
            "media duration must be greater than zero".to_string(),
        ));
    }

    let max_duration_sec = match job.kind.as_str() {
        "clip" => 15.0 * 60.0,
        "trailer" => 20.0 * 60.0,
        "episode" => 4.0 * 60.0 * 60.0,
        "film" | "video" | "vod" | "live_archive" => 8.0 * 60.0 * 60.0,
        other => {
            return Err(AppError::BadRequest(format!(
                "unsupported upload job kind: {other}"
            )));
        }
    };
    if media.duration_sec > max_duration_sec {
        return Err(AppError::BadRequest(format!(
            "{} uploads cannot exceed {:.0} minutes",
            job.kind,
            max_duration_sec / 60.0
        )));
    }

    let video_codec = media
        .video_codec
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("video codec could not be determined".to_string()))?;
    if !matches!(
        video_codec,
        "h264" | "hevc" | "vp9" | "av1" | "mpeg4" | "prores"
    ) {
        return Err(AppError::BadRequest(format!(
            "unsupported video codec: {video_codec}"
        )));
    }

    let audio_codec = media
        .audio_codec
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("audio codec could not be determined".to_string()))?;
    if !matches!(
        audio_codec,
        "aac" | "mp3" | "opus" | "flac" | "alac" | "ac3" | "eac3" | "pcm_s16le"
    ) {
        return Err(AppError::BadRequest(format!(
            "unsupported audio codec: {audio_codec}"
        )));
    }

    let audio_sample_rate_hz = media.audio_sample_rate_hz.ok_or_else(|| {
        AppError::BadRequest("audio sample rate could not be determined".to_string())
    })?;
    if !(8_000..=192_000).contains(&audio_sample_rate_hz) {
        return Err(AppError::BadRequest(format!(
            "audio sample rate {audio_sample_rate_hz}Hz is outside the supported range"
        )));
    }

    let audio_channels = media.audio_channels.ok_or_else(|| {
        AppError::BadRequest("audio channel count could not be determined".to_string())
    })?;
    if !(1..=8).contains(&audio_channels) {
        return Err(AppError::BadRequest(format!(
            "audio channel count {audio_channels} is outside the supported range"
        )));
    }

    if let Some(bitrate_bps) = media.bitrate_bps {
        if !(32_000..=200_000_000).contains(&bitrate_bps) {
            return Err(AppError::BadRequest(format!(
                "media bitrate {bitrate_bps}bps is outside the supported range"
            )));
        }
    }

    Ok(())
}

fn classify_media_processing_error(error: &AppError) -> (String, bool) {
    match error {
        AppError::BadRequest(message) => (message.clone(), false),
        AppError::Internal(message) => (
            format!("internal media processing failure: {message}"),
            true,
        ),
        AppError::MediaPipeline(message) => (message.clone(), true),
        AppError::Io(error) => (format!("io failure during media processing: {error}"), true),
        AppError::Serialization(error) => (
            format!("invalid media probe payload during processing: {error}"),
            false,
        ),
        AppError::NotFound => (
            "required media processing resource was not found".to_string(),
            false,
        ),
        AppError::Unauthorized => ("unauthorized media processing attempt".to_string(), false),
        AppError::Forbidden => ("forbidden media processing attempt".to_string(), false),
        AppError::PaymentRequired(message) => (message.clone(), false),
        AppError::RateLimited => ("media processing rate limited".to_string(), true),
        AppError::Database(error) => (
            format!("database failure during media processing: {error}"),
            false,
        ),
    }
}

async fn generate_poster(
    input_path: &FsPath,
    output_path: &FsPath,
    duration_sec: f64,
) -> AppResult<()> {
    let capture_offset = if duration_sec >= 5.0 {
        "00:00:05"
    } else {
        "00:00:00"
    };
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-ss")
        .arg(capture_offset)
        .arg("-i")
        .arg(input_path)
        .arg("-frames:v")
        .arg("1")
        .arg("-q:v")
        .arg("2")
        .arg(output_path)
        .output()
        .await?;

    if !output.status.success() {
        return Err(AppError::MediaPipeline(format!(
            "ffmpeg poster generation failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    Ok(())
}

async fn generate_thumbnail(
    input_path: &FsPath,
    output_path: &FsPath,
    duration_sec: f64,
    width: i64,
    height: i64,
) -> AppResult<()> {
    let capture_offset = if duration_sec >= 5.0 {
        "00:00:05"
    } else {
        "00:00:00"
    };
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-ss")
        .arg(capture_offset)
        .arg("-i")
        .arg(input_path)
        .arg("-frames:v")
        .arg("1")
        .arg("-vf")
        .arg(format!("scale={width}:{height}"))
        .arg("-q:v")
        .arg("3")
        .arg(output_path)
        .output()
        .await?;

    if !output.status.success() {
        return Err(AppError::MediaPipeline(format!(
            "ffmpeg thumbnail generation failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    Ok(())
}

fn build_timeline_preview_timestamps(duration_sec: f64) -> Vec<f64> {
    const MAX_PREVIEW_FRAMES: usize = 60;
    const MIN_PREVIEW_INTERVAL_SEC: f64 = 5.0;

    if duration_sec <= 0.0 {
        return vec![0.0];
    }

    let interval_sec = (duration_sec / MAX_PREVIEW_FRAMES as f64).max(MIN_PREVIEW_INTERVAL_SEC);
    let mut timestamps = Vec::new();
    let mut cursor = 0.0_f64;
    while cursor < duration_sec && timestamps.len() < MAX_PREVIEW_FRAMES {
        timestamps.push(cursor);
        cursor += interval_sec;
    }
    if timestamps.is_empty() {
        timestamps.push(0.0);
    }
    timestamps
}

fn format_webvtt_timestamp(timestamp_sec: f64) -> String {
    let total_millis = (timestamp_sec.max(0.0) * 1000.0).round() as i64;
    let hours = total_millis / 3_600_000;
    let minutes = (total_millis % 3_600_000) / 60_000;
    let seconds = (total_millis % 60_000) / 1000;
    let millis = total_millis % 1000;
    format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}")
}

async fn generate_timeline_preview_track(
    input_path: &FsPath,
    image_output_path: &FsPath,
    vtt_output_path: &FsPath,
    image_relative_path: &str,
    vtt_relative_path: &str,
    duration_sec: f64,
    source_width: i64,
    source_height: i64,
) -> AppResult<NewMediaPreviewTrack> {
    let timestamps = build_timeline_preview_timestamps(duration_sec);
    let frame_count = timestamps.len() as i64;
    let columns_count = (timestamps.len().min(10).max(1)) as i64;
    let rows_count = ((timestamps.len() as i64) + columns_count - 1) / columns_count;
    let interval_sec = if timestamps.len() > 1 {
        timestamps[1] - timestamps[0]
    } else {
        duration_sec.max(1.0)
    };
    let (tile_width, tile_height) =
        scaled_dimensions_for_rung(source_width, source_height, 320, 180);

    let fps_denominator = interval_sec.max(0.001);
    let sprite_output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(input_path)
        .arg("-frames:v")
        .arg("1")
        .arg("-vf")
        .arg(format!(
            "fps=1/{fps_denominator:.6},scale={tile_width}:{tile_height}:force_original_aspect_ratio=decrease:force_divisible_by=2,pad={tile_width}:{tile_height}:(ow-iw)/2:(oh-ih)/2,tile={}x{}",
            columns_count, rows_count
        ))
        .arg("-q:v")
        .arg("4")
        .arg(image_output_path)
        .output()
        .await?;

    if !sprite_output.status.success() {
        return Err(AppError::MediaPipeline(format!(
            "ffmpeg timeline preview generation failed: {}",
            String::from_utf8_lossy(&sprite_output.stderr).trim()
        )));
    }

    let image_name = PathBuf::from(image_relative_path)
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .ok_or_else(|| {
            AppError::MediaPipeline("invalid timeline preview image path".to_string())
        })?;
    let mut vtt = String::from("WEBVTT\n\n");
    for (index, start_sec) in timestamps.iter().enumerate() {
        let end_sec = timestamps
            .get(index + 1)
            .copied()
            .unwrap_or(duration_sec.max(*start_sec + 0.001));
        let end_sec = end_sec.max(*start_sec + 0.001);
        let column = (index as i64) % columns_count;
        let row = (index as i64) / columns_count;
        let x = column * tile_width;
        let y = row * tile_height;
        vtt.push_str(&format!(
            "{} --> {}\n{}#xywh={},{},{},{}\n\n",
            format_webvtt_timestamp(*start_sec),
            format_webvtt_timestamp(end_sec),
            image_name,
            x,
            y,
            tile_width,
            tile_height
        ));
    }
    tokio::fs::write(vtt_output_path, vtt).await?;

    Ok(NewMediaPreviewTrack {
        label: "timeline_preview".to_string(),
        image_relative_path: image_relative_path.to_string(),
        vtt_relative_path: vtt_relative_path.to_string(),
        tile_width,
        tile_height,
        columns_count,
        rows_count,
        interval_sec,
        frame_count,
        is_default: true,
    })
}

async fn extract_subtitle_stream_to_webvtt(
    input_path: &FsPath,
    stream: &ProbedSubtitleStream,
    output_path: &FsPath,
) -> AppResult<()> {
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(input_path)
        .arg("-map")
        .arg(format!("0:{}", stream.stream_index))
        .arg("-c:s")
        .arg("webvtt")
        .arg("-f")
        .arg("webvtt")
        .arg(output_path)
        .output()
        .await?;

    if !output.status.success() {
        return Err(AppError::MediaPipeline(format!(
            "ffmpeg subtitle normalization failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    Ok(())
}

fn subtitle_codec_supported_for_normalization(codec: Option<&str>) -> bool {
    matches!(
        codec,
        Some("subrip" | "webvtt" | "mov_text" | "ass" | "ssa")
    )
}

pub(super) async fn verify_media_integrity(
    input_path: &FsPath,
    media: &ProbedMedia,
) -> AppResult<()> {
    let mut command = Command::new("ffmpeg");
    command.arg("-v").arg("error").arg("-i").arg(input_path);
    if media.has_video {
        command.arg("-map").arg("0:v:0");
    }
    if media.has_audio {
        command.arg("-map").arg("0:a:0");
    }
    command
        .arg("-threads")
        .arg("1")
        .arg("-max_muxing_queue_size")
        .arg("1024")
        .arg("-f")
        .arg("null")
        .arg("-");

    let output = command.output().await?;
    if !output.status.success() {
        return Err(AppError::MediaPipeline(format!(
            "ffmpeg integrity verification failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    Ok(())
}

fn build_image_derivative_plans(media: &ProbedMedia) -> AppResult<Vec<ImageDerivativePlan>> {
    let width = media
        .width
        .ok_or_else(|| AppError::BadRequest("video width could not be determined".to_string()))?;
    let height = media
        .height
        .ok_or_else(|| AppError::BadRequest("video height could not be determined".to_string()))?;
    let mut plans = Vec::new();

    for candidate in [
        ImageDerivativePlan {
            label: "card_thumbnail",
            max_width: 640,
            max_height: 360,
        },
        ImageDerivativePlan {
            label: "player_thumbnail",
            max_width: 1280,
            max_height: 720,
        },
    ] {
        let candidate_dimensions =
            scaled_dimensions_for_rung(width, height, candidate.max_width, candidate.max_height);
        if candidate_dimensions.0 < 144 || candidate_dimensions.1 < 144 {
            continue;
        }
        if plans.iter().any(|plan: &ImageDerivativePlan| {
            scaled_dimensions_for_rung(width, height, plan.max_width, plan.max_height)
                == candidate_dimensions
        }) {
            continue;
        }
        plans.push(candidate);
    }

    Ok(plans)
}

async fn generate_hls(
    input_path: &FsPath,
    output_path: &FsPath,
    media: &ProbedMedia,
    subtitle_tracks: &[GeneratedHlsSubtitleTrack],
) -> AppResult<GeneratedHlsPackage> {
    if let Some(parent) = output_path.parent() {
        if tokio::fs::try_exists(parent).await? {
            let _ = tokio::fs::remove_dir_all(parent).await;
        }
        tokio::fs::create_dir_all(parent).await?;
    }

    let output_dir = output_path
        .parent()
        .ok_or_else(|| AppError::MediaPipeline("invalid playback output directory".to_string()))?;
    let plans = plan_hls_variants(media)?;
    let mut variants = Vec::with_capacity(plans.len());
    let mut audio_tracks = Vec::with_capacity(media.audio_streams.len().max(1));

    for (ordinal, stream) in media.audio_streams.iter().enumerate() {
        let language = stream.language.clone().unwrap_or_else(|| "und".to_string());
        let label = if ordinal == 0 {
            format!("audio-{language}")
        } else {
            format!("audio-{language}-{}", ordinal + 1)
        };
        let track_dir = output_dir.join(&label);
        tokio::fs::create_dir_all(&track_dir).await?;
        let playlist_path = track_dir.join("playlist.m3u8");
        let segment_pattern = track_dir.join("segment_%03d.aac");

        let output = Command::new("ffmpeg")
            .arg("-y")
            .arg("-i")
            .arg(input_path)
            .arg("-map")
            .arg(format!("0:{}", stream.stream_index))
            .arg("-vn")
            .arg("-c:a")
            .arg("aac")
            .arg("-b:a")
            .arg("128k")
            .arg("-ac")
            .arg("2")
            .arg("-ar")
            .arg("48000")
            .arg("-f")
            .arg("hls")
            .arg("-hls_time")
            .arg("6")
            .arg("-hls_playlist_type")
            .arg("vod")
            .arg("-hls_segment_filename")
            .arg(&segment_pattern)
            .arg(&playlist_path)
            .output()
            .await?;
        if !output.status.success() {
            return Err(AppError::MediaPipeline(format!(
                "ffmpeg audio hls packaging failed for {}: {}",
                label,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        audio_tracks.push(GeneratedHlsAudioTrack {
            label: label.clone(),
            language: language.clone(),
            codec: "aac".to_string(),
            bitrate_bps: 128_000,
            relative_playlist_path: format!("{label}/playlist.m3u8"),
            file_size_bytes: directory_size(&track_dir).await?,
            is_default: ordinal == 0,
            is_dubbed: ordinal > 0
                && language
                    != media.audio_streams[0]
                        .language
                        .clone()
                        .unwrap_or_else(|| "und".to_string()),
        });
    }

    for plan in &plans {
        let variant_dir = output_dir.join(&plan.label);
        tokio::fs::create_dir_all(&variant_dir).await?;
        let playlist_path = variant_dir.join("playlist.m3u8");
        let segment_pattern = variant_dir.join("segment_%03d.ts");

        let mut command = Command::new("ffmpeg");
        command
            .arg("-y")
            .arg("-i")
            .arg(input_path)
            .arg("-map")
            .arg("0:v:0");

        if media.has_video {
            command
                .arg("-c:v")
                .arg("libx264")
                .arg("-preset")
                .arg("veryfast")
                .arg("-pix_fmt")
                .arg("yuv420p")
                .arg("-g")
                .arg("48")
                .arg("-keyint_min")
                .arg("48")
                .arg("-sc_threshold")
                .arg("0")
                .arg("-vf")
                .arg(format!("scale={}:{}", plan.width, plan.height))
                .arg("-maxrate")
                .arg(format!("{}k", (plan.video_bitrate_bps / 1000).max(300)))
                .arg("-bufsize")
                .arg(format!(
                    "{}k",
                    ((plan.video_bitrate_bps * 2) / 1000).max(600)
                ))
                .arg("-an");
        } else {
            command.arg("-vn");
        }

        command
            .arg("-f")
            .arg("hls")
            .arg("-hls_time")
            .arg("6")
            .arg("-hls_playlist_type")
            .arg("vod")
            .arg("-hls_segment_filename")
            .arg(&segment_pattern)
            .arg(&playlist_path);

        let output = command.output().await?;
        if !output.status.success() {
            return Err(AppError::MediaPipeline(format!(
                "ffmpeg hls packaging failed for {}: {}",
                plan.label,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        variants.push(GeneratedHlsVariant {
            plan: plan.clone(),
            relative_playlist_path: format!("{}/playlist.m3u8", plan.label),
            file_size_bytes: directory_size(&variant_dir).await?,
        });
    }

    write_hls_master_manifest(output_path, &variants, &audio_tracks, subtitle_tracks).await?;
    let package = GeneratedHlsPackage {
        master_relative_path: output_path.to_string_lossy().to_string(),
        variants,
        audio_tracks,
        subtitle_tracks: subtitle_tracks.to_vec(),
    };
    validate_generated_hls_package(output_path, &package).await?;
    Ok(package)
}

pub(super) fn plan_hls_variants(media: &ProbedMedia) -> AppResult<Vec<HlsVariantPlan>> {
    let width = media
        .width
        .ok_or_else(|| AppError::BadRequest("video width could not be determined".to_string()))?;
    let height = media
        .height
        .ok_or_else(|| AppError::BadRequest("video height could not be determined".to_string()))?;
    let ladder = [
        (426_i64, 240_i64, 700_000_i64, 96_000_i64),
        (640, 360, 1_200_000, 96_000),
        (854, 480, 2_200_000, 128_000),
        (1280, 720, 4_500_000, 128_000),
        (1920, 1080, 8_000_000, 192_000),
    ];
    let mut planned = Vec::new();
    let mut seen_dimensions = std::collections::HashSet::new();

    for (max_width, max_height, video_bitrate_bps, audio_bitrate_bps) in ladder {
        let (scaled_width, scaled_height) =
            scaled_dimensions_for_rung(width, height, max_width, max_height);
        if scaled_width < 144 || scaled_height < 144 {
            continue;
        }
        if !seen_dimensions.insert((scaled_width, scaled_height)) {
            continue;
        }
        planned.push(HlsVariantPlan {
            label: format!("{}p", scaled_height),
            width: scaled_width,
            height: scaled_height,
            video_bitrate_bps,
            bandwidth_bps: video_bitrate_bps + audio_bitrate_bps,
        });
    }

    if planned.is_empty() {
        let fallback_width = make_even_dimension(width.max(144));
        let fallback_height = make_even_dimension(height.max(144));
        planned.push(HlsVariantPlan {
            label: format!("{}p", fallback_height),
            width: fallback_width,
            height: fallback_height,
            video_bitrate_bps: 1_200_000,
            bandwidth_bps: 1_296_000,
        });
    }

    Ok(planned)
}

fn scaled_dimensions_for_rung(
    source_width: i64,
    source_height: i64,
    max_width: i64,
    max_height: i64,
) -> (i64, i64) {
    let width_ratio = max_width as f64 / source_width as f64;
    let height_ratio = max_height as f64 / source_height as f64;
    let scale = width_ratio.min(height_ratio).min(1.0);

    let scaled_width = make_even_dimension(((source_width as f64) * scale).round() as i64);
    let scaled_height = make_even_dimension(((source_height as f64) * scale).round() as i64);
    (scaled_width.max(2), scaled_height.max(2))
}

fn make_even_dimension(value: i64) -> i64 {
    let value = value.max(2);
    if value % 2 == 0 { value } else { value - 1 }
}

async fn directory_size(path: &FsPath) -> AppResult<i64> {
    let mut total = 0_i64;
    let mut entries = tokio::fs::read_dir(path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.metadata().await?;
        if metadata.is_file() {
            total += metadata.len() as i64;
        }
    }
    Ok(total)
}

pub(super) async fn write_hls_master_manifest(
    output_path: &FsPath,
    variants: &[GeneratedHlsVariant],
    audio_tracks: &[GeneratedHlsAudioTrack],
    subtitle_tracks: &[GeneratedHlsSubtitleTrack],
) -> AppResult<()> {
    let mut body = String::from("#EXTM3U\n#EXT-X-VERSION:3\n");
    for track in audio_tracks {
        body.push_str(&format!(
            "#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"audio\",NAME=\"{}\",LANGUAGE=\"{}\",AUTOSELECT=YES,DEFAULT={},URI=\"{}\"\n",
            track.label,
            track.language,
            if track.is_default { "YES" } else { "NO" },
            track.relative_playlist_path
        ));
    }
    for track in subtitle_tracks {
        body.push_str(&format!(
            "#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"captions\",NAME=\"{}\",LANGUAGE=\"{}\",AUTOSELECT=YES,DEFAULT={},FORCED=NO,URI=\"{}\"\n",
            track.name,
            track.language,
            if track.is_default { "YES" } else { "NO" },
            track.relative_path
        ));
    }
    for variant in variants {
        let codecs = "avc1.64001f,mp4a.40.2";
        body.push_str(&format!(
            "#EXT-X-STREAM-INF:BANDWIDTH={},AVERAGE-BANDWIDTH={},RESOLUTION={}x{},CODECS=\"{}\"{}{}\n{}\n",
            variant.plan.bandwidth_bps,
            variant.plan.bandwidth_bps,
            variant.plan.width,
            variant.plan.height,
            codecs,
            if audio_tracks.is_empty() {
                String::new()
            } else {
                ",AUDIO=\"audio\"".to_string()
            },
            if subtitle_tracks.is_empty() {
                String::new()
            } else {
                ",SUBTITLES=\"captions\"".to_string()
            },
            variant.relative_playlist_path
        ));
    }
    tokio::fs::write(output_path, body).await?;
    Ok(())
}

pub(super) async fn validate_generated_hls_package(
    master_path: &FsPath,
    package: &GeneratedHlsPackage,
) -> AppResult<()> {
    if package.variants.is_empty() {
        return Err(AppError::MediaPipeline(
            "generated HLS package did not produce any playback variants".to_string(),
        ));
    }

    let master_body = tokio::fs::read_to_string(master_path).await?;
    let stream_inf_count = master_body
        .lines()
        .filter(|line| line.starts_with("#EXT-X-STREAM-INF"))
        .count();
    if stream_inf_count != package.variants.len() {
        return Err(AppError::MediaPipeline(format!(
            "generated HLS master manifest expected {} stream entries but found {}",
            package.variants.len(),
            stream_inf_count
        )));
    }

    let master_dir = master_path.parent().ok_or_else(|| {
        AppError::MediaPipeline("generated HLS master manifest has no parent directory".to_string())
    })?;
    let subtitle_media_lines = master_body
        .lines()
        .filter(|line| line.starts_with("#EXT-X-MEDIA:TYPE=SUBTITLES"))
        .collect::<Vec<_>>();
    let audio_media_lines = master_body
        .lines()
        .filter(|line| line.starts_with("#EXT-X-MEDIA:TYPE=AUDIO"))
        .collect::<Vec<_>>();
    if audio_media_lines.len() != package.audio_tracks.len() {
        return Err(AppError::MediaPipeline(format!(
            "generated HLS master manifest expected {} audio track entries but found {}",
            package.audio_tracks.len(),
            audio_media_lines.len()
        )));
    }
    if subtitle_media_lines.len() != package.subtitle_tracks.len() {
        return Err(AppError::MediaPipeline(format!(
            "generated HLS master manifest expected {} subtitle track entries but found {}",
            package.subtitle_tracks.len(),
            subtitle_media_lines.len()
        )));
    }
    let listed_variant_paths = master_body
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect::<Vec<_>>();
    if listed_variant_paths.len() != package.variants.len() {
        return Err(AppError::MediaPipeline(format!(
            "generated HLS master manifest expected {} variant playlist paths but found {}",
            package.variants.len(),
            listed_variant_paths.len()
        )));
    }

    for variant in &package.variants {
        if !listed_variant_paths
            .iter()
            .any(|path| path == &variant.relative_playlist_path)
        {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS master manifest is missing variant playlist {}",
                variant.relative_playlist_path
            )));
        }

        let playlist_path = master_dir.join(&variant.relative_playlist_path);
        let playlist_body = tokio::fs::read_to_string(&playlist_path).await?;
        if !playlist_body.contains("#EXTM3U") {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS variant playlist {} is missing EXTM3U header",
                variant.relative_playlist_path
            )));
        }
        if !playlist_body.contains("#EXTINF") {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS variant playlist {} does not contain media segments",
                variant.relative_playlist_path
            )));
        }

        let playlist_dir = playlist_path.parent().ok_or_else(|| {
            AppError::MediaPipeline(format!(
                "generated HLS variant playlist {} has no parent directory",
                variant.relative_playlist_path
            ))
        })?;
        let segment_paths = playlist_body
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect::<Vec<_>>();
        if segment_paths.is_empty() {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS variant playlist {} does not list any media segments",
                variant.relative_playlist_path
            )));
        }
        for segment in segment_paths {
            let segment_path = playlist_dir.join(segment);
            let metadata = tokio::fs::metadata(&segment_path).await?;
            if !metadata.is_file() || metadata.len() == 0 {
                return Err(AppError::MediaPipeline(format!(
                    "generated HLS segment {} is missing or empty",
                    segment_path.display()
                )));
            }
        }
    }

    for track in &package.audio_tracks {
        if !master_body.contains(&format!("URI=\"{}\"", track.relative_playlist_path)) {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS master manifest is missing audio track {}",
                track.relative_playlist_path
            )));
        }
        let playlist_path = master_dir.join(&track.relative_playlist_path);
        let playlist_body = tokio::fs::read_to_string(&playlist_path).await?;
        if !playlist_body.contains("#EXTM3U") || !playlist_body.contains("#EXTINF") {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS audio playlist {} is incomplete",
                track.relative_playlist_path
            )));
        }
    }

    for track in &package.subtitle_tracks {
        if !master_body.contains(&format!("URI=\"{}\"", track.relative_path)) {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS master manifest is missing subtitle track {}",
                track.relative_path
            )));
        }
        let subtitle_path = master_dir.join(&track.relative_path);
        let subtitle_body = tokio::fs::read_to_string(&subtitle_path).await?;
        if !subtitle_body.starts_with("WEBVTT") {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS subtitle track {} is missing WEBVTT header",
                track.relative_path
            )));
        }
    }

    Ok(())
}
