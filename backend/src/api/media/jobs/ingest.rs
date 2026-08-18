use super::*;

pub(crate) async fn get_upload_ingest_session(
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

pub(crate) async fn start_upload_ingest_session(
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

pub(crate) async fn append_upload_chunk(
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

pub(crate) async fn complete_upload_ingest(
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
