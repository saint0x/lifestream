use super::*;

pub(crate) async fn list_upload_jobs(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<UploadJob>>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(list_creator_upload_jobs(&state.db, creator_id).await?))
}

pub(crate) async fn create_upload_job(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<CreateUploadJobRequest>,
) -> AppResult<Json<UploadJob>> {
    let identity = require_identity(&state.db, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-upload-job-create:{}", identity.user_id),
        30,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    ensure_creator_upload_ingest_enabled_for_jobs(&state.db, creator_id).await?;
    Ok(Json(
        create_creator_upload_job(&state.db, creator_id, input).await?,
    ))
}

pub(crate) async fn update_upload_job(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<UpdateUploadJobRequest>,
) -> AppResult<Json<UploadJob>> {
    let identity = require_identity(&state.db, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-upload-job-update:{}", identity.user_id),
        60,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        update_creator_upload_job(&state.db, creator_id, &id, input).await?,
    ))
}

pub(crate) async fn list_creator_upload_jobs(
    database: &crate::db::Database,
    creator_id: &str,
) -> AppResult<Vec<UploadJob>> {
    if let Ok(pool) = database.try_postgres_adapter() {
        return fetch_postgres_upload_jobs(pool, creator_id).await;
    }
    fetch_upload_jobs(database.try_sqlite_adapter()?, creator_id).await
}

pub(crate) async fn ensure_creator_upload_ingest_enabled_for_jobs(
    database: &crate::db::Database,
    creator_id: &str,
) -> AppResult<()> {
    if let Ok(pool) = database.try_postgres_adapter() {
        return ensure_postgres_creator_upload_ingest_enabled(pool, creator_id).await;
    }
    ensure_creator_upload_ingest_enabled(database.try_sqlite_adapter()?, creator_id).await
}

pub(crate) async fn create_creator_upload_job(
    database: &crate::db::Database,
    creator_id: &str,
    input: CreateUploadJobRequest,
) -> AppResult<UploadJob> {
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
    if let Ok(pool) = database.try_postgres_adapter() {
        sqlx::query(
            r#"
            INSERT INTO upload_jobs (
                id, creator_id, upload_id, series_id, kind, source_type, status, title,
                intended_visibility, bytes_expected, bytes_received, storage_key,
                created_at, updated_at, published_content_id, mime_type, checksum_sha256, completed_at,
                processing_attempt_count, last_processing_error, last_failed_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21)
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
        .execute(pool)
        .await?;

        return fetch_postgres_upload_job_by_id(pool, creator_id, &id).await;
    }

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
    .execute(database.try_sqlite_adapter()?)
    .await?;

    fetch_upload_job_by_id(database.try_sqlite_adapter()?, creator_id, &id).await
}

pub(crate) async fn update_creator_upload_job(
    database: &crate::db::Database,
    creator_id: &str,
    id: &str,
    input: UpdateUploadJobRequest,
) -> AppResult<UploadJob> {
    let current = if let Ok(pool) = database.try_postgres_adapter() {
        fetch_postgres_upload_job_by_id(pool, creator_id, id).await?
    } else {
        fetch_upload_job_by_id(database.try_sqlite_adapter()?, creator_id, id).await?
    };
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
            if !creator_series_exists_for_upload_job(database, creator_id, &series_id).await? {
                return Err(AppError::BadRequest(
                    "seriesId must reference one of the creator's series".to_string(),
                ));
            }
            Some(series_id)
        }
        None => current.series_id,
    };

    if let Ok(pool) = database.try_postgres_adapter() {
        sqlx::query(
            r#"
            UPDATE upload_jobs
            SET title = $1, intended_visibility = $2, series_id = $3, mime_type = $4, updated_at = $5
            WHERE id = $6 AND creator_id = $7
            "#,
        )
        .bind(title)
        .bind(intended_visibility)
        .bind(series_id)
        .bind(mime_type)
        .bind(Utc::now().to_rfc3339())
        .bind(id)
        .bind(creator_id)
        .execute(pool)
        .await?;

        return fetch_postgres_upload_job_by_id(pool, creator_id, id).await;
    }

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
    .bind(id)
    .bind(creator_id)
    .execute(database.try_sqlite_adapter()?)
    .await?;

    fetch_upload_job_by_id(database.try_sqlite_adapter()?, creator_id, id).await
}

