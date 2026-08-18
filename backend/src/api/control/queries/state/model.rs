use super::*;

pub(crate) fn validate_runtime_output_model(
    input: &UpdateLiveRuntimeStateRequest,
) -> AppResult<()> {
    let runtime_state = input.runtime_state.trim();
    let packaging_status = input.packaging_status.trim();
    let archive_status = input.archive_status.trim();
    let has_manifest = has_text(input.manifest_relative_path.as_deref());
    let has_archive = has_text(input.archive_relative_path.as_deref());
    let has_error = has_text(input.last_error.as_deref());

    validate_required_artifact_paths(
        runtime_state,
        packaging_status,
        archive_status,
        has_manifest,
        has_archive,
    )?;
    validate_runtime_packaging_consistency(runtime_state, packaging_status)?;
    validate_archive_consistency(runtime_state, packaging_status, archive_status)?;
    validate_failure_signals(runtime_state, packaging_status, archive_status, has_error)?;

    Ok(())
}

fn validate_required_artifact_paths(
    runtime_state: &str,
    packaging_status: &str,
    archive_status: &str,
    has_manifest: bool,
    has_archive: bool,
) -> AppResult<()> {
    if matches!(packaging_status, "ready" | "complete") && !has_manifest {
        return Err(AppError::BadRequest(
            "manifestRelativePath is required when packaging is ready or complete".to_string(),
        ));
    }
    if matches!(
        runtime_state,
        "packaging_active" | "packaging_degraded" | "archive_finalizing" | "archive_complete"
    ) && !has_manifest
    {
        return Err(AppError::BadRequest(
            "manifestRelativePath is required once live packaging is active".to_string(),
        ));
    }
    if matches!(packaging_status, "pending" | "attached") && has_manifest {
        return Err(AppError::BadRequest(
            "manifestRelativePath is not allowed before live packaging is active".to_string(),
        ));
    }
    if archive_status == "complete" && !has_archive {
        return Err(AppError::BadRequest(
            "archiveRelativePath is required when archiveStatus is complete".to_string(),
        ));
    }
    if archive_status == "not_started" && has_archive {
        return Err(AppError::BadRequest(
            "archiveRelativePath is not allowed before archive finalization starts".to_string(),
        ));
    }

    Ok(())
}

fn validate_runtime_packaging_consistency(
    runtime_state: &str,
    packaging_status: &str,
) -> AppResult<()> {
    if matches!(
        runtime_state,
        "packaging_degraded" | "failed" | "archive_finalizing" | "archive_complete"
    ) && matches!(packaging_status, "pending" | "attached")
    {
        return Err(AppError::BadRequest(
            "packagingStatus is inconsistent with the requested runtimeState".to_string(),
        ));
    }
    if runtime_state == "pending_attach" && packaging_status != "pending" {
        return Err(AppError::BadRequest(
            "pending_attach runtimeState requires packagingStatus=pending".to_string(),
        ));
    }
    if runtime_state == "attached" && packaging_status != "attached" {
        return Err(AppError::BadRequest(
            "attached runtimeState requires packagingStatus=attached".to_string(),
        ));
    }
    if runtime_state == "packaging_active" && packaging_status != "active" {
        return Err(AppError::BadRequest(
            "packaging_active runtimeState requires packagingStatus=active".to_string(),
        ));
    }
    if runtime_state == "packaging_degraded" && packaging_status != "degraded" {
        return Err(AppError::BadRequest(
            "packaging_degraded runtimeState requires packagingStatus=degraded".to_string(),
        ));
    }
    if matches!(runtime_state, "healthy" | "degraded") && packaging_status == "pending" {
        return Err(AppError::BadRequest(
            "healthy and degraded runtime states require packaging attachment".to_string(),
        ));
    }

    Ok(())
}

fn validate_archive_consistency(
    runtime_state: &str,
    packaging_status: &str,
    archive_status: &str,
) -> AppResult<()> {
    if runtime_state == "archive_finalizing" && archive_status != "finalizing" {
        return Err(AppError::BadRequest(
            "archive_finalizing runtimeState requires archiveStatus=finalizing".to_string(),
        ));
    }
    if runtime_state == "archive_complete" && archive_status != "complete" {
        return Err(AppError::BadRequest(
            "archive_complete runtimeState requires archiveStatus=complete".to_string(),
        ));
    }
    if matches!(archive_status, "finalizing" | "complete")
        && !matches!(
            packaging_status,
            "ready" | "complete" | "degraded" | "failed"
        )
    {
        return Err(AppError::BadRequest(
            "archiveStatus requires a packaged live output before archive handling begins"
                .to_string(),
        ));
    }
    if matches!(runtime_state, "archive_finalizing" | "archive_complete")
        && !matches!(packaging_status, "ready" | "complete")
    {
        return Err(AppError::BadRequest(
            "archive runtime states require packagingStatus=ready or complete".to_string(),
        ));
    }

    Ok(())
}

fn validate_failure_signals(
    runtime_state: &str,
    packaging_status: &str,
    archive_status: &str,
    has_error: bool,
) -> AppResult<()> {
    if runtime_state == "failed"
        && packaging_status != "failed"
        && archive_status != "failed"
        && !has_error
    {
        return Err(AppError::BadRequest(
            "failed runtimeState requires an explicit failure signal".to_string(),
        ));
    }
    if (runtime_state == "packaging_degraded"
        || packaging_status == "failed"
        || archive_status == "failed"
        || runtime_state == "failed")
        && !has_error
    {
        return Err(AppError::BadRequest(
            "lastError is required for degraded or failed runtime error states".to_string(),
        ));
    }

    Ok(())
}

fn has_text(value: Option<&str>) -> bool {
    value.is_some_and(|item| !item.trim().is_empty())
}
