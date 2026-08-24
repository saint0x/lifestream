use super::inspect::{RuntimeArtifactInspection, inspect_live_runtime_output_artifacts};
use super::*;

pub(crate) async fn reconcile_live_runtime_output_artifacts_background(
    state: SharedState,
) -> AppResult<usize> {
    let rows = sqlx::query(
        r#"
        SELECT s.id
        FROM live_ingest_sessions s
        JOIN live_runtime_outputs o ON o.session_id = s.id
        WHERE (
            o.packaging_status IN ('ready', 'complete')
            OR o.archive_status IN ('finalizing', 'complete')
        )
        ORDER BY o.last_runtime_event_at ASC
        "#,
    )
    .fetch_all(state.db.sqlite_adapter())
    .await?;

    let mut reconciled = 0_usize;
    for row in rows {
        let session_id: String = row.get("id");
        let session = fetch_live_ingest_session_by_id_global_unreconciled(
            state.db.sqlite_adapter(),
            &session_id,
        )
        .await?;
        let before =
            fetch_live_runtime_output_for_session(state.db.sqlite_adapter(), &session_id).await?;
        let after = reconcile_live_runtime_output_artifacts(&state, &session).await?;
        if runtime_output_changed(before.as_ref(), after.as_ref()) {
            publish_current_creator_live_state(&state, &session.creator_id).await?;
            reconciled += 1;
        }
    }
    Ok(reconciled)
}

pub(crate) async fn reconcile_live_runtime_output_artifacts(
    state: &SharedState,
    session: &LiveIngestSession,
) -> AppResult<Option<LiveRuntimeOutput>> {
    let Some(current) =
        fetch_live_runtime_output_for_session(state.db.sqlite_adapter(), &session.id).await?
    else {
        return Ok(None);
    };

    let inspection = inspect_live_runtime_output_artifacts(state, session, &current).await?;
    if let Some(output) =
        promote_archive_completion_from_artifacts(state, session, &current, &inspection).await?
    {
        return Ok(Some(output));
    }
    if inspection.issues.is_empty() {
        return Ok(Some(current));
    }

    let next_runtime_state = next_runtime_state_for_artifact_issues(&current, &inspection);
    let next_packaging_status = next_packaging_status_for_artifact_issues(&current, &inspection);
    let next_archive_status = next_archive_status_for_artifact_issues(&current, &inspection);
    let next_last_error = Some(inspection.issues.join("; "));

    if current.runtime_state == next_runtime_state
        && current.packaging_status == next_packaging_status
        && current.archive_status == next_archive_status
        && current.last_error == next_last_error
    {
        return Ok(Some(current));
    }

    let (output, actions) = repair_live_runtime_output(
        state.db.sqlite_adapter(),
        session,
        &RepairLiveRuntimeOutputRequest {
            reason: "runtime artifact reconciliation".to_string(),
            runtime_state: Some(next_runtime_state),
            packaging_status: Some(next_packaging_status),
            archive_status: Some(next_archive_status),
            manifest_relative_path: None,
            archive_relative_path: None,
            last_error: next_last_error.clone(),
            clear_manifest_relative_path: false,
            clear_archive_relative_path: false,
            clear_last_error: false,
        },
    )
    .await?;
    persist_live_runtime_spec(state, session).await?;

    record_live_runtime_telemetry(
        state.db.sqlite_adapter(),
        session,
        "runtime_artifact_reconciled",
        &output.runtime_state,
        &output.packaging_status,
        &output.archive_status,
        None,
        None,
        json!({
            "issues": inspection.issues,
            "actions": actions,
        }),
    )
    .await?;
    write_live_ingest_event(
        state.db.sqlite_adapter(),
        &session.id,
        &session.creator_id,
        &session.broadcast_id,
        "runtime_artifact_reconciled",
        json!({
            "issues": next_last_error,
            "runtimeState": output.runtime_state,
            "packagingStatus": output.packaging_status,
            "archiveStatus": output.archive_status,
            "manifestRelativePath": output.manifest_relative_path,
            "archiveRelativePath": output.archive_relative_path,
            "actions": actions,
        }),
    )
    .await?;
    Ok(Some(output))
}

