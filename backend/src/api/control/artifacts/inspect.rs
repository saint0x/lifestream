use super::*;
use super::spec::{
    collaboration_audio_relative_path, collaboration_engine_relative_path,
    collaboration_program_relative_path, collaboration_route_relative_path,
    collaboration_bundle_relative_path,
};
use crate::api::collab::{
    build_collaboration_runtime_response_for_host, fetch_active_collaboration_session_for_broadcast,
};
use crate::api::control::{
    canonical_live_runtime_archive_relative_path, canonical_live_runtime_manifest_relative_path,
};
use crate::models::{LiveRuntimeArtifactHealth, LiveRuntimeArtifactState};
use std::collections::HashSet;

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
        if let Some(issue) = validate_archive_artifact(state, archive_relative_path).await? {
            archive_invalid = true;
            issues.push(issue);
        } else {
            archive_valid = true;
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
    let collaboration = inspection.collaboration_present.then(|| LiveRuntimeArtifactState {
        expected_relative_path: inspection.collaboration_expected_relative_path.clone(),
        persisted_relative_path: inspection.collaboration_expected_relative_path.clone(),
        state: artifact_state_label(
            if inspection.collaboration_present { "ready" } else { "pending" },
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
    let collaboration = None;
    let issues = [manifest.issue.clone(), archive.issue.clone()]
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

#[derive(Clone, Debug, Default)]
struct CollaborationArtifactInspection {
    present: bool,
    valid: bool,
    engine_relative_path: Option<String>,
    issues: Vec<String>,
}

async fn inspect_live_runtime_collaboration_artifacts(
    state: &SharedState,
    session: &LiveIngestSession,
) -> AppResult<CollaborationArtifactInspection> {
    let Some(collaboration_session) =
        fetch_active_collaboration_session_for_broadcast(&state.pool, &session.broadcast_id).await?
    else {
        return Ok(CollaborationArtifactInspection::default());
    };
    let runtime =
        build_collaboration_runtime_response_for_host(&state.pool, collaboration_session).await?;
    let engine_relative_path = collaboration_engine_relative_path(session);
    let bundle_relative_path = collaboration_bundle_relative_path(session);
    let mut issues = Vec::new();

    validate_execution_plan_consistency(&runtime.topology.engine, &mut issues);
    validate_required_artifact_path(state, &engine_relative_path, "collaboration engine", &mut issues)
        .await?;
    validate_required_artifact_path(
        state,
        &bundle_relative_path,
        "collaboration runtime bundle",
        &mut issues,
    )
    .await?;

    for program in &runtime.topology.programs {
        let relative_path = collaboration_program_relative_path(session, program);
        validate_required_artifact_path(
            state,
            &relative_path,
            &format!("collaboration program {}", program.id),
            &mut issues,
        )
        .await?;
    }
    for audio in &runtime.topology.audio {
        let relative_path = collaboration_audio_relative_path(session, audio);
        validate_required_artifact_path(
            state,
            &relative_path,
            &format!("collaboration audio {}", audio.participant_id),
            &mut issues,
        )
        .await?;
    }
    for route in &runtime.topology.outputs {
        let Some(relative_path) = collaboration_route_relative_path(session, route) else {
            continue;
        };
        let should_exist = match route.output_kind.as_str() {
            "host_channel" => false,
            "mirror_channel" => route.playback_enabled,
            "archive" => route.recording_enabled,
            _ => false,
        };
        if should_exist {
            validate_required_artifact_path(
                state,
                &relative_path,
                &format!("collaboration route {}", route.id),
                &mut issues,
            )
            .await?;
        }
    }

    Ok(CollaborationArtifactInspection {
        present: true,
        valid: issues.is_empty(),
        engine_relative_path: Some(bundle_relative_path),
        issues,
    })
}

async fn validate_required_artifact_path(
    state: &SharedState,
    relative_path: &str,
    label: &str,
    issues: &mut Vec<String>,
) -> AppResult<()> {
    let path = media_path_for_relative(state, relative_path);
    let metadata = match tokio::fs::metadata(&path).await {
        Ok(metadata) => metadata,
        Err(_) => {
            issues.push(format!(
                "{label} artifact missing at collaboration runtime path {relative_path}"
            ));
            return Ok(());
        }
    };
    if metadata.len() == 0 {
        issues.push(format!(
            "{label} artifact empty at collaboration runtime path {relative_path}"
        ));
    }
    Ok(())
}

fn validate_execution_plan_consistency(
    engine: &crate::models::CollaborationExecutionPlan,
    issues: &mut Vec<String>,
) {
    let node_ids = engine.nodes.iter().map(|node| node.id.clone()).collect::<HashSet<_>>();
    let bus_ids = engine.buses.iter().map(|bus| bus.id.clone()).collect::<HashSet<_>>();
    if engine.operations.is_empty() && !engine.edges.is_empty() {
        issues.push("collaboration engine compiled plan missing execution operations".to_string());
    }
    for edge in &engine.edges {
        if !node_ids.contains(&edge.from_node_id) || !node_ids.contains(&edge.to_node_id) {
            issues.push(format!(
                "collaboration engine edge {} references missing execution node",
                edge.id
            ));
        }
    }
    for operation in &engine.operations {
        if !bus_ids.contains(&operation.output_bus_id) {
            issues.push(format!(
                "collaboration operation {} references missing output bus",
                operation.id
            ));
        }
        if operation
            .input_bus_ids
            .iter()
            .any(|input_bus_id| !bus_ids.contains(input_bus_id))
        {
            issues.push(format!(
                "collaboration operation {} references missing input bus",
                operation.id
            ));
        }
    }
}

fn artifact_state_label(
    status: &str,
    valid: bool,
    invalid: bool,
    persisted_relative_path: Option<&str>,
    expected_relative_path: Option<&str>,
) -> String {
    if valid {
        return "valid".to_string();
    }
    if invalid {
        return "invalid".to_string();
    }
    if matches!(status, "ready" | "complete" | "finalizing") && persisted_relative_path.is_none() {
        return "missing".to_string();
    }
    if persisted_relative_path.is_some() && expected_relative_path != persisted_relative_path {
        return "drifted".to_string();
    }
    "pending".to_string()
}

fn declared_artifact_state_label(
    status: &str,
    persisted_relative_path: Option<&str>,
    expected_relative_path: Option<&str>,
) -> String {
    if !matches!(status, "ready" | "complete" | "finalizing") {
        return "pending".to_string();
    }
    match persisted_relative_path {
        None => "missing".to_string(),
        Some(path) if Some(path) != expected_relative_path => "drifted".to_string(),
        Some(_) => "declared".to_string(),
    }
}

fn declared_artifact_issue(
    status: &str,
    persisted_relative_path: Option<&str>,
    expected_relative_path: Option<&str>,
    artifact_kind: &str,
) -> Option<String> {
    if !matches!(status, "ready" | "complete" | "finalizing") {
        return None;
    }
    match persisted_relative_path {
        None => Some(format!(
            "{artifact_kind} is required for the current runtime state but no persisted path is present"
        )),
        Some(path) if Some(path) != expected_relative_path => Some(format!(
            "{artifact_kind} path {path} does not match the backend-owned runtime path {}",
            expected_relative_path.unwrap_or_default()
        )),
        Some(_) => None,
    }
}

async fn validate_manifest_artifact(
    state: &SharedState,
    relative_path: &str,
) -> AppResult<Option<String>> {
    let manifest_path = media_path_for_relative(state, relative_path);
    let metadata = match tokio::fs::metadata(&manifest_path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Some(format!(
                "runtime manifest {relative_path} does not exist on disk"
            )));
        }
        Err(error) => return Err(AppError::Io(error)),
    };
    if !metadata.is_file() {
        return Ok(Some(format!(
            "runtime manifest {relative_path} is not a regular file"
        )));
    }
    if metadata.len() == 0 {
        return Ok(Some(format!("runtime manifest {relative_path} is empty")));
    }
    let body = tokio::fs::read_to_string(&manifest_path)
        .await
        .map_err(AppError::Io)?;
    let trimmed = body.trim_start();
    if !trimmed.starts_with("#EXTM3U") {
        return Ok(Some(format!(
            "runtime manifest {relative_path} is not a valid HLS playlist"
        )));
    }
    Ok(None)
}

async fn validate_archive_artifact(
    state: &SharedState,
    relative_path: &str,
) -> AppResult<Option<String>> {
    let archive_path = media_path_for_relative(state, relative_path);
    let metadata = match tokio::fs::metadata(&archive_path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Some(format!(
                "runtime archive {relative_path} does not exist on disk"
            )));
        }
        Err(error) => return Err(AppError::Io(error)),
    };
    if !metadata.is_file() {
        return Ok(Some(format!(
            "runtime archive {relative_path} is not a regular file"
        )));
    }
    if metadata.len() == 0 {
        return Ok(Some(format!("runtime archive {relative_path} is empty")));
    }
    Ok(None)
}
