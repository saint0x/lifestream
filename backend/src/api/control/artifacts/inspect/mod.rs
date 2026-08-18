use super::spec::{
    collaboration_audio_relative_path, collaboration_bundle_relative_path,
    collaboration_engine_relative_path, collaboration_media_relative_path,
    collaboration_program_relative_path, collaboration_route_relative_path,
};
use super::*;
use crate::api::collab::{
    build_collaboration_runtime_response_for_host, fetch_active_collaboration_session_for_broadcast,
};
use crate::api::control::{
    canonical_live_runtime_archive_relative_path, canonical_live_runtime_manifest_relative_path,
};
use crate::models::{LiveRuntimeArtifactHealth, LiveRuntimeArtifactState};

mod collab;
mod live;
mod state;

use collab::inspect_live_runtime_collaboration_artifacts;
use live::{archive_artifact_exists, validate_archive_artifact, validate_manifest_artifact};
use state::{artifact_state_label, declared_artifact_issue, declared_artifact_state_label};

#[derive(Clone, Debug)]
pub(super) struct RuntimeArtifactInspection {
    pub(super) manifest_invalid: bool,
    pub(super) archive_invalid: bool,
    pub(super) collaboration_present: bool,
    pub(super) collaboration_invalid: bool,
    pub(super) manifest_valid: bool,
    pub(super) archive_valid: bool,
    pub(super) collaboration_valid: bool,
    pub(super) collaboration_expected_relative_path: Option<String>,
    pub(super) issues: Vec<String>,
}

pub(super) async fn inspect_live_runtime_output_artifacts(
    state: &SharedState,
    session: &LiveIngestSession,
    output: &LiveRuntimeOutput,
) -> AppResult<RuntimeArtifactInspection> {
    let mut issues = Vec::new();
    let mut manifest_invalid = false;
    let mut archive_invalid = false;
    let mut collaboration_present = false;
    let mut collaboration_invalid = false;
    let mut manifest_valid = false;
    let mut archive_valid = false;
    let mut collaboration_valid = false;
    let mut collaboration_expected_relative_path = None;

    if matches!(output.packaging_status.as_str(), "ready" | "complete") {
        let Some(manifest_relative_path) = output.manifest_relative_path.as_deref() else {
            manifest_invalid = true;
            issues
                .push("packaging reported ready without a runtime-owned manifest path".to_string());
            return Ok(RuntimeArtifactInspection {
                manifest_invalid,
                archive_invalid,
                collaboration_present,
                collaboration_invalid,
                manifest_valid,
                archive_valid,
                collaboration_valid,
                collaboration_expected_relative_path,
                issues,
            });
        };
        if let Some(issue) = validate_manifest_artifact(state, manifest_relative_path).await? {
            manifest_invalid = true;
            issues.push(issue);
        } else {
            manifest_valid = true;
        }
    }

    if matches!(output.archive_status.as_str(), "finalizing" | "complete") {
        let Some(archive_relative_path) = output.archive_relative_path.as_deref() else {
            archive_invalid = true;
            issues.push(
                "archive reported available without a runtime-owned archive path".to_string(),
            );
            return Ok(RuntimeArtifactInspection {
                manifest_invalid,
                archive_invalid,
                collaboration_present,
                collaboration_invalid,
                manifest_valid,
                archive_valid,
                collaboration_valid,
                collaboration_expected_relative_path,
                issues,
            });
        };
        if output.archive_status == "complete" {
            if let Some(issue) = validate_archive_artifact(state, archive_relative_path).await? {
                archive_invalid = true;
                issues.push(issue);
            } else {
                archive_valid = true;
            }
        } else if archive_artifact_exists(state, archive_relative_path).await? {
            if let Some(issue) = validate_archive_artifact(state, archive_relative_path).await? {
                archive_invalid = true;
                issues.push(issue);
            } else {
                archive_valid = true;
            }
        }
    }

    let collaboration_inspection =
        inspect_live_runtime_collaboration_artifacts(state, session).await?;
    collaboration_present = collaboration_inspection.present;
    collaboration_invalid = !collaboration_inspection.issues.is_empty();
    collaboration_valid = collaboration_inspection.valid;
    collaboration_expected_relative_path = collaboration_inspection.engine_relative_path;
    issues.extend(collaboration_inspection.issues);

    Ok(RuntimeArtifactInspection {
        manifest_invalid,
        archive_invalid,
        collaboration_present,
        collaboration_invalid,
        manifest_valid,
        archive_valid,
        collaboration_valid,
        collaboration_expected_relative_path,
        issues,
    })
}

