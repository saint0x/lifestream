use super::*;

pub(crate) fn require_upload_token(headers: &HeaderMap) -> AppResult<String> {
    let value = headers
        .get("x-upload-token")
        .ok_or(AppError::Unauthorized)?
        .to_str()
        .map_err(|_| AppError::Unauthorized)?
        .trim()
        .to_string();

    if value.is_empty() {
        return Err(AppError::Unauthorized);
    }

    Ok(value)
}

pub(crate) fn require_ingest_token(headers: &HeaderMap) -> AppResult<String> {
    let value = headers
        .get("x-ingest-token")
        .ok_or(AppError::Unauthorized)?
        .to_str()
        .map_err(|_| AppError::Unauthorized)?
        .trim()
        .to_string();

    if value.is_empty() {
        return Err(AppError::Unauthorized);
    }

    Ok(value)
}

pub(crate) async fn validate_upload_ingest_token(
    pool: &SqlitePool,
    creator_id: &str,
    job_id: &str,
    upload_token: &str,
) -> AppResult<()> {
    let token_hash = crate::auth::hash_token(upload_token);
    let exists = sqlx::query(
        "SELECT 1 FROM upload_job_ingest_sessions WHERE creator_id = ? AND job_id = ? AND upload_token_hash = ?",
    )
    .bind(creator_id)
    .bind(job_id)
    .bind(token_hash)
    .fetch_optional(pool)
    .await?
    .is_some();

    if exists {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}
