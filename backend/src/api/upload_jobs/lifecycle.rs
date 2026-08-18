use super::*;

pub(crate) async fn list_upload_jobs(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<UploadJob>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(fetch_upload_jobs(&state.pool, creator_id).await?))
}

pub(crate) async fn create_upload_job(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<CreateUploadJobRequest>,
) -> AppResult<Json<UploadJob>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-upload-job-create:{}", identity.user_id),
        30,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    ensure_creator_upload_ingest_enabled(&state.pool, creator_id).await?;
    if input.title.trim().is_empty() || input.kind.trim().is_empty() {
        return Err(AppError::BadRequest(
            "title and kind are required".to_string(),
        ));
    }
    if input.bytes_expected <= 0 {
        return Err(AppError::BadRequest(
            "bytesExpected must be greater than zero".to_string(),
        ));
    }
    validate_upload_job_kind(input.kind.trim())?;
    validate_upload_job_source_type(input.source_type.trim())?;
    validate_upload_visibility(input.intended_visibility.trim())?;
    let storage_key = sanitize_storage_key(input.storage_key.trim())?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO upload_jobs (
            id, creator_id, upload_id, series_id, kind, source_type, status, title,
            intended_visibility, bytes_expected, bytes_received, storage_key,
            created_at, updated_at, published_content_id, mime_type, checksum_sha256, completed_at,
            processing_attempt_count, last_processing_error, last_failed_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(creator_id)
    .bind(input.upload_id)
    .bind(input.series_id)
    .bind(input.kind.trim())
    .bind(input.source_type.trim())
    .bind("created")
    .bind(input.title.trim())
    .bind(input.intended_visibility.trim())
    .bind(input.bytes_expected)
    .bind(0_i64)
    .bind(&storage_key)
    .bind(&now)
    .bind(&now)
    .bind(Option::<String>::None)
    .bind(
        input
            .mime_type
            .as_deref()
            .unwrap_or("application/octet-stream"),
    )
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(0_i64)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .execute(&state.pool)
    .await?;

    Ok(Json(
        fetch_upload_job_by_id(&state.pool, creator_id, &id).await?,
    ))
}

pub(crate) async fn update_upload_job(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<UpdateUploadJobRequest>,
) -> AppResult<Json<UploadJob>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-upload-job-update:{}", identity.user_id),
        60,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    let current = fetch_upload_job_by_id(&state.pool, creator_id, &id).await?;
    if current.status == "processing" {
        return Err(AppError::BadRequest(
            "processing upload jobs cannot be edited".to_string(),
        ));
    }
    if current.status == "published" {
        return Err(AppError::BadRequest(
            "published upload jobs cannot be edited through upload-job controls".to_string(),
        ));
    }

    let title = input
        .title
        .as_deref()
        .map(str::trim)
        .unwrap_or(current.title.as_str());
    if title.is_empty() {
        return Err(AppError::BadRequest("title must not be empty".to_string()));
    }

    let intended_visibility = input
        .intended_visibility
        .as_deref()
        .unwrap_or(current.intended_visibility.as_str());
    validate_upload_visibility(intended_visibility)?;

    let mime_type = input
        .mime_type
        .as_deref()
        .map(str::trim)
        .unwrap_or(current.mime_type.as_str());
    if mime_type.is_empty() {
        return Err(AppError::BadRequest(
            "mimeType must not be empty".to_string(),
        ));
    }

    let series_id = match input.series_id {
        Some(series_id) => {
            if fetch_creator_series_title(&state.pool, creator_id, &series_id)
                .await?
                .is_none()
            {
                return Err(AppError::BadRequest(
                    "seriesId must reference one of the creator's series".to_string(),
                ));
            }
            Some(series_id)
        }
        None => current.series_id,
    };

    sqlx::query(
        r#"
        UPDATE upload_jobs
        SET title = ?, intended_visibility = ?, series_id = ?, mime_type = ?, updated_at = ?
        WHERE id = ? AND creator_id = ?
        "#,
    )
    .bind(title)
    .bind(intended_visibility)
    .bind(series_id)
    .bind(mime_type)
    .bind(Utc::now().to_rfc3339())
    .bind(&id)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;

    Ok(Json(
        fetch_upload_job_by_id(&state.pool, creator_id, &id).await?,
    ))
}
