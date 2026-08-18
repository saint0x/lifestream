use super::*;

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
        let _ = super::failures::fail_media_job(
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
    let before =
        super::queries::fetch_upload_job_by_id_raw(&state.pool, &creator_id, job_id).await?;
    let now = Utc::now().to_rfc3339();
    let mut actions = Vec::new();

    if before.status == "processing" && is_upload_job_stale(&before) {
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
            let after =
                super::queries::fetch_upload_job_by_id_raw(&state.pool, &creator_id, job_id)
                    .await?;
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
