use super::*;

pub(crate) fn summarize_creator_content(
    uploads: &[Upload],
    filtered_count: i64,
) -> CreatorContentSummary {
    CreatorContentSummary {
        total_uploads: uploads.len() as i64,
        published_uploads: uploads
            .iter()
            .filter(|upload| upload.status == "published")
            .count() as i64,
        scheduled_uploads: uploads
            .iter()
            .filter(|upload| upload.status == "scheduled")
            .count() as i64,
        processing_uploads: uploads
            .iter()
            .filter(|upload| upload.status == "processing")
            .count() as i64,
        draft_uploads: uploads
            .iter()
            .filter(|upload| upload.status == "draft")
            .count() as i64,
        archived_uploads: uploads
            .iter()
            .filter(|upload| upload.status == "archived")
            .count() as i64,
        total_views: uploads.iter().map(|upload| upload.views).sum(),
        total_watch_hours: uploads.iter().map(|upload| upload.watch_hours).sum(),
        total_storage_bytes: uploads.iter().map(|upload| upload.size_bytes).sum(),
        filtered_count,
    }
}

pub(crate) async fn fetch_creator_upload_operations_response(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorUploadOperationsResponse> {
    let jobs: Vec<UploadJob> = fetch_upload_jobs(pool, creator_id).await?;
    let ingest_sessions: Vec<UploadIngestSession> =
        fetch_upload_ingest_sessions(pool, creator_id).await?;
    let media_assets: Vec<MediaAsset> = fetch_media_assets(pool, creator_id).await?;
    let uploads = fetch_uploads(pool, creator_id).await?;

    Ok(build_creator_upload_operations_response(
        jobs,
        ingest_sessions,
        media_assets,
        uploads,
    ))
}

pub(crate) async fn fetch_creator_upload_operations_response_for_database(
    database: &crate::db::Database,
    creator_id: &str,
) -> AppResult<CreatorUploadOperationsResponse> {
    if let Ok(pool) = database.try_postgres_adapter() {
        let (jobs, ingest_sessions, media_assets, uploads) = tokio::try_join!(
            list_creator_upload_jobs(database, creator_id),
            fetch_postgres_upload_ingest_sessions(pool, creator_id),
            fetch_media_assets_for_database(database, creator_id),
            fetch_uploads_for_database(database, creator_id),
        )?;
        return Ok(build_creator_upload_operations_response(
            jobs,
            ingest_sessions,
            media_assets,
            uploads,
        ));
    }

    fetch_creator_upload_operations_response(database.try_sqlite_adapter()?, creator_id).await
}

async fn fetch_postgres_upload_ingest_sessions(
    pool: &sqlx::PgPool,
    creator_id: &str,
) -> AppResult<Vec<UploadIngestSession>> {
    let rows = sqlx::query(
        r#"
        SELECT job_id, relative_path, status, mime_type,
               bytes_received::BIGINT AS bytes_received, created_at, updated_at, completed_at
        FROM upload_job_ingest_sessions
        WHERE creator_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| UploadIngestSession {
            job_id: row.get("job_id"),
            relative_path: row.get("relative_path"),
            status: row.get("status"),
            mime_type: row.get("mime_type"),
            bytes_received: row.get("bytes_received"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            completed_at: row.get("completed_at"),
        })
        .collect())
}

pub(crate) async fn fetch_creator_upload_operations_summary(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorUploadOperationsSummary> {
    let jobs_row = sqlx::query(
        r#"
        SELECT
            COUNT(*) AS total_jobs,
            SUM(CASE WHEN status = 'created' THEN 1 ELSE 0 END) AS created_jobs,
            SUM(CASE WHEN status = 'uploaded' THEN 1 ELSE 0 END) AS uploaded_jobs,
            SUM(CASE WHEN status = 'processing' THEN 1 ELSE 0 END) AS processing_jobs,
            SUM(CASE WHEN status = 'ready' THEN 1 ELSE 0 END) AS ready_jobs,
            SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) AS failed_jobs,
            SUM(CASE WHEN status = 'published' THEN 1 ELSE 0 END) AS published_jobs,
            COALESCE(SUM(bytes_expected), 0) AS total_bytes_expected,
            COALESCE(SUM(bytes_received), 0) AS total_bytes_received
        FROM upload_jobs
        WHERE creator_id = ?
        "#,
    )
    .bind(creator_id)
    .fetch_one(pool)
    .await?;

    let ingest_row = sqlx::query(
        r#"
        SELECT
            SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) AS completed_ingest_sessions,
            SUM(CASE WHEN status != 'completed' THEN 1 ELSE 0 END) AS active_ingest_sessions
        FROM upload_job_ingest_sessions
        WHERE creator_id = ?
        "#,
    )
    .bind(creator_id)
    .fetch_one(pool)
    .await?;

    let assets_row = sqlx::query(
        r#"
        SELECT
            SUM(CASE WHEN status = 'ready' THEN 1 ELSE 0 END) AS ready_assets,
            SUM(CASE WHEN status = 'processing' THEN 1 ELSE 0 END) AS processing_assets,
            SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) AS failed_assets,
            SUM(CASE WHEN status = 'published' THEN 1 ELSE 0 END) AS published_assets,
            COALESCE(SUM(file_size_bytes), 0) AS total_asset_bytes
        FROM media_assets
        WHERE creator_id = ?
        "#,
    )
    .bind(creator_id)
    .fetch_one(pool)
    .await?;

    Ok(CreatorUploadOperationsSummary {
        total_jobs: jobs_row.get("total_jobs"),
        created_jobs: jobs_row.get::<Option<i64>, _>("created_jobs").unwrap_or(0),
        uploaded_jobs: jobs_row.get::<Option<i64>, _>("uploaded_jobs").unwrap_or(0),
        processing_jobs: jobs_row
            .get::<Option<i64>, _>("processing_jobs")
            .unwrap_or(0),
        ready_jobs: jobs_row.get::<Option<i64>, _>("ready_jobs").unwrap_or(0),
        failed_jobs: jobs_row.get::<Option<i64>, _>("failed_jobs").unwrap_or(0),
        published_jobs: jobs_row
            .get::<Option<i64>, _>("published_jobs")
            .unwrap_or(0),
        active_ingest_sessions: ingest_row
            .get::<Option<i64>, _>("active_ingest_sessions")
            .unwrap_or(0),
        completed_ingest_sessions: ingest_row
            .get::<Option<i64>, _>("completed_ingest_sessions")
            .unwrap_or(0),
        ready_assets: assets_row
            .get::<Option<i64>, _>("ready_assets")
            .unwrap_or(0),
        processing_assets: assets_row
            .get::<Option<i64>, _>("processing_assets")
            .unwrap_or(0),
        failed_assets: assets_row
            .get::<Option<i64>, _>("failed_assets")
            .unwrap_or(0),
        published_assets: assets_row
            .get::<Option<i64>, _>("published_assets")
            .unwrap_or(0),
        total_bytes_expected: jobs_row.get("total_bytes_expected"),
        total_bytes_received: jobs_row.get("total_bytes_received"),
        total_asset_bytes: assets_row.get("total_asset_bytes"),
    })
}

