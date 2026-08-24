use super::*;

pub(super) async fn bulk_uploads(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<BulkUploadRequest>,
) -> AppResult<Json<Vec<Upload>>> {
    let identity = require_identity(&state.db, &headers).await?;
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
        let current = fetch_upload_by_id_for_database(&state.db, creator_id, upload_id).await?;
        match input.action.as_str() {
            "archive" => {
                validate_bulk_upload_action(&current, "archive")?;
                update_bulk_upload_status(&state.db, creator_id, upload_id, "archived", &now)
                    .await?;
                expire_playback_sessions_for_upload_in_database(&state.db, upload_id).await?;
            }
            "make_public" => {
                validate_bulk_upload_action(&current, "make_public")?;
                let next_status = derive_upload_lifecycle_status(
                    current.status.as_str(),
                    "public",
                    current.release_at.as_deref(),
                    &now,
                )?;
                update_bulk_upload_visibility(
                    &state.db,
                    creator_id,
                    upload_id,
                    "public",
                    &next_status,
                    &now,
                )
                .await?;
                expire_playback_sessions_for_upload_in_database(&state.db, upload_id).await?;
            }
            "make_unlisted" => {
                validate_bulk_upload_action(&current, "make_unlisted")?;
                let next_status = derive_upload_lifecycle_status(
                    current.status.as_str(),
                    "unlisted",
                    current.release_at.as_deref(),
                    &now,
                )?;
                update_bulk_upload_visibility(
                    &state.db,
                    creator_id,
                    upload_id,
                    "unlisted",
                    &next_status,
                    &now,
                )
                .await?;
                expire_playback_sessions_for_upload_in_database(&state.db, upload_id).await?;
            }
            "delete" => {
                validate_bulk_upload_action(&current, "delete")?;
                let active_purchase_exists =
                    upload_has_active_purchase(&state.db, upload_id).await?;
                if active_purchase_exists {
                    return Err(AppError::BadRequest(
                        "cannot delete uploads with active purchases".to_string(),
                    ));
                }
                expire_playback_sessions_for_upload_in_database(&state.db, upload_id).await?;
                delete_upload_records(&state.db, creator_id, upload_id).await?;
            }
            other => {
                return Err(AppError::BadRequest(format!(
                    "unsupported bulk action: {other}"
                )));
            }
        }
    }

    Ok(Json(
        fetch_uploads_for_database(&state.db, creator_id).await?,
    ))
}

async fn update_bulk_upload_status(
    database: &crate::db::Database,
    creator_id: &str,
    upload_id: &str,
    status: &str,
    now: &str,
) -> AppResult<()> {
    if let Ok(pool) = database.try_postgres_adapter() {
        sqlx::query("UPDATE uploads SET status = $1 WHERE id = $2 AND creator_id = $3")
            .bind(status)
            .bind(upload_id)
            .bind(creator_id)
            .execute(pool)
            .await?;
    } else {
        sqlx::query("UPDATE uploads SET status = ? WHERE id = ? AND creator_id = ?")
            .bind(status)
            .bind(upload_id)
            .bind(creator_id)
            .execute(database.try_sqlite_adapter()?)
            .await?;
    }

    update_media_asset_status(database, creator_id, upload_id, status, now).await
}

async fn update_bulk_upload_visibility(
    database: &crate::db::Database,
    creator_id: &str,
    upload_id: &str,
    visibility: &str,
    status: &str,
    now: &str,
) -> AppResult<()> {
    if let Ok(pool) = database.try_postgres_adapter() {
        sqlx::query(
            r#"
            UPDATE uploads
            SET visibility = $1,
                status = $2,
                published_at = CASE
                    WHEN $3 = 'published' AND published_at IS NULL THEN $4
                    ELSE published_at
                END
            WHERE id = $5 AND creator_id = $6
            "#,
        )
        .bind(visibility)
        .bind(status)
        .bind(status)
        .bind(now)
        .bind(upload_id)
        .bind(creator_id)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            r#"
            UPDATE uploads
            SET visibility = ?,
                status = ?,
                published_at = CASE
                    WHEN ? = 'published' AND published_at IS NULL THEN ?
                    ELSE published_at
                END
            WHERE id = ? AND creator_id = ?
            "#,
        )
        .bind(visibility)
        .bind(status)
        .bind(status)
        .bind(now)
        .bind(upload_id)
        .bind(creator_id)
        .execute(database.try_sqlite_adapter()?)
        .await?;
    }

    sync_upload_media_asset_lifecycle(database, creator_id, upload_id, visibility, status, now)
        .await
}

async fn upload_has_active_purchase(
    database: &crate::db::Database,
    upload_id: &str,
) -> AppResult<bool> {
    if let Ok(pool) = database.try_postgres_adapter() {
        return Ok(sqlx::query(
            "SELECT 1 FROM content_purchases WHERE upload_id = $1 AND status = 'active' LIMIT 1",
        )
        .bind(upload_id)
        .fetch_optional(pool)
        .await?
        .is_some());
    }

    Ok(sqlx::query(
        "SELECT 1 FROM content_purchases WHERE upload_id = ? AND status = 'active' LIMIT 1",
    )
    .bind(upload_id)
    .fetch_optional(database.try_sqlite_adapter()?)
    .await?
    .is_some())
}

async fn delete_upload_records(
    database: &crate::db::Database,
    creator_id: &str,
    upload_id: &str,
) -> AppResult<()> {
    if let Ok(pool) = database.try_postgres_adapter() {
        sqlx::query("DELETE FROM media_assets WHERE upload_id = $1 AND creator_id = $2")
            .bind(upload_id)
            .bind(creator_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM upload_jobs WHERE upload_id = $1 AND creator_id = $2")
            .bind(upload_id)
            .bind(creator_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM uploads WHERE id = $1 AND creator_id = $2")
            .bind(upload_id)
            .bind(creator_id)
            .execute(pool)
            .await?;
        return Ok(());
    }

    sqlx::query("DELETE FROM media_assets WHERE upload_id = ? AND creator_id = ?")
        .bind(upload_id)
        .bind(creator_id)
        .execute(database.try_sqlite_adapter()?)
        .await?;
    sqlx::query("DELETE FROM upload_jobs WHERE upload_id = ? AND creator_id = ?")
        .bind(upload_id)
        .bind(creator_id)
        .execute(database.try_sqlite_adapter()?)
        .await?;
    sqlx::query("DELETE FROM uploads WHERE id = ? AND creator_id = ?")
        .bind(upload_id)
        .bind(creator_id)
        .execute(database.try_sqlite_adapter()?)
        .await?;
    Ok(())
}

async fn update_media_asset_status(
    database: &crate::db::Database,
    creator_id: &str,
    upload_id: &str,
    status: &str,
    now: &str,
) -> AppResult<()> {
    if let Ok(pool) = database.try_postgres_adapter() {
        sqlx::query(
            "UPDATE media_assets SET status = $1, updated_at = $2 WHERE upload_id = $3 AND creator_id = $4",
        )
        .bind(status)
        .bind(now)
        .bind(upload_id)
        .bind(creator_id)
        .execute(pool)
        .await?;
        return Ok(());
    }

    sqlx::query(
        "UPDATE media_assets SET status = ?, updated_at = ? WHERE upload_id = ? AND creator_id = ?",
    )
    .bind(status)
    .bind(now)
    .bind(upload_id)
    .bind(creator_id)
    .execute(database.try_sqlite_adapter()?)
    .await?;
    Ok(())
}
