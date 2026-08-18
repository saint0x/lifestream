use super::*;

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/api/v1/creator/me/upload-jobs",
            get(list_upload_jobs).post(create_upload_job),
        )
        .route(
            "/api/v1/creator/me/upload-jobs/:id",
            patch(update_upload_job),
        )
        .route(
            "/api/v1/creator/me/upload-jobs/:id/ingest",
            get(get_upload_ingest_session).post(start_upload_ingest_session),
        )
        .route(
            "/api/v1/creator/me/upload-jobs/:id/ingest/chunk",
            put(append_upload_chunk),
        )
        .route(
            "/api/v1/creator/me/upload-jobs/:id/ingest/complete",
            post(complete_upload_ingest),
        )
        .route(
            "/api/v1/creator/me/upload-jobs/:id/retry",
            post(retry_upload_job_processing),
        )
        .route("/api/v1/creator/me/media-assets", get(list_media_assets))
        .route(
            "/api/v1/creator/me/upload-jobs/:id/media-asset",
            get(get_media_asset_for_upload_job),
        )
        .route(
            "/api/v1/creator/me/upload-jobs/:id/publish",
            post(publish_upload_job),
        )
}

pub(super) async fn list_upload_jobs(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<UploadJob>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(fetch_upload_jobs(&state.pool, creator_id).await?))
}

pub(super) async fn create_upload_job(
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

pub(super) async fn update_upload_job(
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

pub(super) async fn get_upload_ingest_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<UploadIngestSession>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_upload_ingest_session(&state.pool, creator_id, &id).await?,
    ))
}

pub(super) async fn start_upload_ingest_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<UploadIngestTicket>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-upload-ingest-start:{}", identity.user_id),
        20,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    ensure_creator_upload_ingest_enabled(&state.pool, creator_id).await?;
    let job = fetch_upload_job_by_id(&state.pool, creator_id, &id).await?;
    let storage_key = sanitize_storage_key(&job.storage_key)?;

    if let Ok(session) = fetch_upload_ingest_session(&state.pool, creator_id, &id).await {
        if session.status == "completed" {
            return Err(AppError::BadRequest(
                "ingest session already completed".to_string(),
            ));
        }
        let token = format!("up_{}", Uuid::new_v4().simple());
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE upload_job_ingest_sessions SET upload_token_hash = ?, updated_at = ? WHERE job_id = ? AND creator_id = ?",
        )
        .bind(hash_token(&token))
        .bind(&now)
        .bind(&id)
        .bind(creator_id)
        .execute(&state.pool)
        .await?;
        return Ok(Json(UploadIngestTicket {
            session: fetch_upload_ingest_session(&state.pool, creator_id, &id).await?,
            upload_token: token,
        }));
    }

    let token = format!("up_{}", Uuid::new_v4().simple());
    let relative_path = format!(
        "{creator_id}/{}/{}/{}",
        Utc::now().format("%Y"),
        Utc::now().format("%m"),
        storage_key
    );
    let now = Utc::now().to_rfc3339();
    let file_path = media_path_for_relative(&state, &relative_path);
    ensure_parent_dir(&file_path).await?;
    tokio::fs::File::create(&file_path).await?;

    sqlx::query(
        r#"
        INSERT INTO upload_job_ingest_sessions (
            job_id, creator_id, relative_path, upload_token_hash, status, mime_type,
            bytes_received, created_at, updated_at, completed_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(creator_id)
    .bind(&relative_path)
    .bind(hash_token(&token))
    .bind("active")
    .bind(&job.mime_type)
    .bind(0_i64)
    .bind(&now)
    .bind(&now)
    .bind(Option::<String>::None)
    .execute(&state.pool)
    .await?;

    Ok(Json(UploadIngestTicket {
        session: fetch_upload_ingest_session(&state.pool, creator_id, &id).await?,
        upload_token: token,
    }))
}

pub(super) async fn append_upload_chunk(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<AppendUploadChunkQuery>,
    body: Bytes,
) -> AppResult<Json<UploadIngestSession>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-upload-ingest-chunk:{}", identity.user_id),
        300,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    let upload_token = require_upload_token(&headers)?;
    let session = fetch_upload_ingest_session(&state.pool, creator_id, &id).await?;
    validate_upload_ingest_token(&state.pool, creator_id, &id, &upload_token).await?;
    let job = fetch_upload_job_by_id(&state.pool, creator_id, &id).await?;
    if session.status != "active" {
        return Err(AppError::BadRequest(
            "ingest session is not active".to_string(),
        ));
    }
    if query.offset != session.bytes_received {
        return Err(AppError::BadRequest(format!(
            "chunk offset mismatch: expected {}, got {}",
            session.bytes_received, query.offset
        )));
    }
    let next_bytes_received = session.bytes_received + body.len() as i64;
    if next_bytes_received > job.bytes_expected {
        return Err(AppError::BadRequest(format!(
            "chunk exceeds declared upload size: expected at most {} bytes, got {}",
            job.bytes_expected, next_bytes_received
        )));
    }

    use tokio::io::{AsyncSeekExt, AsyncWriteExt};
    let path = media_path_for_relative(&state, &session.relative_path);
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .await?;
    file.seek(std::io::SeekFrom::Start(query.offset as u64))
        .await?;
    file.write_all(&body).await?;
    file.flush().await?;

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE upload_job_ingest_sessions SET bytes_received = ?, updated_at = ? WHERE job_id = ? AND creator_id = ?",
    )
    .bind(next_bytes_received)
    .bind(&now)
    .bind(&id)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE upload_jobs SET bytes_received = ?, updated_at = ? WHERE id = ? AND creator_id = ?",
    )
    .bind(next_bytes_received)
    .bind(&now)
    .bind(&id)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;

    Ok(Json(
        fetch_upload_ingest_session(&state.pool, creator_id, &id).await?,
    ))
}