pub(crate) fn build_creator_upload_operations_response(
    jobs: Vec<UploadJob>,
    ingest_sessions: Vec<UploadIngestSession>,
    media_assets: Vec<MediaAsset>,
    uploads: Vec<Upload>,
) -> CreatorUploadOperationsResponse {
    let session_by_job: HashMap<String, UploadIngestSession> = ingest_sessions
        .iter()
        .cloned()
        .map(|session| (session.job_id.clone(), session))
        .collect();
    let asset_by_job: HashMap<String, MediaAsset> = media_assets
        .iter()
        .cloned()
        .map(|asset| (asset.upload_job_id.clone(), asset))
        .collect();
    let upload_by_id: HashMap<String, Upload> = uploads
        .into_iter()
        .map(|upload| (upload.id.clone(), upload))
        .collect();

    let records = jobs
        .iter()
        .cloned()
        .map(|job| {
            let media_asset = asset_by_job.get(&job.id).cloned();
            let published_upload_id = job
                .upload_id
                .clone()
                .or_else(|| {
                    media_asset
                        .as_ref()
                        .and_then(|asset| asset.upload_id.clone())
                })
                .or_else(|| job.published_content_id.clone())
                .or_else(|| {
                    media_asset
                        .as_ref()
                        .and_then(|asset| asset.published_content_id.clone())
                });
            CreatorUploadOperationRecord {
                ingest_session: session_by_job.get(&job.id).cloned(),
                media_asset,
                published_upload: published_upload_id
                    .as_ref()
                    .and_then(|upload_id| upload_by_id.get(upload_id).cloned()),
                upload_job: job,
            }
        })
        .collect::<Vec<_>>();

    let summary = summarize_creator_upload_operations(&records);
    CreatorUploadOperationsResponse { summary, records }
}

pub(crate) fn summarize_creator_upload_operations(
    records: &[CreatorUploadOperationRecord],
) -> CreatorUploadOperationsSummary {
    let mut summary = CreatorUploadOperationsSummary {
        total_jobs: records.len() as i64,
        created_jobs: 0,
        uploaded_jobs: 0,
        processing_jobs: 0,
        ready_jobs: 0,
        failed_jobs: 0,
        published_jobs: 0,
        active_ingest_sessions: 0,
        completed_ingest_sessions: 0,
        ready_assets: 0,
        processing_assets: 0,
        failed_assets: 0,
        published_assets: 0,
        total_bytes_expected: 0,
        total_bytes_received: 0,
        total_asset_bytes: 0,
    };

    for record in records {
        summary.total_bytes_expected += record.upload_job.bytes_expected;
        summary.total_bytes_received += record.upload_job.bytes_received;

        match record.upload_job.status.as_str() {
            "created" => summary.created_jobs += 1,
            "uploaded" => summary.uploaded_jobs += 1,
            "processing" => summary.processing_jobs += 1,
            "ready" => summary.ready_jobs += 1,
            "failed" => summary.failed_jobs += 1,
            "published" => summary.published_jobs += 1,
            _ => {}
        }

        if let Some(session) = record.ingest_session.as_ref() {
            if session.status == "completed" {
                summary.completed_ingest_sessions += 1;
            } else {
                summary.active_ingest_sessions += 1;
            }
        }

        if let Some(asset) = record.media_asset.as_ref() {
            summary.total_asset_bytes += asset.file_size_bytes;
            match asset.status.as_str() {
                "ready" => summary.ready_assets += 1,
                "processing" => summary.processing_assets += 1,
                "failed" => summary.failed_assets += 1,
                "published" => summary.published_assets += 1,
                _ => {}
            }
        }
    }

    summary
}
