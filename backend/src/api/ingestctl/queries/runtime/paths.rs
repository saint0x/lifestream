use super::*;

pub(crate) fn canonical_live_runtime_manifest_relative_path(session: &LiveIngestSession) -> String {
    format!(
        "live/{}/{}/{}/master.m3u8",
        session.creator_id, session.broadcast_id, session.id
    )
}

pub(crate) fn canonical_live_runtime_archive_relative_path(session: &LiveIngestSession) -> String {
    format!(
        "archive/{}/{}/{}/final.mp4",
        session.creator_id, session.broadcast_id, session.id
    )
}

pub(crate) fn canonical_live_runtime_archive_staging_relative_path(
    session: &LiveIngestSession,
) -> String {
    format!(
        "archive/{}/{}/{}/staging/final.partial.mp4",
        session.creator_id, session.broadcast_id, session.id
    )
}

pub(crate) fn canonical_live_runtime_spec_relative_path(session: &LiveIngestSession) -> String {
    format!(
        "runtime/{}/{}/{}/spec.json",
        session.creator_id, session.broadcast_id, session.id
    )
}

pub(super) fn normalize_optional_path(value: Option<&str>) -> AppResult<Option<String>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.starts_with('/') || value.contains("..") {
        return Err(AppError::BadRequest(
            "runtime output paths must be repository-relative".to_string(),
        ));
    }
    Ok(Some(value.to_string()))
}

pub(super) fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn resolve_manifest_relative_path(
    session: &LiveIngestSession,
    packaging_status: &str,
    value: Option<&str>,
) -> AppResult<Option<String>> {
    resolve_runtime_relative_path(
        "manifestRelativePath",
        &canonical_live_runtime_manifest_relative_path(session),
        matches!(packaging_status, "ready" | "complete"),
        value,
    )
}

pub(super) fn resolve_archive_relative_path(
    session: &LiveIngestSession,
    archive_status: &str,
    value: Option<&str>,
) -> AppResult<Option<String>> {
    resolve_runtime_relative_path(
        "archiveRelativePath",
        &canonical_live_runtime_archive_relative_path(session),
        matches!(archive_status, "finalizing" | "complete"),
        value,
    )
}

fn resolve_runtime_relative_path(
    field: &str,
    expected: &str,
    required: bool,
    value: Option<&str>,
) -> AppResult<Option<String>> {
    let normalized = normalize_optional_path(value)?;
    match normalized {
        Some(path) if path == expected => Ok(Some(path)),
        Some(_) => Err(AppError::BadRequest(format!(
            "{field} must match the backend-owned runtime path {expected}"
        ))),
        None if required => Err(AppError::BadRequest(format!(
            "{field} is required and must match the backend-owned runtime path {expected}"
        ))),
        None => Ok(None),
    }
}