async fn promote_archive_completion_from_artifacts(
    state: &SharedState,
    session: &LiveIngestSession,
    current: &LiveRuntimeOutput,
    inspection: &RuntimeArtifactInspection,
) -> AppResult<Option<LiveRuntimeOutput>> {
    if !inspection.issues.is_empty()
        || !inspection.archive_valid
        || !inspection.manifest_valid
        || current.archive_status != "finalizing"
    {
        return Ok(None);
    }

    let next_runtime_state = match current.runtime_state.as_str() {
        "archive_finalizing" | "disconnected" | "stale" => "archive_complete",
        state => state,
    };
    let (output, actions) = repair_live_runtime_output(
        state.db.sqlite_adapter(),
        session,
        &RepairLiveRuntimeOutputRequest {
            reason: "runtime archive completion reconciliation".to_string(),
            runtime_state: Some(next_runtime_state.to_string()),
            packaging_status: Some("ready".to_string()),
            archive_status: Some("complete".to_string()),
            manifest_relative_path: None,
            archive_relative_path: None,
            last_error: None,
            clear_manifest_relative_path: false,
            clear_archive_relative_path: false,
            clear_last_error: true,
        },
    )
    .await?;
    persist_live_runtime_spec(state, session).await?;
    record_live_runtime_telemetry(
        state.db.sqlite_adapter(),
        session,
        "runtime_archive_completed",
        &output.runtime_state,
        &output.packaging_status,
        &output.archive_status,
        None,
        None,
        json!({
            "manifestRelativePath": output.manifest_relative_path,
            "archiveRelativePath": output.archive_relative_path,
            "actions": actions,
        }),
    )
    .await?;
    write_live_ingest_event(
        state.db.sqlite_adapter(),
        &session.id,
        &session.creator_id,
        &session.broadcast_id,
        "runtime_archive_completed",
        json!({
            "runtimeState": output.runtime_state,
            "packagingStatus": output.packaging_status,
            "archiveStatus": output.archive_status,
            "manifestRelativePath": output.manifest_relative_path,
            "archiveRelativePath": output.archive_relative_path,
            "actions": actions,
        }),
    )
    .await?;
    Ok(Some(output))
}

fn next_runtime_state_for_artifact_issues(
    current: &LiveRuntimeOutput,
    inspection: &RuntimeArtifactInspection,
) -> String {
    if inspection.archive_invalid {
        return match current.runtime_state.as_str() {
            "disconnected" | "stale" => current.runtime_state.clone(),
            _ => "failed".to_string(),
        };
    }
    if inspection.manifest_invalid || inspection.collaboration_invalid {
        if matches!(current.packaging_status.as_str(), "pending" | "attached") {
            return current.runtime_state.clone();
        }
        return match current.runtime_state.as_str() {
            "disconnected" | "stale" | "failed" => current.runtime_state.clone(),
            _ => "packaging_degraded".to_string(),
        };
    }
    current.runtime_state.clone()
}

fn next_packaging_status_for_artifact_issues(
    current: &LiveRuntimeOutput,
    inspection: &RuntimeArtifactInspection,
) -> String {
    if (inspection.manifest_invalid || inspection.collaboration_invalid)
        && matches!(current.packaging_status.as_str(), "ready" | "complete")
    {
        return "degraded".to_string();
    }
    current.packaging_status.clone()
}

fn next_archive_status_for_artifact_issues(
    current: &LiveRuntimeOutput,
    inspection: &RuntimeArtifactInspection,
) -> String {
    if inspection.archive_invalid
        && matches!(current.archive_status.as_str(), "finalizing" | "complete")
    {
        return "failed".to_string();
    }
    current.archive_status.clone()
}

fn runtime_output_changed(
    before: Option<&LiveRuntimeOutput>,
    after: Option<&LiveRuntimeOutput>,
) -> bool {
    match (before, after) {
        (Some(before), Some(after)) => {
            before.runtime_state != after.runtime_state
                || before.packaging_status != after.packaging_status
                || before.archive_status != after.archive_status
                || before.last_error != after.last_error
                || before.manifest_relative_path != after.manifest_relative_path
                || before.archive_relative_path != after.archive_relative_path
        }
        (None, Some(_)) | (Some(_), None) => true,
        (None, None) => false,
    }
}
