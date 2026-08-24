use super::*;

pub(crate) const LIVE_RUNTIME_SPEC_RETENTION_DAYS: i64 = 30;
pub(crate) const LIVE_PLAYBACK_ARTIFACT_RETENTION_HOURS: i64 = 24;
pub(crate) const LIVE_MIRROR_ARTIFACT_RETENTION_HOURS: i64 = 24;
pub(crate) const LIVE_ARCHIVE_RETENTION_DAYS: i64 = 3650;
pub(crate) const LIVE_ARCHIVE_STAGING_RETENTION_HOURS: i64 = 24;

pub(crate) fn live_playback_artifact_prefix(session: &LiveIngestSession) -> String {
    format!(
        "live/{}/{}/{}",
        session.creator_id, session.broadcast_id, session.id
    )
}

pub(crate) fn live_archive_artifact_prefix(session: &LiveIngestSession) -> String {
    format!(
        "archive/{}/{}/{}",
        session.creator_id, session.broadcast_id, session.id
    )
}

pub(crate) fn live_runtime_workspace_prefix(session: &LiveIngestSession) -> String {
    format!(
        "runtime/{}/{}/{}",
        session.creator_id, session.broadcast_id, session.id
    )
}

pub(crate) fn live_mirror_playback_artifact_prefix(
    creator_id: &str,
    broadcast_id: &str,
    route_id: &str,
) -> String {
    format!("live/{creator_id}/{broadcast_id}/{route_id}")
}

pub(crate) fn live_mirror_archive_artifact_prefix(
    creator_id: &str,
    broadcast_id: &str,
    route_id: &str,
) -> String {
    format!("archive/{creator_id}/{broadcast_id}/{route_id}")
}

pub(crate) fn canonical_live_runtime_manifest_relative_path(session: &LiveIngestSession) -> String {
    format!("{}/master.m3u8", live_playback_artifact_prefix(session))
}

pub(crate) fn canonical_live_runtime_archive_relative_path(session: &LiveIngestSession) -> String {
    format!("{}/final.mp4", live_archive_artifact_prefix(session))
}

pub(crate) fn canonical_live_runtime_archive_staging_relative_path(
    session: &LiveIngestSession,
) -> String {
    format!(
        "{}/staging/final.partial.mp4",
        live_archive_artifact_prefix(session)
    )
}

pub(crate) fn canonical_live_runtime_spec_relative_path(session: &LiveIngestSession) -> String {
    format!("{}/spec.json", live_runtime_workspace_prefix(session))
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
