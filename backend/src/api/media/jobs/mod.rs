use super::*;

mod ingest;
mod lifecycle;
mod publish;

pub(crate) use ingest::{
    append_upload_chunk, complete_upload_ingest, get_creator_upload_job, get_upload_ingest_session,
    start_upload_ingest_session,
};
pub(crate) use lifecycle::{
    create_creator_upload_job, create_upload_job, list_creator_upload_jobs, list_upload_jobs,
    update_creator_upload_job, update_upload_job,
};
pub(crate) use publish::{
    get_creator_media_asset_for_upload_job, get_media_asset_for_upload_job,
    list_creator_media_assets, list_media_assets, publish_upload_job, retry_upload_job_processing,
};

pub(crate) fn routes() -> Router<SharedState> {
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
