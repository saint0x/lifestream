use super::*;
use crate::api::control::{
    fetch_current_operational_telemetry, fetch_live_runtime_output_for_session,
    persist_live_runtime_spec, reconcile_live_runtime_output_artifacts,
    record_live_runtime_telemetry, sync_live_runtime_output_artifacts,
};

pub(crate) async fn disconnect_live_ingest(
    State(state): State<SharedState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> AppResult<Json<LiveIngestSession>> {
    let ingest_token = require_ingest_token(&headers)?;
    let session = validate_live_ingest_session_any_status(
        state.db.sqlite_adapter(),
        &session_id,
        &ingest_token,
    )
    .await?;
    if session.status == "connected" || session.status == "stale" {
        close_live_ingest_session(&state, &session, "ended", "disconnected", json!({})).await?;
        return Ok(Json(
            fetch_live_ingest_session_by_id(
                state.db.sqlite_adapter(),
                &session.creator_id,
                &session_id,
            )
            .await?,
        ));
    }
    Ok(Json(session))
}

pub(crate) async fn terminate_live_ingest(
    State(state): State<SharedState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<TerminateLiveIngestRequest>,
) -> AppResult<Json<LiveIngestSession>> {
    let ingest_token = require_ingest_token(&headers)?;
    let session =
        validate_live_ingest_session(state.db.sqlite_adapter(), &session_id, &ingest_token).await?;
    close_live_ingest_session(
        &state,
        &session,
        "terminated",
        "runtime_terminated",
        json!({
            "reason": input
                .reason
                .unwrap_or_else(|| "runtime requested termination".to_string()),
        }),
    )
    .await?;
    Ok(Json(
        fetch_live_ingest_session_by_id(
            state.db.sqlite_adapter(),
            &session.creator_id,
            &session_id,
        )
        .await?,
    ))
}

pub(crate) async fn report_live_runtime(
    State(state): State<SharedState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<UpdateLiveRuntimeStateRequest>,
) -> AppResult<Json<LiveRuntimeOutput>> {
    let ingest_token = require_ingest_token(&headers)?;
    let session = validate_live_ingest_session_any_status(
        state.db.sqlite_adapter(),
        &session_id,
        &ingest_token,
    )
    .await?;
    let previous_output =
        fetch_live_runtime_output_for_session(state.db.sqlite_adapter(), &session.id).await?;
    let output = update_live_runtime_output(state.db.sqlite_adapter(), &session, &input).await?;
    sync_live_runtime_output_artifacts(&state, &session, &output).await?;
    let output = reconcile_live_runtime_output_artifacts(&state, &session)
        .await?
        .unwrap_or(output);
    sync_live_runtime_output_artifacts(&state, &session, &output).await?;
    let session = fetch_live_ingest_session_by_id(
        state.db.sqlite_adapter(),
        &session.creator_id,
        &session.id,
    )
    .await?;
    persist_live_runtime_spec(&state, &session).await?;
    let (cpu_percent, free_disk_gb) =
        fetch_current_operational_telemetry(state.db.sqlite_adapter(), &session.creator_id).await?;
    record_live_runtime_telemetry(
        state.db.sqlite_adapter(),
        &session,
        "runtime_report",
        &output.runtime_state,
        &output.packaging_status,
        &output.archive_status,
        cpu_percent,
        free_disk_gb,
        json!({
            "previousRuntimeState": previous_output.as_ref().map(|item| item.runtime_state.as_str()),
            "previousPackagingStatus": previous_output.as_ref().map(|item| item.packaging_status.as_str()),
            "previousArchiveStatus": previous_output.as_ref().map(|item| item.archive_status.as_str()),
            "manifestRelativePath": output.manifest_relative_path,
            "archiveRelativePath": output.archive_relative_path,
            "lastError": output.last_error,
        }),
    )
    .await?;
    write_live_ingest_event(
        state.db.sqlite_adapter(),
        &session.id,
        &session.creator_id,
        &session.broadcast_id,
        "runtime_reported",
        json!({
            "previousRuntimeState": previous_output.as_ref().map(|item| item.runtime_state.as_str()),
            "previousPackagingStatus": previous_output.as_ref().map(|item| item.packaging_status.as_str()),
            "previousArchiveStatus": previous_output.as_ref().map(|item| item.archive_status.as_str()),
            "runtimeState": output.runtime_state,
            "packagingStatus": output.packaging_status,
            "archiveStatus": output.archive_status,
            "manifestRelativePath": output.manifest_relative_path,
            "archiveRelativePath": output.archive_relative_path,
            "lastError": output.last_error,
        }),
    )
    .await?;
    publish_current_creator_live_state(&state, &session.creator_id).await?;
    Ok(Json(output))
}
