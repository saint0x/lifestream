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
    let jobs = fetch_upload_jobs(pool, creator_id).await?;
    let ingest_sessions = fetch_upload_ingest_sessions(pool, creator_id).await?;
    let media_assets = fetch_media_assets(pool, creator_id).await?;
    let uploads = fetch_uploads(pool, creator_id).await?;

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
    Ok(CreatorUploadOperationsResponse { summary, records })
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
