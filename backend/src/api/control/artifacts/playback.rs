use super::inspect::validate_playback_manifest_artifact;
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
    if let Some(issue) =
        validate_playback_manifest_artifact(state, expected_manifest_relative_path).await?
    {
        return Err(AppError::BadRequest(issue));
    }
    Ok(())
}
