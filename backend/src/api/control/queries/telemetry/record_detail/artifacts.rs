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
    let manifest_state = telemetry_artifact_state(
        "runtime manifest",
        packaging_status,
        output.and_then(|item| item.manifest_relative_path.as_deref()),
        Some(expected_manifest_relative_path.as_str()),
        output.and_then(|item| item.last_error.as_deref()),
    );
    let archive_state = telemetry_artifact_state(
        "runtime archive",
        archive_status,
        output.and_then(|item| item.archive_relative_path.as_deref()),
        Some(expected_archive_relative_path.as_str()),
        output.and_then(|item| item.last_error.as_deref()),
    );

    json!({
        "status": if artifact_state_has_attention(manifest_state)
            || artifact_state_has_attention(archive_state) {
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
            "state": manifest_state,
        },
        "archive": {
            "expectedRelativePath": expected_archive_relative_path,
            "persistedRelativePath": persisted_archive_relative_path,
            "state": archive_state,
        }
    })
}

fn telemetry_artifact_state(
    artifact_label: &str,
    status: &str,
    persisted_relative_path: Option<&str>,
    expected_relative_path: Option<&str>,
    last_error: Option<&str>,
) -> &'static str {
    if last_error.is_some_and(|error| error.contains(artifact_label)) {
        return "missing";
    }
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

fn artifact_state_has_attention(state: &str) -> bool {
    matches!(state, "missing" | "drifted")
}
