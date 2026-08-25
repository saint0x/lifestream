use axum::{
    Json, Router,
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use serde::Deserialize;
use serde_json::json;
use thiserror::Error;

use crate::{
    AppState,
    domain::{RenderRequest, TimelinePatch, validate_publish},
    media::MediaError,
    store::{CommentInput, CreateProjectInput, EditorProject, ReviewRequestInput},
};

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("not found")]
    NotFound,
    #[error("forbidden")]
    Forbidden,
    #[error(transparent)]
    Store(#[from] crate::store::StoreError),
    #[error(transparent)]
    Media(#[from] MediaError),
    #[error(transparent)]
    Integration(#[from] crate::integrations::IntegrationError),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::NotFound => StatusCode::NOT_FOUND,
            ApiError::Forbidden => StatusCode::FORBIDDEN,
            ApiError::Store(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Media(MediaError::BadRequest(_)) => StatusCode::BAD_REQUEST,
            ApiError::Media(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Integration(_) => StatusCode::BAD_GATEWAY,
        };
        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}

pub type ApiResult<T> = Result<Json<T>, ApiError>;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route(
            "/api/v1/editor/me/projects",
            get(projects).post(create_project),
        )
        .route(
            "/api/v1/editor/me/projects/:project_id",
            get(project).patch(update_project).delete(delete_project),
        )
        .route("/api/v1/editor/me/projects/:project_id/assets", get(assets))
        .route(
            "/api/v1/editor/me/assets/:asset_id",
            patch(update_asset).delete(delete_asset),
        )
        .route(
            "/api/v1/editor/me/projects/:project_id/assets/upload",
            post(upload_asset),
        )
        .route(
            "/api/v1/editor/me/projects/:project_id/import-media-asset",
            post(import_media_asset),
        )
        .route(
            "/api/v1/editor/me/projects/:project_id/timeline",
            get(timeline).patch(update_timeline),
        )
        .route(
            "/api/v1/editor/me/projects/:project_id/timeline/versions",
            post(create_timeline_version),
        )
        .route(
            "/api/v1/editor/me/projects/:project_id/tracks",
            get(tracks).post(create_track),
        )
        .route(
            "/api/v1/editor/me/tracks/:track_id",
            patch(update_track).delete(delete_track),
        )
        .route(
            "/api/v1/editor/me/projects/:project_id/ad-slots",
            get(ad_slots).post(create_ad_slot),
        )
        .route(
            "/api/v1/editor/me/projects/:project_id/clips",
            get(clips).post(create_clip),
        )
        .route(
            "/api/v1/editor/me/clips/:clip_id",
            patch(update_clip).delete(delete_clip),
        )
        .route(
            "/api/v1/editor/me/ad-slots/:ad_slot_id",
            patch(update_ad_slot).delete(delete_ad_slot),
        )
        .route(
            "/api/v1/editor/me/ad-slots/:ad_slot_id/validate",
            post(validate_ad_slot),
        )
        .route(
            "/api/v1/editor/me/ad-slots/:ad_slot_id/lock",
            post(lock_ad_slot),
        )
        .route(
            "/api/v1/editor/me/projects/:project_id/campaign-requirements",
            get(campaign_requirements).post(create_campaign_requirement),
        )
        .route(
            "/api/v1/editor/me/campaign-requirements/:requirement_id",
            patch(update_campaign_requirement).delete(delete_campaign_requirement),
        )
        .route(
            "/api/v1/editor/me/projects/:project_id/transcript",
            get(transcript).post(create_transcript_segment),
        )
        .route(
            "/api/v1/editor/me/transcript/:segment_id",
            patch(update_transcript_segment).delete(delete_transcript_segment),
        )
        .route(
            "/api/v1/editor/me/projects/:project_id/comments",
            get(comments).post(create_comment),
        )
        .route(
            "/api/v1/editor/me/comments/:comment_id",
            patch(update_comment).delete(delete_comment),
        )
        .route(
            "/api/v1/editor/me/comments/:comment_id/resolve",
            post(resolve_comment),
        )
        .route(
            "/api/v1/editor/me/projects/:project_id/review-requests",
            get(review_requests).post(create_review_request),
        )
        .route(
            "/api/v1/editor/me/review-requests/:review_request_id",
            patch(update_review_request).delete(delete_review_request),
        )
        .route(
            "/api/v1/editor/me/projects/:project_id/render-jobs",
            get(render_jobs).post(create_render_job),
        )
        .route(
            "/api/v1/editor/me/render-jobs/:render_job_id",
            get(render_job).delete(delete_render_job),
        )
        .route(
            "/api/v1/editor/me/exports/:export_id",
            patch(update_export).delete(delete_export),
        )
        .route(
            "/api/v1/editor/me/render-jobs/:render_job_id/cancel",
            post(cancel_render_job),
        )
        .route(
            "/api/v1/editor/me/exports/:export_id/publish",
            post(publish_export),
        )
        .route(
            "/api/v1/editor/me/exports/:export_id/proof-link",
            post(create_proof_link),
        )
        .route(
            "/api/v1/editor/me/exports/:export_id/submit-advertiser-review",
            post(submit_advertiser_review),
        )
        .route(
            "/api/v1/editor/me/projects/:project_id/exports",
            get(exports),
        )
        .route(
            "/api/v1/editor/me/projects/:project_id/proof-links",
            get(proof_links),
        )
        .route(
            "/api/v1/editor/me/proof-links/:proof_link_id",
            patch(update_proof_link).delete(delete_proof_link),
        )
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok", "service": "vanta-video-editor" }))
}

async fn projects(State(state): State<AppState>) -> ApiResult<Vec<EditorProject>> {
    Ok(Json(state.store.projects().await?))
}

async fn create_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateProjectInput>,
) -> ApiResult<EditorProject> {
    require_editor_role(&headers, &["creator_owner", "vanta_operator"])?;
    Ok(Json(state.store.create_project(input).await?))
}

async fn project(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> ApiResult<serde_json::Value> {
    state
        .store
        .project_bundle(&project_id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound)
}

#[derive(Deserialize)]
struct ProjectPatch {
    status: String,
}

async fn update_project(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<ProjectPatch>,
) -> ApiResult<EditorProject> {
    require_editor_role(&headers, &["creator_owner", "vanta_operator"])?;
    Ok(Json(
        state
            .store
            .update_project_status(&project_id, &input.status)
            .await?,
    ))
}

async fn delete_project(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    require_editor_role(&headers, &["creator_owner", "vanta_operator"])?;
    Ok(Json(state.store.delete_project(&project_id).await?))
}

async fn assets(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> ApiResult<Vec<serde_json::Value>> {
    Ok(Json(state.store.assets(&project_id).await?))
}

async fn update_asset(
    State(state): State<AppState>,
    Path(asset_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "creator_collaborator", "vanta_operator"],
    )?;
    Ok(Json(state.store.update_asset(&asset_id, input).await?))
}

async fn delete_asset(
    State(state): State<AppState>,
    Path(asset_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    require_editor_role(&headers, &["creator_owner", "vanta_operator"])?;
    Ok(Json(state.store.delete_asset(&asset_id).await?))
}

async fn upload_asset(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    multipart: Multipart,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "creator_collaborator", "vanta_operator"],
    )?;
    Ok(Json(
        state
            .media
            .upload_asset(&state.store, &project_id, multipart)
            .await?,
    ))
}

#[derive(Deserialize)]
struct ImportAssetRequest {
    media_asset_id: String,
    role: String,
}

async fn import_media_asset(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<ImportAssetRequest>,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "creator_collaborator", "vanta_operator"],
    )?;
    Ok(Json(
        state
            .store
            .import_asset(&project_id, &input.media_asset_id, &input.role)
            .await?,
    ))
}

