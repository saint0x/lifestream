use super::*;

pub(crate) fn validate_runtime_state_input(input: &UpdateLiveRuntimeStateRequest) -> AppResult<()> {
    ensure_allowed_value(
        input.runtime_state.trim(),
        &[
            "pending_attach",
            "attached",
            "healthy",
            "degraded",
            "stale",
            "disconnected",
            "packaging_active",
            "packaging_degraded",
            "archive_finalizing",
            "archive_complete",
            "failed",
        ],
        "runtimeState",
    )?;
    ensure_allowed_value(
        input.packaging_status.trim(),
        &[
            "pending", "attached", "active", "ready", "degraded", "complete", "failed",
        ],
        "packagingStatus",
    )?;
    ensure_allowed_value(
        input.archive_status.trim(),
        &["not_started", "finalizing", "complete", "failed"],
        "archiveStatus",
    )?;
    Ok(())
}

fn ensure_allowed_value(value: &str, allowed: &[&str], field: &str) -> AppResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(AppError::BadRequest(format!(
            "{field} must be one of: {}",
            allowed.join(", ")
        )))
    }
}
