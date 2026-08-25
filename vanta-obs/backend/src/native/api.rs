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
    native::{
        protocol::{NativeHelperCommandInput, NativeHelperRecoverInput, NativeHelperStartInput},
        service::NativeServiceError,
        store::NativeStoreError,
    },
};

#[derive(Debug, Error)]
pub enum NativeApiError {
    #[error(transparent)]
    Service(#[from] NativeServiceError),
}

impl IntoResponse for NativeApiError {
    fn into_response(self) -> Response {
        let status = match self {
            NativeApiError::Service(NativeServiceError::Store(NativeStoreError::NotFound)) => {
                StatusCode::NOT_FOUND
            }
            NativeApiError::Service(NativeServiceError::Protocol(_)) => StatusCode::BAD_REQUEST,
            NativeApiError::Service(NativeServiceError::Supervisor(_)) => StatusCode::BAD_GATEWAY,
            NativeApiError::Service(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}

type NativeResult<T> = Result<Json<T>, NativeApiError>;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/native/helpers/sessions",
            get(sessions).post(start_session),
        )
        .route("/api/v1/native/helpers/packages", get(packages))
        .route("/api/v1/native/helpers/sessions/:session_id", get(session))
        .route(
            "/api/v1/native/helpers/sessions/:session_id/command",
            post(command),
        )
        .route(
            "/api/v1/native/helpers/sessions/:session_id/heartbeat",
            post(heartbeat),
        )
        .route(
            "/api/v1/native/helpers/sessions/:session_id/shutdown",
            post(shutdown),
        )
        .route(
            "/api/v1/native/helpers/sessions/:session_id/recover",
            post(recover),
        )
        .route(
            "/api/v1/native/helpers/sessions/:session_id/events",
            get(events),
        )
        .route(
            "/api/v1/native/helpers/sessions/:session_id/logs",
            get(logs),
        )
}

async fn start_session(
    State(state): State<AppState>,
    Json(input): Json<NativeHelperStartInput>,
) -> NativeResult<serde_json::Value> {
    Ok(Json(state.native.start_session(input).await?))
}

async fn sessions(State(state): State<AppState>) -> NativeResult<Vec<serde_json::Value>> {
    Ok(Json(state.native.sessions().await?))
}

async fn packages(
    State(state): State<AppState>,
) -> NativeResult<Vec<crate::native::package::NativePackageState>> {
    Ok(Json(state.native.packages()))
}

async fn session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> NativeResult<serde_json::Value> {
    Ok(Json(state.native.session(&session_id).await?))
}

async fn command(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(input): Json<NativeHelperCommandInput>,
) -> NativeResult<serde_json::Value> {
    Ok(Json(state.native.command(&session_id, input).await?))
}

async fn heartbeat(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> NativeResult<serde_json::Value> {
    Ok(Json(
        state
            .native
            .command(
                &session_id,
                NativeHelperCommandInput {
                    command_kind: "heartbeat".to_string(),
                    payload_json: None,
                },
            )
            .await?,
    ))
}

async fn shutdown(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> NativeResult<serde_json::Value> {
    Ok(Json(
        state
            .native
            .command(
                &session_id,
                NativeHelperCommandInput {
                    command_kind: "shutdown".to_string(),
                    payload_json: None,
                },
            )
            .await?,
    ))
}

async fn events(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> NativeResult<Vec<serde_json::Value>> {
    Ok(Json(state.native.events(&session_id).await?))
}

async fn logs(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> NativeResult<Vec<serde_json::Value>> {
    Ok(Json(state.native.logs(&session_id).await?))
}

async fn recover(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(input): Json<NativeHelperRecoverInput>,
) -> NativeResult<serde_json::Value> {
    Ok(Json(
        state.native.recover_session(&session_id, input).await?,
    ))
}