async fn timeline(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> ApiResult<serde_json::Value> {
    state
        .store
        .timeline_bundle(&project_id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound)
}

async fn update_timeline(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<TimelinePatch>,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "creator_collaborator", "vanta_operator"],
    )?;
    let timeline = state.store.apply_timeline_patch(&project_id, input).await?;
    Ok(Json(timeline))
}

async fn create_timeline_version(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "creator_collaborator", "vanta_operator"],
    )?;
    Ok(Json(
        state
            .store
            .create_timeline_version(&project_id, "Manual save")
            .await?,
    ))
}

async fn tracks(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> ApiResult<Vec<serde_json::Value>> {
    Ok(Json(state.store.tracks(&project_id).await?))
}

async fn create_track(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "creator_collaborator", "vanta_operator"],
    )?;
    Ok(Json(state.store.create_track(&project_id, input).await?))
}

async fn update_track(
    State(state): State<AppState>,
    Path(track_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "creator_collaborator", "vanta_operator"],
    )?;
    Ok(Json(state.store.update_track(&track_id, input).await?))
}

async fn delete_track(
    State(state): State<AppState>,
    Path(track_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    require_editor_role(&headers, &["creator_owner", "vanta_operator"])?;
    Ok(Json(state.store.delete_track(&track_id).await?))
}

async fn ad_slots(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> ApiResult<Vec<serde_json::Value>> {
    Ok(Json(state.store.ad_slots(&project_id).await?))
}

async fn create_ad_slot(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "vanta_operator", "vanta_ad_ops"],
    )?;
    Ok(Json(state.store.create_ad_slot(&project_id, input).await?))
}

