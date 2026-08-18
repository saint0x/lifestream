use super::*;

pub(crate) async fn retry_upload_job_processing(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<UploadJob>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-upload-job-retry:{}", identity.user_id),
        30,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    let job = fetch_upload_job_by_id(&state.pool, creator_id, &id).await?;
    if job.status != "failed" {
        return Err(AppError::BadRequest(
            "only failed upload jobs can be retried".to_string(),
        ));
    }

    let session = fetch_upload_ingest_session(&state.pool, creator_id, &id).await?;
    if session.status != "completed" {
        return Err(AppError::BadRequest(
            "only completed ingest sessions can be retried".to_string(),
        ));
    }

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE upload_jobs SET status = 'uploaded', last_processing_error = NULL, last_failed_at = NULL, updated_at = ? WHERE id = ? AND creator_id = ?",
    )
    .bind(&now)
    .bind(&id)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE media_assets SET status = 'uploaded', updated_at = ? WHERE upload_job_id = ? AND creator_id = ?",
    )
    .bind(&now)
    .bind(&id)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;

    schedule_media_processing(state.clone(), creator_id.to_string(), id.clone()).await;

    Ok(Json(
        fetch_upload_job_by_id(&state.pool, creator_id, &id).await?,
    ))
}
