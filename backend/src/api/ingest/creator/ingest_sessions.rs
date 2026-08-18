use super::*;
use crate::api::control::{
    describe_live_runtime_artifact_health, reconcile_live_runtime_output_artifacts,
};
use crate::models::{LiveRuntimeRepairReport, RepairLiveRuntimeOutputRequest};

pub(crate) async fn get_creator_live_ingest_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Option<LiveIngestSession>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_active_live_ingest_session(&state.pool, creator_id).await?,
    ))
}

pub(crate) async fn get_creator_live_ingest_session_by_id(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<AdminLiveIngestSessionRecord>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let session = fetch_live_ingest_session_by_id(&state.pool, creator_id, &session_id).await?;
    let _ = reconcile_live_runtime_output_artifacts(&state, &session).await?;
    let mut record =
        fetch_creator_live_ingest_session_record(&state.pool, creator_id, &session_id).await?;
    if let Some(output) = record.runtime_output.as_ref() {
        record.artifact_health =
            Some(describe_live_runtime_artifact_health(&state, &record.session, output).await?);
    }
    Ok(Json(record))
}

pub(crate) async fn list_creator_live_ingest_events(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<Vec<LiveIngestEvent>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    fetch_live_ingest_session_by_id(&state.pool, creator_id, &session_id).await?;
    Ok(Json(
        fetch_live_ingest_events_for_session(&state.pool, &session_id, 50).await?,
    ))
}

pub(crate) async fn reconcile_creator_live_ingest_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<LiveIngestReconciliationReport>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    fetch_live_ingest_session_by_id_unreconciled(&state.pool, creator_id, &session_id).await?;
    Ok(Json(
        reconcile_single_live_ingest_session(state, &session_id).await?,
    ))
}

pub(crate) async fn terminate_creator_live_ingest(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Json(input): Json<TerminateLiveIngestRequest>,
) -> AppResult<Json<LiveIngestSession>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-live-ingest-terminate:{}", identity.user_id),
        20,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    let session = fetch_live_ingest_session_by_id(&state.pool, creator_id, &session_id).await?;
    if session.status != "connected" {
        return Err(AppError::BadRequest(
            "only connected live ingest sessions can be terminated".to_string(),
        ));
    }

    close_live_ingest_session(
        &state,
        &session,
        "terminated",
        "creator_terminated",
        json!({
            "reason": input.reason.unwrap_or_else(|| "creator requested termination".to_string()),
            "actorUserId": identity.user_id,
        }),
    )
    .await?;

    publish_creator_live_state(&state, creator_id).await?;
    Ok(Json(
        fetch_live_ingest_session_by_id(&state.pool, creator_id, &session_id).await?,
    ))
}

pub(crate) async fn repair_creator_live_runtime_output(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Json(input): Json<RepairLiveRuntimeOutputRequest>,
) -> AppResult<Json<LiveRuntimeRepairReport>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-live-runtime-repair:{}", identity.user_id),
        20,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    let session = fetch_live_ingest_session_by_id(&state.pool, creator_id, &session_id).await?;
    Ok(Json(
        super::super::repair::repair_runtime_output_authoritatively(
            state,
            session,
            &identity.user_id,
            "creator",
            input,
        )
        .await?,
    ))
}