pub(super) async fn complete_upload_ingest(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<UploadJob>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-upload-ingest-complete:{}", identity.user_id),
        30,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    let upload_token = require_upload_token(&headers)?;
    let session = fetch_upload_ingest_session(&state.pool, creator_id, &id).await?;
    validate_upload_ingest_token(&state.pool, creator_id, &id, &upload_token).await?;
    let job = fetch_upload_job_by_id(&state.pool, creator_id, &id).await?;
    if session.bytes_received != job.bytes_expected {
        return Err(AppError::BadRequest(format!(
            "upload incomplete: expected {} bytes, received {}",
            job.bytes_expected, session.bytes_received
        )));
    }

    let path = media_path_for_relative(&state, &session.relative_path);
    let digest = sha256_file(&path).await?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE upload_job_ingest_sessions SET status = 'completed', updated_at = ?, completed_at = ? WHERE job_id = ? AND creator_id = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(&id)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE upload_jobs SET status = 'uploaded', checksum_sha256 = ?, completed_at = ?, updated_at = ?, processing_attempt_count = 0, last_processing_error = NULL, last_failed_at = NULL WHERE id = ? AND creator_id = ?",
    )
    .bind(&digest)
    .bind(&now)
    .bind(&now)
    .bind(&id)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;

    ensure_media_asset_shell(&state.pool, creator_id, &job, &session.relative_path).await?;
    schedule_media_processing(state.clone(), creator_id.to_string(), id.clone()).await;

    Ok(Json(
        fetch_upload_job_by_id(&state.pool, creator_id, &id).await?,
    ))
}

pub(super) async fn retry_upload_job_processing(
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

pub(super) async fn list_media_assets(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<MediaAsset>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(fetch_media_assets(&state.pool, creator_id).await?))
}

pub(super) async fn get_media_asset_for_upload_job(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<MediaAsset>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_media_asset_by_upload_job(&state.pool, creator_id, &id).await?,
    ))
}

