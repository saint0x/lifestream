use super::lifecycle::ensure_creator_upload_ingest_enabled_for_jobs;
use super::*;

pub(crate) async fn get_upload_ingest_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<UploadIngestSession>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        get_creator_upload_ingest_session(&state.db, creator_id, &id).await?,
    ))
}

pub(crate) async fn start_upload_ingest_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<UploadIngestTicket>> {
    let identity = require_identity(&state.db, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-upload-ingest-start:{}", identity.user_id),
        20,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    ensure_creator_upload_ingest_enabled_for_jobs(&state.db, creator_id).await?;
    let job = get_creator_upload_job(&state.db, creator_id, &id).await?;
    let storage_key = sanitize_storage_key(&job.storage_key)?;

    if let Ok(session) = get_creator_upload_ingest_session(&state.db, creator_id, &id).await {
        if session.status == "completed" {
            return Err(AppError::BadRequest(
                "ingest session already completed".to_string(),
            ));
        }
        let token = format!("up_{}", Uuid::new_v4().simple());
        let now = Utc::now().to_rfc3339();
        rotate_creator_upload_ingest_token(&state.db, creator_id, &id, &token, &now).await?;
        return Ok(Json(UploadIngestTicket {
            session: get_creator_upload_ingest_session(&state.db, creator_id, &id).await?,
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

    create_creator_upload_ingest_session(
        &state.db,
        creator_id,
        &id,
        &relative_path,
        &token,
        &job.mime_type,
        &now,
    )
    .await?;

    Ok(Json(UploadIngestTicket {
        session: get_creator_upload_ingest_session(&state.db, creator_id, &id).await?,
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
    let identity = require_identity(&state.db, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-upload-ingest-chunk:{}", identity.user_id),
        300,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    let upload_token = require_upload_token(&headers)?;
    let session = get_creator_upload_ingest_session(&state.db, creator_id, &id).await?;
    validate_creator_upload_ingest_token(&state.db, creator_id, &id, &upload_token).await?;
    let job = get_creator_upload_job(&state.db, creator_id, &id).await?;
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
    update_creator_upload_ingest_progress(&state.db, creator_id, &id, next_bytes_received, &now)
        .await?;

    Ok(Json(
        get_creator_upload_ingest_session(&state.db, creator_id, &id).await?,
    ))
}

pub(crate) async fn complete_upload_ingest(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<UploadJob>> {
    let identity = require_identity(&state.db, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-upload-ingest-complete:{}", identity.user_id),
        30,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    let upload_token = require_upload_token(&headers)?;
    let session = get_creator_upload_ingest_session(&state.db, creator_id, &id).await?;
    validate_creator_upload_ingest_token(&state.db, creator_id, &id, &upload_token).await?;
    let job = get_creator_upload_job(&state.db, creator_id, &id).await?;
    if session.bytes_received != job.bytes_expected {
        return Err(AppError::BadRequest(format!(
            "upload incomplete: expected {} bytes, received {}",
            job.bytes_expected, session.bytes_received
        )));
    }

    let path = media_path_for_relative(&state, &session.relative_path);
    let digest = sha256_file(&path).await?;
    let now = Utc::now().to_rfc3339();
    complete_creator_upload_ingest(&state.db, creator_id, &id, &digest, &now).await?;

    ensure_media_asset_shell_for_ingest(&state.db, creator_id, &job, &session.relative_path)
        .await?;
    schedule_media_processing(state.clone(), creator_id.to_string(), id.clone()).await;

    Ok(Json(
        get_creator_upload_job(&state.db, creator_id, &id).await?,
    ))
}

pub(crate) async fn get_creator_upload_job(
    database: &crate::db::Database,
    creator_id: &str,
    job_id: &str,
) -> AppResult<UploadJob> {
    fetch_upload_job_by_id(database.sqlite_adapter(), creator_id, job_id).await
}

pub(crate) async fn get_creator_upload_ingest_session(
    database: &crate::db::Database,
    creator_id: &str,
    job_id: &str,
) -> AppResult<UploadIngestSession> {
    fetch_upload_ingest_session(database.sqlite_adapter(), creator_id, job_id).await
}

pub(crate) async fn validate_creator_upload_ingest_token(
    database: &crate::db::Database,
    creator_id: &str,
    job_id: &str,
    upload_token: &str,
) -> AppResult<()> {
    validate_upload_ingest_token(database.sqlite_adapter(), creator_id, job_id, upload_token).await
}

async fn rotate_creator_upload_ingest_token(
    database: &crate::db::Database,
    creator_id: &str,
    job_id: &str,
    upload_token: &str,
    updated_at: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE upload_job_ingest_sessions SET upload_token_hash = ?, updated_at = ? WHERE job_id = ? AND creator_id = ?",
    )
    .bind(hash_token(upload_token))
    .bind(updated_at)
    .bind(job_id)
    .bind(creator_id)
    .execute(database.sqlite_adapter())
    .await?;
    Ok(())
}

async fn create_creator_upload_ingest_session(
    database: &crate::db::Database,
    creator_id: &str,
    job_id: &str,
    relative_path: &str,
    upload_token: &str,
    mime_type: &str,
    now: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO upload_job_ingest_sessions (
            job_id, creator_id, relative_path, upload_token_hash, status, mime_type,
            bytes_received, created_at, updated_at, completed_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(job_id)
    .bind(creator_id)
    .bind(relative_path)
    .bind(hash_token(upload_token))
    .bind("active")
    .bind(mime_type)
    .bind(0_i64)
    .bind(now)
    .bind(now)
    .bind(Option::<String>::None)
    .execute(database.sqlite_adapter())
    .await?;
    Ok(())
}

async fn update_creator_upload_ingest_progress(
    database: &crate::db::Database,
    creator_id: &str,
    job_id: &str,
    bytes_received: i64,
    updated_at: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE upload_job_ingest_sessions SET bytes_received = ?, updated_at = ? WHERE job_id = ? AND creator_id = ?",
    )
    .bind(bytes_received)
    .bind(updated_at)
    .bind(job_id)
    .bind(creator_id)
    .execute(database.sqlite_adapter())
    .await?;
    sqlx::query(
        "UPDATE upload_jobs SET bytes_received = ?, updated_at = ? WHERE id = ? AND creator_id = ?",
    )
    .bind(bytes_received)
    .bind(updated_at)
    .bind(job_id)
    .bind(creator_id)
    .execute(database.sqlite_adapter())
    .await?;
    Ok(())
}

async fn complete_creator_upload_ingest(
    database: &crate::db::Database,
    creator_id: &str,
    job_id: &str,
    checksum_sha256: &str,
    completed_at: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE upload_job_ingest_sessions SET status = 'completed', updated_at = ?, completed_at = ? WHERE job_id = ? AND creator_id = ?",
    )
    .bind(completed_at)
    .bind(completed_at)
    .bind(job_id)
    .bind(creator_id)
    .execute(database.sqlite_adapter())
    .await?;
    sqlx::query(
        "UPDATE upload_jobs SET status = 'uploaded', checksum_sha256 = ?, completed_at = ?, updated_at = ?, processing_attempt_count = 0, last_processing_error = NULL, last_failed_at = NULL WHERE id = ? AND creator_id = ?",
    )
    .bind(checksum_sha256)
    .bind(completed_at)
    .bind(completed_at)
    .bind(job_id)
    .bind(creator_id)
    .execute(database.sqlite_adapter())
    .await?;
    Ok(())
}

async fn ensure_media_asset_shell_for_ingest(
    database: &crate::db::Database,
    creator_id: &str,
    job: &UploadJob,
    source_relative_path: &str,
) -> AppResult<()> {
    ensure_media_asset_shell(
        database.sqlite_adapter(),
        creator_id,
        job,
        source_relative_path,
    )
    .await
    .map(|_| ())
}