async fn fetch_postgres_upload_jobs(
    pool: &sqlx::PgPool,
    creator_id: &str,
) -> AppResult<Vec<UploadJob>> {
    let rows = sqlx::query(
        r#"
        SELECT id, upload_id, series_id, kind, source_type, status, title, intended_visibility,
               bytes_expected::BIGINT AS bytes_expected, bytes_received::BIGINT AS bytes_received,
               storage_key, created_at, updated_at, published_content_id,
               mime_type, checksum_sha256, completed_at, processing_attempt_count::BIGINT AS processing_attempt_count,
               last_processing_error, last_failed_at
        FROM upload_jobs
        WHERE creator_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(postgres_upload_job_from_row).collect())
}

async fn fetch_postgres_upload_job_by_id(
    pool: &sqlx::PgPool,
    creator_id: &str,
    id: &str,
) -> AppResult<UploadJob> {
    let row = sqlx::query(
        r#"
        SELECT id, upload_id, series_id, kind, source_type, status, title, intended_visibility,
               bytes_expected::BIGINT AS bytes_expected, bytes_received::BIGINT AS bytes_received,
               storage_key, created_at, updated_at, published_content_id,
               mime_type, checksum_sha256, completed_at, processing_attempt_count::BIGINT AS processing_attempt_count,
               last_processing_error, last_failed_at
        FROM upload_jobs
        WHERE creator_id = $1 AND id = $2
        "#,
    )
    .bind(creator_id)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(postgres_upload_job_from_row(row))
}

async fn ensure_postgres_creator_upload_ingest_enabled(
    pool: &sqlx::PgPool,
    creator_id: &str,
) -> AppResult<()> {
    let exists = sqlx::query("SELECT 1 FROM creator_profiles WHERE id = $1")
        .bind(creator_id)
        .fetch_optional(pool)
        .await?
        .is_some();
    if !exists {
        return Err(AppError::NotFound);
    }

    let now = Utc::now().to_rfc3339();
    let blocked = sqlx::query(
        r#"
        SELECT 1
        FROM creator_enforcement_actions
        WHERE creator_id = $1
          AND scope = 'uploads'
          AND state = 'active'
          AND (expires_at IS NULL OR expires_at > $2)
        LIMIT 1
        "#,
    )
    .bind(creator_id)
    .bind(&now)
    .fetch_optional(pool)
    .await?
    .is_some();

    if blocked {
        Err(AppError::BadRequest(
            "creator is not currently allowed to ingest or publish uploads".to_string(),
        ))
    } else {
        Ok(())
    }
}

async fn creator_series_exists_for_upload_job(
    database: &crate::db::Database,
    creator_id: &str,
    series_id: &str,
) -> AppResult<bool> {
    if let Ok(pool) = database.try_postgres_adapter() {
        return Ok(sqlx::query(
            "SELECT 1 FROM creator_series_projects WHERE creator_id = $1 AND id = $2",
        )
        .bind(creator_id)
        .bind(series_id)
        .fetch_optional(pool)
        .await?
        .is_some());
    }

    Ok(
        fetch_creator_series_title(database.try_sqlite_adapter()?, creator_id, series_id)
            .await?
            .is_some(),
    )
}

fn postgres_upload_job_from_row(row: sqlx::postgres::PgRow) -> UploadJob {
    UploadJob {
        id: row.get("id"),
        upload_id: row.get("upload_id"),
        series_id: row.get("series_id"),
        kind: row.get("kind"),
        source_type: row.get("source_type"),
        status: row.get("status"),
        title: row.get("title"),
        intended_visibility: row.get("intended_visibility"),
        bytes_expected: row.get("bytes_expected"),
        bytes_received: row.get("bytes_received"),
        storage_key: row.get("storage_key"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        published_content_id: row.get("published_content_id"),
        mime_type: row.get("mime_type"),
        checksum_sha256: row.get("checksum_sha256"),
        completed_at: row.get("completed_at"),
        processing_attempt_count: row.get("processing_attempt_count"),
        last_processing_error: row.get("last_processing_error"),
        last_failed_at: row.get("last_failed_at"),
    }
}
