use super::inspect::inspect_live_runtime_output_artifacts;
use super::*;

pub(crate) async fn ensure_live_runtime_output_ready_for_playback(
    state: &SharedState,
    output: &LiveRuntimeOutput,
    expected_manifest_relative_path: &str,
) -> AppResult<()> {
    if !matches!(output.packaging_status.as_str(), "ready" | "complete") {
        return Err(AppError::BadRequest(
            "live runtime has not confirmed playback readiness".to_string(),
        ));
    }
    if output.manifest_relative_path.as_deref() != Some(expected_manifest_relative_path) {
        return Err(AppError::BadRequest(
            "live runtime manifest is not aligned with the published playback path".to_string(),
        ));
    }
    let session =
        fetch_live_ingest_session_by_id_global_unreconciled(&state.pool, &output.session_id)
            .await?;
    let inspection = inspect_live_runtime_output_artifacts(state, &session, output).await?;
    if !inspection.issues.is_empty() {
        return Err(AppError::BadRequest(inspection.issues.join("; ")));
    }
    Ok(())
}
