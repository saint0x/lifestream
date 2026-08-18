use super::*;

pub(crate) async fn begin_media_processing_attempt(
    state: &SharedState,
    creator_id: &str,
    job_id: &str,
) -> AppResult<Option<MediaProcessingAttempt>> {
    let job = fetch_upload_job_by_id(&state.pool, creator_id, job_id).await?;
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
    .execute(&state.pool)
    .await?;
    if claimed.rows_affected() == 0 {
        return Ok(None);
    }

    let job = fetch_upload_job_by_id(&state.pool, creator_id, job_id).await?;
    let session = fetch_upload_ingest_session(&state.pool, creator_id, job_id).await?;
    let asset =
        ensure_media_asset_shell(&state.pool, creator_id, &job, &session.relative_path).await?;
    let source_path = media_path_for_relative(state, &session.relative_path);

    sqlx::query(
        "UPDATE media_assets SET status = 'processing', updated_at = ? WHERE upload_job_id = ? AND creator_id = ?",
    )
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