async fn create_clip(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "creator_collaborator", "vanta_operator"],
    )?;
    Ok(Json(state.store.create_clip(&project_id, input).await?))
}

async fn clips(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> ApiResult<Vec<serde_json::Value>> {
    Ok(Json(state.store.clips(&project_id).await?))
}

async fn update_clip(
    State(state): State<AppState>,
    Path(clip_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "creator_collaborator", "vanta_operator"],
    )?;
    Ok(Json(state.store.update_clip(&clip_id, input).await?))
}

async fn delete_clip(
    State(state): State<AppState>,
    Path(clip_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "creator_collaborator", "vanta_operator"],
    )?;
    Ok(Json(state.store.delete_clip(&clip_id).await?))
}

async fn update_ad_slot(
    State(state): State<AppState>,
    Path(ad_slot_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "vanta_operator", "vanta_ad_ops"],
    )?;
    Ok(Json(state.store.update_ad_slot(&ad_slot_id, input).await?))
}

async fn delete_ad_slot(
    State(state): State<AppState>,
    Path(ad_slot_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "vanta_operator", "vanta_ad_ops"],
    )?;
    Ok(Json(state.store.delete_ad_slot(&ad_slot_id).await?))
}

async fn validate_ad_slot(
    State(state): State<AppState>,
    Path(ad_slot_id): Path<String>,
) -> ApiResult<serde_json::Value> {
    Ok(Json(state.store.validate_ad_slot(&ad_slot_id).await?))
}