pub(crate) async fn describe_live_runtime_artifact_health(
    state: &SharedState,
    session: &LiveIngestSession,
    output: &LiveRuntimeOutput,
) -> AppResult<LiveRuntimeArtifactHealth> {
    let inspection = inspect_live_runtime_output_artifacts(state, session, output).await?;
    let checked_at = Utc::now().to_rfc3339();
    let expected_manifest_relative_path = canonical_live_runtime_manifest_relative_path(session);
    let expected_archive_relative_path = canonical_live_runtime_archive_relative_path(session);
    let manifest_issue = inspection
        .issues
        .iter()
        .find(|issue| issue.contains("manifest"))
        .cloned();
    let archive_issue = inspection
        .issues
        .iter()
        .find(|issue| issue.contains("archive"))
        .cloned();
    let collaboration_issue = inspection
        .issues
        .iter()
        .find(|issue| issue.contains("collaboration"))
        .cloned();
    let manifest = LiveRuntimeArtifactState {
        expected_relative_path: Some(expected_manifest_relative_path.clone()),
        persisted_relative_path: output.manifest_relative_path.clone(),
        state: artifact_state_label(
            output.packaging_status.as_str(),
            inspection.manifest_valid,
            inspection.manifest_invalid,
            output.manifest_relative_path.as_deref(),
            Some(expected_manifest_relative_path.as_str()),
        ),
        ready: matches!(output.packaging_status.as_str(), "ready" | "complete"),
        valid: inspection.manifest_valid,
        issue: manifest_issue,
    };
    let archive = LiveRuntimeArtifactState {
        expected_relative_path: Some(expected_archive_relative_path.clone()),
        persisted_relative_path: output.archive_relative_path.clone(),
        state: artifact_state_label(
            output.archive_status.as_str(),
            inspection.archive_valid,
            inspection.archive_invalid,
            output.archive_relative_path.as_deref(),
            Some(expected_archive_relative_path.as_str()),
        ),
        ready: matches!(output.archive_status.as_str(), "finalizing" | "complete"),
        valid: inspection.archive_valid,
        issue: archive_issue,
    };
    let collaboration = inspection
        .collaboration_present
        .then(|| LiveRuntimeArtifactState {
            expected_relative_path: inspection.collaboration_expected_relative_path.clone(),
            persisted_relative_path: inspection.collaboration_expected_relative_path.clone(),
            state: artifact_state_label(
                if inspection.collaboration_present {
                    "ready"
                } else {
                    "pending"
                },
                inspection.collaboration_valid,
                inspection.collaboration_invalid,
                inspection.collaboration_expected_relative_path.as_deref(),
                inspection.collaboration_expected_relative_path.as_deref(),
            ),
            ready: inspection.collaboration_present,
            valid: inspection.collaboration_valid,
            issue: collaboration_issue,
        });
    let status = if inspection.archive_invalid
        || inspection.manifest_invalid
        || inspection.collaboration_invalid
    {
        "invalid"
    } else if inspection.archive_valid
        || inspection.manifest_valid
        || inspection.collaboration_valid
        || matches!(output.packaging_status.as_str(), "ready" | "complete")
        || matches!(output.archive_status.as_str(), "finalizing" | "complete")
    {
        "checked"
    } else {
        "pending"
    };

    Ok(LiveRuntimeArtifactHealth {
        status: status.to_string(),
        checked_at,
        manifest,
        archive,
        collaboration,
        issues: inspection.issues,
    })
}