pub(super) async fn publish_upload_job(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<PublishUploadJobRequest>,
) -> AppResult<Json<Upload>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-upload-publish:{}", identity.user_id),
        30,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    ensure_creator_upload_ingest_enabled(&state.pool, creator_id).await?;
    let job = fetch_upload_job_by_id(&state.pool, creator_id, &id).await?;
    if job.status != "ready" && job.status != "published" {
        return Err(AppError::BadRequest(
            "upload job must be ready before publish".to_string(),
        ));
    }

    let asset = fetch_media_asset_by_upload_job(&state.pool, creator_id, &id).await?;
    if asset.playback_path.is_none() {
        return Err(AppError::BadRequest(
            "media asset does not yet have a playback manifest".to_string(),
        ));
    }

    let visibility = input
        .visibility
        .unwrap_or_else(|| job.intended_visibility.clone());
    let access_terms = resolve_upload_access_terms(
        input.access_policy.clone(),
        input.access_tier_id.clone(),
        input.price_cents,
        input.currency.clone(),
        input.rental_window_hours,
    )?;
    if monetized_access_policy(&access_terms.access_policy) {
        ensure_creator_can_publish_paid_content(&state.pool, creator_id).await?;
    }
    validate_creator_access_tier(
        &state.pool,
        creator_id,
        &access_terms.access_policy,
        access_terms.access_tier_id.as_deref(),
    )
    .await?;
    let slug = sanitize_slug(input.slug.as_deref().unwrap_or(&slugify(&asset.title)))?;
    let upload_id = job
        .upload_id
        .clone()
        .unwrap_or_else(|| format!("upl-{}", Uuid::new_v4().simple()));
    let now = Utc::now().to_rfc3339();
    let release_at = input.release_at.unwrap_or_else(|| now.clone());
    let is_released = release_at <= now;
    let resolution = match (asset.width, asset.height) {
        (Some(width), Some(height)) => format!("{width}x{height}"),
        _ => "audio".to_string(),
    };
    let series_title = if let Some(series_id) = job.series_id.as_deref() {
        fetch_creator_series_title(&state.pool, creator_id, series_id).await?
    } else {
        None
    };
    let thumbnail = asset
        .variants
        .iter()
        .find(|variant| {
            variant.variant_type == "thumbnail"
                && (variant.label == "card_thumbnail" || variant.is_default)
        })
        .map(|variant| variant.url.clone())
        .or_else(|| asset.poster_url.clone())
        .unwrap_or_else(|| "https://cdn.lifestream.local/thumb/upload-default.jpg".to_string());
    let upload_status = if is_released && (visibility == "public" || visibility == "unlisted") {
        "published"
    } else if !is_released {
        "scheduled"
    } else {
        "draft"
    };
    let published_at = if upload_status == "published" {
        Some(now.clone())
    } else {
        None
    };

    sqlx::query(
        r#"
        INSERT INTO uploads (
            id, creator_id, slug, title, description, kind, duration_sec, uploaded_at, published_at, release_at, status,
            visibility, access_policy, access_tier_id, price_cents, currency, rental_window_hours,
            views, likes, comments, watch_hours, thumbnail, series_title,
            season_number, episode_number, size_bytes, resolution, transcode_progress, series_id
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            creator_id = excluded.creator_id,
            slug = excluded.slug,
            title = excluded.title,
            description = excluded.description,
            kind = excluded.kind,
            published_at = excluded.published_at,
            release_at = excluded.release_at,
            status = excluded.status,
            visibility = excluded.visibility,
            access_policy = excluded.access_policy,
            access_tier_id = excluded.access_tier_id,
            price_cents = excluded.price_cents,
            currency = excluded.currency,
            rental_window_hours = excluded.rental_window_hours,
            thumbnail = excluded.thumbnail,
            series_title = excluded.series_title,
            season_number = excluded.season_number,
            episode_number = excluded.episode_number,
            size_bytes = excluded.size_bytes,
            resolution = excluded.resolution,
            transcode_progress = excluded.transcode_progress,
            series_id = excluded.series_id
        "#,
    )
    .bind(&upload_id)
    .bind(creator_id)
    .bind(&slug)
    .bind(&asset.title)
    .bind(input.description.unwrap_or_default())
    .bind(&asset.kind)
    .bind(asset.duration_sec.round() as i64)
    .bind(job.completed_at.clone().unwrap_or_else(|| now.clone()))
    .bind(published_at)
    .bind(&release_at)
    .bind(upload_status)
    .bind(&visibility)
    .bind(&access_terms.access_policy)
    .bind(access_terms.access_tier_id.clone())
    .bind(access_terms.price_cents)
    .bind(access_terms.currency.clone())
    .bind(access_terms.rental_window_hours)
    .bind(0_i64)
    .bind(0_i64)
    .bind(0_i64)
    .bind(0_i64)
    .bind(&thumbnail)
    .bind(series_title)
    .bind(input.season_number)
    .bind(input.episode_number)
    .bind(asset.file_size_bytes)
    .bind(&resolution)
    .bind(1.0_f64)
    .bind(job.series_id.clone())
    .execute(&state.pool)
    .await?;

    if let (Some(series_id), Some(season_number)) = (job.series_id.as_deref(), input.season_number)
    {
        ensure_creator_series_season(
            &state.pool,
            creator_id,
            series_id,
            season_number,
            input
                .season_title
                .unwrap_or_else(|| format!("Season {season_number}")),
            input.season_synopsis.unwrap_or_default(),
        )
        .await?;
    }

    sqlx::query(
        "UPDATE upload_jobs SET status = 'published', upload_id = ?, published_content_id = ?, updated_at = ? WHERE id = ? AND creator_id = ?",
    )
    .bind(&upload_id)
    .bind(&upload_id)
    .bind(&now)
    .bind(&id)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;

    sqlx::query(
        "UPDATE media_assets SET status = ?, visibility = ?, upload_id = ?, published_content_id = ?, updated_at = ? WHERE upload_job_id = ? AND creator_id = ?",
    )
    .bind(upload_status)
    .bind(&visibility)
    .bind(&upload_id)
    .bind(&upload_id)
    .bind(&now)
    .bind(&id)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;
    let creator_profile = fetch_creator_profile(&state.pool, creator_id).await?;
    enqueue_notification_event(
        &state.pool,
        "content_release",
        &format!("{} is now {}.", asset.title, upload_status),
        Some(&identity.user_id),
        Some(&creator_profile.display_name),
        Some(creator_id),
        None,
        None,
        json!({
            "uploadId": upload_id,
            "jobId": id,
            "status": upload_status,
            "releaseAt": release_at,
            "visibility": visibility,
            "slug": slug,
        }),
        &[],
        &[creator_id.to_string()],
    )
    .await?;

    Ok(Json(
        fetch_upload_by_id(&state.pool, creator_id, &upload_id).await?,
    ))
}