async fn lock_ad_slot(
    State(state): State<AppState>,
    Path(ad_slot_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    require_editor_role(&headers, &["vanta_operator", "vanta_ad_ops"])?;
    let validation = state.store.validate_ad_slot(&ad_slot_id).await?;
    if !validation["valid"].as_bool().unwrap_or(false) {
        return Err(ApiError::BadRequest(
            "ad slot is not valid for locking".to_string(),
        ));
    }
    Ok(Json(state.store.lock_ad_slot(&ad_slot_id).await?))
}

async fn campaign_requirements(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> ApiResult<Vec<serde_json::Value>> {
    Ok(Json(state.store.requirements(&project_id).await?))
}

async fn create_campaign_requirement(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    require_editor_role(&headers, &["vanta_operator", "vanta_ad_ops"])?;
    Ok(Json(
        state
            .store
            .create_campaign_requirement(&project_id, input)
            .await?,
    ))
}

async fn update_campaign_requirement(
    State(state): State<AppState>,
    Path(requirement_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    require_editor_role(&headers, &["vanta_operator", "vanta_ad_ops"])?;
    Ok(Json(
        state
            .store
            .update_campaign_requirement(&requirement_id, input)
            .await?,
    ))
}

async fn delete_campaign_requirement(
    State(state): State<AppState>,
    Path(requirement_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    require_editor_role(&headers, &["vanta_operator", "vanta_ad_ops"])?;
    Ok(Json(
        state
            .store
            .delete_campaign_requirement(&requirement_id)
            .await?,
    ))
}

async fn transcript(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> ApiResult<Vec<serde_json::Value>> {
    Ok(Json(state.store.transcript(&project_id).await?))
}

async fn create_transcript_segment(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "creator_collaborator", "vanta_operator"],
    )?;
    Ok(Json(
        state
            .store
            .create_transcript_segment(&project_id, input)
            .await?,
    ))
}

async fn update_transcript_segment(
    State(state): State<AppState>,
    Path(segment_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "creator_collaborator", "vanta_operator"],
    )?;
    Ok(Json(
        state
            .store
            .update_transcript_segment(&segment_id, input)
            .await?,
    ))
}

async fn delete_transcript_segment(
    State(state): State<AppState>,
    Path(segment_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    require_editor_role(&headers, &["creator_owner", "vanta_operator"])?;
    Ok(Json(
        state.store.delete_transcript_segment(&segment_id).await?,
    ))
}

async fn comments(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> ApiResult<Vec<serde_json::Value>> {
    Ok(Json(state.store.comments(&project_id).await?))
}

async fn create_comment(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<CommentInput>,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &[
            "creator_owner",
            "creator_collaborator",
            "vanta_operator",
            "vanta_ad_ops",
        ],
    )?;
    Ok(Json(state.store.create_comment(&project_id, input).await?))
}

async fn update_comment(
    State(state): State<AppState>,
    Path(comment_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &[
            "creator_owner",
            "creator_collaborator",
            "vanta_operator",
            "vanta_ad_ops",
        ],
    )?;
    Ok(Json(state.store.update_comment(&comment_id, input).await?))
}

async fn delete_comment(
    State(state): State<AppState>,
    Path(comment_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    require_editor_role(&headers, &["creator_owner", "vanta_operator"])?;
    Ok(Json(state.store.delete_comment(&comment_id).await?))
}

async fn resolve_comment(
    State(state): State<AppState>,
    Path(comment_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &[
            "creator_owner",
            "creator_collaborator",
            "vanta_operator",
            "vanta_ad_ops",
        ],
    )?;
    Ok(Json(state.store.resolve_comment(&comment_id).await?))
}

async fn create_review_request(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<ReviewRequestInput>,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "vanta_operator", "vanta_ad_ops"],
    )?;
    Ok(Json(
        state
            .store
            .create_review_request(&project_id, input)
            .await?,
    ))
}

async fn review_requests(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> ApiResult<Vec<serde_json::Value>> {
    Ok(Json(state.store.review_requests(&project_id).await?))
}

async fn update_review_request(
    State(state): State<AppState>,
    Path(review_request_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "vanta_operator", "vanta_ad_ops"],
    )?;
    Ok(Json(
        state
            .store
            .update_review_request(&review_request_id, input)
            .await?,
    ))
}

async fn delete_review_request(
    State(state): State<AppState>,
    Path(review_request_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "vanta_operator", "vanta_ad_ops"],
    )?;
    Ok(Json(
        state
            .store
            .delete_review_request(&review_request_id)
            .await?,
    ))
}

async fn create_render_job(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<RenderRequest>,
) -> ApiResult<serde_json::Value> {
    require_editor_role(&headers, &["creator_owner", "vanta_operator"])?;
    let bundle = state
        .store
        .timeline_bundle(&project_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let validation = validate_publish(&bundle);
    let job = state
        .store
        .create_render_job(&project_id, input, validation)
        .await?;
    let job_id = job["id"].as_str().unwrap_or_default();
    match state
        .media
        .package_hls(&state.store, &project_id, job_id)
        .await
    {
        Ok(packaged) => Ok(Json(packaged)),
        Err(MediaError::BadRequest(_)) => Ok(Json(job)),
        Err(error) => Err(error.into()),
    }
}

async fn render_jobs(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> ApiResult<Vec<serde_json::Value>> {
    Ok(Json(state.store.render_jobs(&project_id).await?))
}

async fn render_job(
    State(state): State<AppState>,
    Path(render_job_id): Path<String>,
) -> ApiResult<serde_json::Value> {
    state
        .store
        .render_job(&render_job_id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound)
}

async fn delete_render_job(
    State(state): State<AppState>,
    Path(render_job_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    require_editor_role(&headers, &["creator_owner", "vanta_operator"])?;
    Ok(Json(state.store.delete_render_job(&render_job_id).await?))
}

async fn cancel_render_job(
    State(state): State<AppState>,
    Path(render_job_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    require_editor_role(&headers, &["creator_owner", "vanta_operator"])?;
    Ok(Json(state.store.cancel_render_job(&render_job_id).await?))
}

async fn publish_export(
    State(state): State<AppState>,
    Path(export_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    require_editor_role(&headers, &["creator_owner", "vanta_operator"])?;
    let export = state.store.publish_export(&export_id).await?;
    let project_id = export["project_id"].as_str().unwrap_or_default();
    let project_bundle = state
        .store
        .project_bundle(project_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let render_job = state
        .store
        .render_job(export["render_job_id"].as_str().unwrap_or_default())
        .await?
        .ok_or(ApiError::NotFound)?;
    let pipeline = state
        .integrations
        .publish_export(&export, &project_bundle, &render_job)
        .await?;
    Ok(Json(json!({
        "export": export,
        "media_pipeline": pipeline
    })))
}

async fn update_export(
    State(state): State<AppState>,
    Path(export_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    require_editor_role(&headers, &["creator_owner", "vanta_operator"])?;
    Ok(Json(state.store.update_export(&export_id, input).await?))
}

async fn delete_export(
    State(state): State<AppState>,
    Path(export_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    require_editor_role(&headers, &["creator_owner", "vanta_operator"])?;
    Ok(Json(state.store.delete_export(&export_id).await?))
}

async fn create_proof_link(
    State(state): State<AppState>,
    Path(export_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "vanta_operator", "vanta_ad_ops"],
    )?;
    Ok(Json(state.store.create_proof_link(&export_id).await?))
}

async fn submit_advertiser_review(
    State(state): State<AppState>,
    Path(export_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "vanta_operator", "vanta_ad_ops"],
    )?;
    let submission = state.store.submit_advertiser_review(&export_id).await?;
    let export = state
        .store
        .export(&export_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let project_bundle = state
        .store
        .project_bundle(export["project_id"].as_str().unwrap_or_default())
        .await?
        .ok_or(ApiError::NotFound)?;
    let ad_hub = state
        .integrations
        .submit_advertiser_review(&submission, &export, &project_bundle)
        .await?;
    Ok(Json(json!({
        "review_request": submission["review_request"],
        "proof_link": submission["proof_link"],
        "external_room": ad_hub
    })))
}

async fn exports(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> ApiResult<Vec<serde_json::Value>> {
    Ok(Json(state.store.exports(&project_id).await?))
}

async fn proof_links(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> ApiResult<Vec<serde_json::Value>> {
    Ok(Json(state.store.proof_links(&project_id).await?))
}

async fn update_proof_link(
    State(state): State<AppState>,
    Path(proof_link_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "vanta_operator", "vanta_ad_ops"],
    )?;
    Ok(Json(
        state.store.update_proof_link(&proof_link_id, input).await?,
    ))
}

async fn delete_proof_link(
    State(state): State<AppState>,
    Path(proof_link_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    require_editor_role(
        &headers,
        &["creator_owner", "vanta_operator", "vanta_ad_ops"],
    )?;
    Ok(Json(state.store.delete_proof_link(&proof_link_id).await?))
}

fn require_editor_role(headers: &HeaderMap, allowed: &[&str]) -> Result<(), ApiError> {
    let role = headers
        .get("x-vanta-role")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if allowed.contains(&role) {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}
