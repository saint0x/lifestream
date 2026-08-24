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
    update_upload_lifecycle_record(
        &state.db,
        creator_id,
        &id,
        &visibility,
        release_at,
        &next_status,
        &now,
    )
    .await?;
    sync_upload_media_asset_lifecycle(&state.db, creator_id, &id, &visibility, &next_status, &now)
        .await?;
    expire_playback_sessions_for_upload_in_database(&state.db, &id).await?;
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
    set_upload_fixed_lifecycle(&state.db, creator_id, &id, "private", "draft", &now).await?;
    expire_playback_sessions_for_upload_in_database(&state.db, &id).await?;
    enqueue_upload_lifecycle_notification(
        &state.db,
        "content_unpublished",
        &format!("{} was unpublished.", current.title),
        &identity.user_id,
        creator_id,
        json!({
            "uploadId": id,
            "previousStatus": current.status,
        }),
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
    set_upload_fixed_lifecycle(&state.db, creator_id, &id, "private", "taken_down", &now).await?;
    expire_playback_sessions_for_upload_in_database(&state.db, &id).await?;
    enqueue_upload_lifecycle_notification(
        &state.db,
        "content_takedown",
        &format!("{} was taken down.", current.title),
        &identity.user_id,
        creator_id,
        json!({
            "uploadId": id,
            "previousStatus": current.status,
        }),
    )
    .await?;
    Ok(Json(
        fetch_upload_by_id_for_database(&state.db, creator_id, &id).await?,
    ))
}

async fn update_upload_lifecycle_record(
    database: &crate::db::Database,
    creator_id: &str,
    upload_id: &str,
    visibility: &str,
    release_at: Option<String>,
    status: &str,
    now: &str,
) -> AppResult<()> {
    if let Ok(pool) = database.try_postgres_adapter() {
        sqlx::query(
            r#"
            UPDATE uploads
            SET visibility = $1,
                release_at = $2,
                status = $3,
                published_at = CASE
                    WHEN $4 = 'published' AND published_at IS NULL THEN $5
                    ELSE published_at
                END
            WHERE id = $6 AND creator_id = $7
            "#,
        )
        .bind(visibility)
        .bind(release_at)
        .bind(status)
        .bind(status)
        .bind(now)
        .bind(upload_id)
        .bind(creator_id)
        .execute(pool)
        .await?;
        return Ok(());
    }

    sqlx::query(
        r#"
        UPDATE uploads
        SET visibility = ?,
            release_at = ?,
            status = ?,
            published_at = CASE
                WHEN ? = 'published' AND published_at IS NULL THEN ?
                ELSE published_at
            END
        WHERE id = ? AND creator_id = ?
        "#,
    )
    .bind(visibility)
    .bind(release_at)
    .bind(status)
    .bind(status)
    .bind(now)
    .bind(upload_id)
    .bind(creator_id)
    .execute(database.try_sqlite_adapter()?)
    .await?;
    Ok(())
}

async fn set_upload_fixed_lifecycle(
    database: &crate::db::Database,
    creator_id: &str,
    upload_id: &str,
    visibility: &str,
    status: &str,
    now: &str,
) -> AppResult<()> {
    if let Ok(pool) = database.try_postgres_adapter() {
        sqlx::query(
            "UPDATE uploads SET visibility = $1, status = $2, release_at = NULL WHERE id = $3 AND creator_id = $4",
        )
        .bind(visibility)
        .bind(status)
        .bind(upload_id)
        .bind(creator_id)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            "UPDATE uploads SET visibility = ?, status = ?, release_at = NULL WHERE id = ? AND creator_id = ?",
        )
        .bind(visibility)
        .bind(status)
        .bind(upload_id)
        .bind(creator_id)
        .execute(database.try_sqlite_adapter()?)
        .await?;
    }

    sync_upload_media_asset_lifecycle(database, creator_id, upload_id, visibility, status, now)
        .await
}
