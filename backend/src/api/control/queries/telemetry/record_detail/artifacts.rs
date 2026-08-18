use super::*;
use crate::api::control::{
    canonical_live_runtime_archive_relative_path, canonical_live_runtime_manifest_relative_path,
};

pub(super) fn build_live_runtime_telemetry_artifact_detail(
    session: &LiveIngestSession,
    packaging_status: &str,
    archive_status: &str,
    output: Option<&LiveRuntimeOutput>,
) -> Value {
    let expected_manifest_relative_path = canonical_live_runtime_manifest_relative_path(session);
    let expected_archive_relative_path = canonical_live_runtime_archive_relative_path(session);
    let persisted_manifest_relative_path =
        output.and_then(|item| item.manifest_relative_path.clone());
    let persisted_archive_relative_path =
        output.and_then(|item| item.archive_relative_path.clone());

    json!({
        "status": if artifact_state_has_attention(
            packaging_status,
            persisted_manifest_relative_path.as_deref(),
            Some(expected_manifest_relative_path.as_str()),
        ) || artifact_state_has_attention(
            archive_status,
            persisted_archive_relative_path.as_deref(),
            Some(expected_archive_relative_path.as_str()),
        ) {
            "attention"
        } else if matches!(packaging_status, "ready" | "complete")
            || matches!(archive_status, "finalizing" | "complete")
        {
            "declared"
        } else {
            "pending"
        },
        "manifest": {
            "expectedRelativePath": expected_manifest_relative_path,
            "persistedRelativePath": persisted_manifest_relative_path,
            "state": telemetry_artifact_state(
                packaging_status,
                output.and_then(|item| item.manifest_relative_path.as_deref()),
                Some(expected_manifest_relative_path.as_str()),
            ),
        },
        "archive": {
            "expectedRelativePath": expected_archive_relative_path,
            "persistedRelativePath": persisted_archive_relative_path,
            "state": telemetry_artifact_state(
                archive_status,
                output.and_then(|item| item.archive_relative_path.as_deref()),
                Some(expected_archive_relative_path.as_str()),
            ),
        }
    })
}

fn telemetry_artifact_state(
    status: &str,
    persisted_relative_path: Option<&str>,
    expected_relative_path: Option<&str>,
) -> &'static str {
    let ready = matches!(status, "ready" | "complete" | "finalizing");
    if !ready {
        return "pending";
    }
    match persisted_relative_path {
        None => "missing",
        Some(path) if Some(path) != expected_relative_path => "drifted",
        Some(_) => "declared",
    }
}

fn artifact_state_has_attention(
    status: &str,
    persisted_relative_path: Option<&str>,
    expected_relative_path: Option<&str>,
) -> bool {
    matches!(
        telemetry_artifact_state(status, persisted_relative_path, expected_relative_path),
        "missing" | "drifted"
    )
}
