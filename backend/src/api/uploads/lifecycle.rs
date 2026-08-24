use super::*;

pub(super) async fn update_upload_lifecycle(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<UpdateUploadLifecycleRequest>,
) -> AppResult<Json<Upload>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let current = fetch_upload_by_id_for_database(&state.db, creator_id, &id).await?;
    if current.status == "taken_down" {
        return Err(AppError::BadRequest(
            "taken-down uploads cannot be updated through lifecycle patch".to_string(),
        ));
    }
    let visibility = input.visibility.unwrap_or(current.visibility.clone());
    validate_upload_visibility(&visibility)?;
    let now = Utc::now().to_rfc3339();
    let release_at = input.release_at.or(current.release_at.clone());
    let next_status = derive_upload_lifecycle_status(
        current.status.as_str(),
        &visibility,
        release_at.as_deref(),
        &now,
    )?;
    sqlx::query(
        "UPDATE uploads SET visibility = ?, release_at = ?, status = ?, published_at = CASE WHEN ? = 'published' AND published_at IS NULL THEN ? ELSE published_at END WHERE id = ?",
    )
    .bind(&visibility)
    .bind(&release_at)
    .bind(&next_status)
    .bind(&next_status)
    .bind(&now)
    .bind(&id)
    .execute(state.db.try_sqlite_adapter()?)
    .await?;
    sqlx::query(
        "UPDATE media_assets SET visibility = ?, status = ?, updated_at = ? WHERE upload_id = ? AND creator_id = ?",
    )
    .bind(&visibility)
    .bind(&next_status)
    .bind(&now)
    .bind(&id)
    .bind(creator_id)
    .execute(state.db.try_sqlite_adapter()?)
    .await?;
    expire_playback_sessions_for_upload(state.db.try_sqlite_adapter()?, &id).await?;
    Ok(Json(
        fetch_upload_by_id_for_database(&state.db, creator_id, &id).await?,
    ))
}

pub(crate) async fn unpublish_upload(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<Upload>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let current = fetch_upload_by_id_for_database(&state.db, creator_id, &id).await?;
    if current.status == "taken_down" {
        return Err(AppError::BadRequest(
            "taken-down uploads cannot be unpublished".to_string(),
        ));
    }
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE uploads SET visibility = 'private', status = 'draft', release_at = NULL WHERE id = ?",
    )
    .bind(&id)
    .execute(state.db.try_sqlite_adapter()?)
    .await?;
    sqlx::query(
        "UPDATE media_assets SET visibility = 'private', status = 'draft', updated_at = ? WHERE upload_id = ? AND creator_id = ?",
    )
    .bind(&now)
    .bind(&id)
    .bind(creator_id)
    .execute(state.db.try_sqlite_adapter()?)
    .await?;
    expire_playback_sessions_for_upload(state.db.try_sqlite_adapter()?, &id).await?;
    enqueue_notification_event(
        state.db.try_sqlite_adapter()?,
        "content_unpublished",
        &format!("{} was unpublished.", current.title),
        Some(&identity.user_id),
        Some("creator"),
        Some(creator_id),
        None,
        None,
        json!({
            "uploadId": id,
            "previousStatus": current.status,
        }),
        &[],
        &[creator_id.to_string()],
    )
    .await?;
    Ok(Json(
        fetch_upload_by_id_for_database(&state.db, creator_id, &id).await?,
    ))
}

pub(crate) async fn takedown_upload(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<Upload>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let current = fetch_upload_by_id_for_database(&state.db, creator_id, &id).await?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE uploads SET visibility = 'private', status = 'taken_down', release_at = NULL WHERE id = ?",
    )
    .bind(&id)
    .execute(state.db.try_sqlite_adapter()?)
    .await?;
    sqlx::query(
        "UPDATE media_assets SET visibility = 'private', status = 'taken_down', updated_at = ? WHERE upload_id = ? AND creator_id = ?",
    )
    .bind(&now)
    .bind(&id)
    .bind(creator_id)
    .execute(state.db.try_sqlite_adapter()?)
    .await?;
    expire_playback_sessions_for_upload(state.db.try_sqlite_adapter()?, &id).await?;
    enqueue_notification_event(
        state.db.try_sqlite_adapter()?,
        "content_takedown",
        &format!("{} was taken down.", current.title),
        Some(&identity.user_id),
        Some("creator"),
        Some(creator_id),
        None,
        None,
        json!({
            "uploadId": id,
            "previousStatus": current.status,
        }),
        &[],
        &[creator_id.to_string()],
    )
    .await?;
    Ok(Json(
        fetch_upload_by_id_for_database(&state.db, creator_id, &id).await?,
    ))
}
