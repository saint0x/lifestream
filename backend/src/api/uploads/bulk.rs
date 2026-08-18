use super::*;

pub(super) async fn bulk_uploads(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<BulkUploadRequest>,
) -> AppResult<Json<Vec<Upload>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-bulk-uploads:{}", identity.user_id),
        20,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    if input.upload_ids.is_empty() {
        return Err(AppError::BadRequest(
            "uploadIds cannot be empty".to_string(),
        ));
    }
    let now = Utc::now().to_rfc3339();

    for upload_id in &input.upload_ids {
        let current = fetch_upload_by_id(&state.pool, creator_id, upload_id).await?;
        match input.action.as_str() {
            "archive" => {
                validate_bulk_upload_action(&current, "archive")?;
                sqlx::query(
                    "UPDATE uploads SET status = 'archived' WHERE id = ? AND creator_id = ?",
                )
                .bind(upload_id)
                .bind(creator_id)
                .execute(&state.pool)
                .await?;
                sqlx::query(
                    "UPDATE media_assets SET status = 'archived', updated_at = ? WHERE upload_id = ? AND creator_id = ?",
                )
                .bind(&now)
                .bind(upload_id)
                .bind(creator_id)
                .execute(&state.pool)
                .await?;
                expire_playback_sessions_for_upload(&state.pool, upload_id).await?;
            }
            "make_public" => {
                validate_bulk_upload_action(&current, "make_public")?;
                let next_status = derive_upload_lifecycle_status(
                    current.status.as_str(),
                    "public",
                    current.release_at.as_deref(),
                    &now,
                )?;
                sqlx::query(
                    "UPDATE uploads SET visibility = 'public', status = ?, published_at = CASE WHEN ? = 'published' AND published_at IS NULL THEN ? ELSE published_at END WHERE id = ? AND creator_id = ?",
                )
                .bind(&next_status)
                .bind(&next_status)
                .bind(&now)
                .bind(upload_id)
                .bind(creator_id)
                .execute(&state.pool)
                .await?;
                sqlx::query(
                    "UPDATE media_assets SET visibility = 'public', status = ?, updated_at = ? WHERE upload_id = ? AND creator_id = ?",
                )
                .bind(&next_status)
                .bind(&now)
                .bind(upload_id)
                .bind(creator_id)
                .execute(&state.pool)
                .await?;
                expire_playback_sessions_for_upload(&state.pool, upload_id).await?;
            }
            "make_unlisted" => {
                validate_bulk_upload_action(&current, "make_unlisted")?;
                let next_status = derive_upload_lifecycle_status(
                    current.status.as_str(),
                    "unlisted",
                    current.release_at.as_deref(),
                    &now,
                )?;
                sqlx::query(
                    "UPDATE uploads SET visibility = 'unlisted', status = ?, published_at = CASE WHEN ? = 'published' AND published_at IS NULL THEN ? ELSE published_at END WHERE id = ? AND creator_id = ?",
                )
                .bind(&next_status)
                .bind(&next_status)
                .bind(&now)
                .bind(upload_id)
                .bind(creator_id)
                .execute(&state.pool)
                .await?;
                sqlx::query(
                    "UPDATE media_assets SET visibility = 'unlisted', status = ?, updated_at = ? WHERE upload_id = ? AND creator_id = ?",
                )
                .bind(&next_status)
                .bind(&now)
                .bind(upload_id)
                .bind(creator_id)
                .execute(&state.pool)
                .await?;
                expire_playback_sessions_for_upload(&state.pool, upload_id).await?;
            }
            "delete" => {
                validate_bulk_upload_action(&current, "delete")?;
                let active_purchase_exists = sqlx::query(
                    "SELECT 1 FROM content_purchases WHERE upload_id = ? AND status = 'active' LIMIT 1",
                )
                .bind(upload_id)
                .fetch_optional(&state.pool)
                .await?
                .is_some();
                if active_purchase_exists {
                    return Err(AppError::BadRequest(
                        "cannot delete uploads with active purchases".to_string(),
                    ));
                }
                expire_playback_sessions_for_upload(&state.pool, upload_id).await?;
                sqlx::query("DELETE FROM media_assets WHERE upload_id = ? AND creator_id = ?")
                    .bind(upload_id)
                    .bind(creator_id)
                    .execute(&state.pool)
                    .await?;
                sqlx::query("DELETE FROM upload_jobs WHERE upload_id = ? AND creator_id = ?")
                    .bind(upload_id)
                    .bind(creator_id)
                    .execute(&state.pool)
                    .await?;
                sqlx::query("DELETE FROM uploads WHERE id = ? AND creator_id = ?")
                    .bind(upload_id)
                    .bind(creator_id)
                    .execute(&state.pool)
                    .await?;
            }
            other => {
                return Err(AppError::BadRequest(format!(
                    "unsupported bulk action: {other}"
                )));
            }
        }
    }

    Ok(Json(fetch_uploads(&state.pool, creator_id).await?))
}
