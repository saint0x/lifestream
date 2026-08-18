use super::*;
use crate::api::control::{
    describe_live_runtime_artifact_health, persist_live_runtime_spec,
    record_live_runtime_telemetry, repair_live_runtime_output, sync_live_runtime_output_artifacts,
    write_live_ingest_event,
};
use crate::models::{LiveRuntimeRepairReport, RepairLiveRuntimeOutputRequest};

pub(super) async fn repair_runtime_output_authoritatively(
    state: SharedState,
    session: LiveIngestSession,
    actor_user_id: &str,
    actor_scope: &str,
    input: RepairLiveRuntimeOutputRequest,
) -> AppResult<LiveRuntimeRepairReport> {
    let reason = input.reason.trim().to_string();
    let (output, actions) = repair_live_runtime_output(&state.pool, &session, &input).await?;
    let repaired_at = Utc::now().to_rfc3339();

    record_live_runtime_telemetry(
        &state.pool,
        &session,
        "runtime_repair",
        &output.runtime_state,
        &output.packaging_status,
        &output.archive_status,
        None,
        None,
        json!({
            "actorUserId": actor_user_id,
            "actorScope": actor_scope,
            "reason": reason,
            "actions": actions,
        }),
    )
    .await?;
    write_live_ingest_event(
        &state.pool,
        &session.id,
        &session.creator_id,
        &session.broadcast_id,
        "runtime_repaired",
        json!({
            "actorUserId": actor_user_id,
            "actorScope": actor_scope,
            "reason": reason,
            "runtimeState": output.runtime_state,
            "packagingStatus": output.packaging_status,
            "archiveStatus": output.archive_status,
            "manifestRelativePath": output.manifest_relative_path,
            "archiveRelativePath": output.archive_relative_path,
            "lastError": output.last_error,
            "actions": actions,
        }),
    )
    .await?;
    sync_live_runtime_output_artifacts(&state, &session, &output).await?;
    persist_live_runtime_spec(&state, &session).await?;
    publish_creator_live_state(&state, &session.creator_id).await?;

    let mut record = fetch_admin_live_ingest_session_record(&state.pool, &session.id).await?;
    if let Some(output) = record.runtime_output.as_ref() {
        record.artifact_health =
            Some(describe_live_runtime_artifact_health(&state, &record.session, output).await?);
    }

    Ok(LiveRuntimeRepairReport {
        session_id: session.id.clone(),
        creator_id: session.creator_id.clone(),
        broadcast_id: session.broadcast_id.clone(),
        actor_user_id: actor_user_id.to_string(),
        actor_scope: actor_scope.to_string(),
        reason,
        repaired_at,
        actions,
        record,
    })
}
