use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::json;
use thiserror::Error;

use crate::{
    AppState,
    media::{
        domain::{
            CaptureStartInput, EncodeStartInput, RuntimeProgramFrameInput, RuntimeSourceFrameInput,
            RuntimeSourcePlayoutInput, SourceAudioIngestInput,
        },
        service::MediaServiceError,
        source::SourceMediaError,
        store::MediaStoreError,
    },
    native::service::NativeServiceError,
};

#[derive(Debug, Error)]
pub enum MediaApiError {
    #[error(transparent)]
    Service(#[from] MediaServiceError),
}

impl IntoResponse for MediaApiError {
    fn into_response(self) -> Response {
        let status = match self {
            MediaApiError::Service(MediaServiceError::Store(MediaStoreError::NotFound)) => {
                StatusCode::NOT_FOUND
            }
            MediaApiError::Service(MediaServiceError::Invalid { .. }) => StatusCode::BAD_REQUEST,
            MediaApiError::Service(MediaServiceError::Native(NativeServiceError::Supervisor(
                _,
            ))) => StatusCode::BAD_GATEWAY,
            MediaApiError::Service(MediaServiceError::Native(NativeServiceError::Protocol(_))) => {
                StatusCode::BAD_REQUEST
            }
            MediaApiError::Service(MediaServiceError::Ffmpeg(_)) => StatusCode::BAD_GATEWAY,
            MediaApiError::Service(MediaServiceError::NativeCapture(_)) => StatusCode::BAD_GATEWAY,
            MediaApiError::Service(MediaServiceError::SourceMedia(SourceMediaError::Invalid(
                _,
            ))) => StatusCode::BAD_REQUEST,
            MediaApiError::Service(MediaServiceError::SourceMedia(_)) => StatusCode::BAD_GATEWAY,
            MediaApiError::Service(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}

type MediaResult<T> = Result<Json<T>, MediaApiError>;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/media/capabilities", get(capabilities))
        .route("/api/v1/media/capture/devices", get(capture_devices))
        .route(
            "/api/v1/media/capture/sessions",
            get(capture_sessions).post(start_capture),
        )
        .route(
            "/api/v1/media/capture/sessions/:session_id/stop",
            post(stop_capture),
        )
        .route(
            "/api/v1/media/capture/sessions/:session_id/reconcile",
            post(reconcile_capture),
        )
        .route(
            "/api/v1/media/capture/sessions/:session_id/preview-frame",
            post(capture_preview_frame),
        )
        .route(
            "/api/v1/media/capture/sessions/:session_id/runtime-frame",
            post(ingest_runtime_program_frame),
        )
        .route(
            "/api/v1/media/capture/sessions/:session_id/source-frame",
            post(ingest_runtime_source_frame),
        )
        .route(
            "/api/v1/media/capture/sessions/:session_id/source-playout",
            post(create_runtime_source_playout),
        )
        .route(
            "/api/v1/media/capture/sessions/:session_id/segment",
            post(capture_segment),
        )
        .route(
            "/api/v1/media/capture/sessions/:session_id/frames",
            get(capture_frames),
        )
        .route(
            "/api/v1/media/capture/sessions/:session_id/artifacts",
            get(capture_artifacts),
        )
        .route(
            "/api/v1/media/encode/jobs",
            get(encode_jobs).post(start_encode),
        )
        .route("/api/v1/media/sources/audio", post(ingest_source_audio))
        .route(
            "/api/v1/media/sources/:source_id/artifacts",
            get(source_artifacts),
        )
        .route("/api/v1/media/encode/jobs/:job_id/stop", post(stop_encode))
        .route(
            "/api/v1/media/encode/jobs/:job_id/render",
            post(render_encode),
        )
        .route(
            "/api/v1/media/encode/jobs/:job_id/package",
            post(package_encode),
        )
        .route("/api/v1/media/packages", get(packages))
}

async fn capabilities(State(state): State<AppState>) -> MediaResult<serde_json::Value> {
    Ok(Json(state.media.capabilities().await?))
}

async fn capture_devices(State(state): State<AppState>) -> MediaResult<serde_json::Value> {
    Ok(Json(state.media.capture_inventory().await?))
}

async fn start_capture(
    State(state): State<AppState>,
    Json(input): Json<CaptureStartInput>,
) -> MediaResult<serde_json::Value> {
    Ok(Json(state.media.start_capture(input).await?))
}

async fn stop_capture(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> MediaResult<serde_json::Value> {
    Ok(Json(state.media.stop_capture(&session_id).await?))
}

async fn reconcile_capture(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> MediaResult<serde_json::Value> {
    Ok(Json(state.media.reconcile_capture(&session_id).await?))
}

async fn capture_preview_frame(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> MediaResult<serde_json::Value> {
    Ok(Json(state.media.capture_preview_frame(&session_id).await?))
}

async fn ingest_runtime_program_frame(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(input): Json<RuntimeProgramFrameInput>,
) -> MediaResult<serde_json::Value> {
    Ok(Json(
        state
            .media
            .ingest_runtime_program_frame(&session_id, input)
            .await?,
    ))
}

async fn ingest_runtime_source_frame(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(input): Json<RuntimeSourceFrameInput>,
) -> MediaResult<serde_json::Value> {
    Ok(Json(
        state
            .media
            .ingest_runtime_source_frame(&session_id, input)
            .await?,
    ))
}

async fn create_runtime_source_playout(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(input): Json<RuntimeSourcePlayoutInput>,
) -> MediaResult<serde_json::Value> {
    Ok(Json(
        state
            .media
            .create_runtime_source_playout(&session_id, input)
            .await?,
    ))
}

async fn capture_segment(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> MediaResult<serde_json::Value> {
    Ok(Json(state.media.capture_segment(&session_id).await?))
}

async fn capture_sessions(State(state): State<AppState>) -> MediaResult<Vec<serde_json::Value>> {
    Ok(Json(state.media.capture_sessions().await?))
}

async fn capture_frames(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> MediaResult<Vec<serde_json::Value>> {
    Ok(Json(state.media.capture_frames(&session_id).await?))
}

async fn capture_artifacts(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> MediaResult<Vec<serde_json::Value>> {
    Ok(Json(state.media.capture_artifacts(&session_id).await?))
}

async fn ingest_source_audio(
    State(state): State<AppState>,
    Json(input): Json<SourceAudioIngestInput>,
) -> MediaResult<serde_json::Value> {
    Ok(Json(state.media.ingest_source_audio(input).await?))
}

async fn source_artifacts(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> MediaResult<Vec<serde_json::Value>> {
    Ok(Json(state.media.source_artifacts(&source_id).await?))
}

async fn start_encode(
    State(state): State<AppState>,
    Json(input): Json<EncodeStartInput>,
) -> MediaResult<serde_json::Value> {
    Ok(Json(state.media.start_encode(input).await?))
}

async fn stop_encode(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> MediaResult<serde_json::Value> {
    Ok(Json(state.media.stop_encode(&job_id).await?))
}

async fn render_encode(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> MediaResult<serde_json::Value> {
    Ok(Json(state.media.render_encode(&job_id).await?))
}

async fn package_encode(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> MediaResult<serde_json::Value> {
    Ok(Json(state.media.package_encode(&job_id).await?))
}

async fn encode_jobs(State(state): State<AppState>) -> MediaResult<Vec<serde_json::Value>> {
    Ok(Json(state.media.encode_jobs().await?))
}

async fn packages(State(state): State<AppState>) -> MediaResult<Vec<serde_json::Value>> {
    Ok(Json(state.media.packages().await?))
}
