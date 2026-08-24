use super::super::ingest::{get_creator_upload_ingest_session, get_creator_upload_job};
use super::*;

pub(crate) async fn retry_upload_job_processing(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<UploadJob>> {
    let identity = require_identity(&state.db, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-upload-job-retry:{}", identity.user_id),
        30,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    let job = get_creator_upload_job(&state.db, creator_id, &id).await?;
    if job.status != "failed" {
        return Err(AppError::BadRequest(
            "only failed upload jobs can be retried".to_string(),
        ));
    }

    let session = get_creator_upload_ingest_session(&state.db, creator_id, &id).await?;
    if session.status != "completed" {
        return Err(AppError::BadRequest(
            "only completed ingest sessions can be retried".to_string(),
        ));
    }

    retry_creator_upload_job_processing_state(&state.db, creator_id, &id, &Utc::now().to_rfc3339())
        .await?;

    schedule_media_processing(state.clone(), creator_id.to_string(), id.clone()).await;

    Ok(Json(
        get_creator_upload_job(&state.db, creator_id, &id).await?,
    ))
}

async fn retry_creator_upload_job_processing_state(
    database: &crate::db::Database,
    creator_id: &str,
    job_id: &str,
    updated_at: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE upload_jobs SET status = 'uploaded', last_processing_error = NULL, last_failed_at = NULL, updated_at = ? WHERE id = ? AND creator_id = ?",
    )
    .bind(updated_at)
    .bind(job_id)
    .bind(creator_id)
    .execute(database.try_sqlite_adapter()?)
    .await?;
    sqlx::query(
        "UPDATE media_assets SET status = 'uploaded', updated_at = ? WHERE upload_job_id = ? AND creator_id = ?",
    )
    .bind(updated_at)
    .bind(job_id)
    .bind(creator_id)
    .execute(database.try_sqlite_adapter()?)
    .await?;
    Ok(())
}