pub(crate) fn describe_declared_live_runtime_artifact_health(
    session: &LiveIngestSession,
    output: &LiveRuntimeOutput,
) -> LiveRuntimeArtifactHealth {
    let checked_at = Utc::now().to_rfc3339();
    let expected_manifest_relative_path = canonical_live_runtime_manifest_relative_path(session);
    let expected_archive_relative_path = canonical_live_runtime_archive_relative_path(session);
    let collaboration_expected_relative_path =
        declared_collaboration_expected_relative_path(session, output);
    let manifest = LiveRuntimeArtifactState {
        expected_relative_path: Some(expected_manifest_relative_path.clone()),
        persisted_relative_path: output.manifest_relative_path.clone(),
        state: declared_artifact_state_label(
            output.packaging_status.as_str(),
            output.manifest_relative_path.as_deref(),
            Some(expected_manifest_relative_path.as_str()),
        ),
        ready: matches!(output.packaging_status.as_str(), "ready" | "complete"),
        valid: false,
        issue: declared_artifact_issue(
            output.packaging_status.as_str(),
            output.manifest_relative_path.as_deref(),
            Some(expected_manifest_relative_path.as_str()),
            "manifest",
        ),
    };
    let archive = LiveRuntimeArtifactState {
        expected_relative_path: Some(expected_archive_relative_path.clone()),
        persisted_relative_path: output.archive_relative_path.clone(),
        state: declared_artifact_state_label(
            output.archive_status.as_str(),
            output.archive_relative_path.as_deref(),
            Some(expected_archive_relative_path.as_str()),
        ),
        ready: matches!(output.archive_status.as_str(), "finalizing" | "complete"),
        valid: false,
        issue: declared_artifact_issue(
            output.archive_status.as_str(),
            output.archive_relative_path.as_deref(),
            Some(expected_archive_relative_path.as_str()),
            "archive",
        ),
    };
    let collaboration =
        collaboration_expected_relative_path
            .clone()
            .map(|expected_relative_path| {
                let issue = declared_collaboration_artifact_issue(output.last_error.as_deref());
                LiveRuntimeArtifactState {
                    expected_relative_path: Some(expected_relative_path.clone()),
                    persisted_relative_path: Some(expected_relative_path),
                    state: if issue.is_some() {
                        "invalid".to_string()
                    } else {
                        "declared".to_string()
                    },
                    ready: true,
                    valid: false,
                    issue,
                }
            });
    let issues = [
        manifest.issue.clone(),
        archive.issue.clone(),
        collaboration.as_ref().and_then(|state| state.issue.clone()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    LiveRuntimeArtifactHealth {
        status: if issues.is_empty() {
            if manifest.ready || archive.ready {
                "declared".to_string()
            } else {
                "pending".to_string()
            }
        } else {
            "attention".to_string()
        },
        checked_at,
        manifest,
        archive,
        collaboration,
        issues,
    }
}

fn declared_collaboration_expected_relative_path(
    session: &LiveIngestSession,
    output: &LiveRuntimeOutput,
) -> Option<String> {
    let collaboration_artifacts_expected = matches!(
        output.packaging_status.as_str(),
        "ready" | "complete" | "degraded" | "failed"
    ) || matches!(
        output.archive_status.as_str(),
        "finalizing" | "complete" | "failed"
    );
    collaboration_artifacts_expected.then(|| collaboration_engine_relative_path(session))
}

fn declared_collaboration_artifact_issue(last_error: Option<&str>) -> Option<String> {
    let last_error = last_error?;
    last_error
        .contains("collaboration")
        .then(|| last_error.to_string())
}
