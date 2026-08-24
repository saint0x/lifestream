use super::*;
use crate::api::control::artifacts::spec::{
    collaboration_launch_relative_path, collaboration_return_relative_path,
};
use crate::api::media::build_collaboration_media_runtime;
use std::collections::HashSet;

#[derive(Clone, Debug, Default)]
pub(super) struct CollaborationArtifactInspection {
    pub(super) present: bool,
    pub(super) valid: bool,
    pub(super) engine_relative_path: Option<String>,
    pub(super) issues: Vec<String>,
}

pub(super) async fn inspect_live_runtime_collaboration_artifacts(
    state: &SharedState,
    session: &LiveIngestSession,
) -> AppResult<CollaborationArtifactInspection> {
    let Some(collaboration_session) = fetch_active_collaboration_session_for_broadcast(
        state.db.try_sqlite_adapter()?,
        &session.broadcast_id,
    )
    .await?
    else {
        return Ok(CollaborationArtifactInspection::default());
    };
    let runtime = build_collaboration_runtime_response_for_host(
        state.db.try_sqlite_adapter()?,
        collaboration_session,
    )
    .await?;
    let engine_relative_path = collaboration_engine_relative_path(session);
    let bundle_relative_path = collaboration_bundle_relative_path(session);
    let media_relative_path = collaboration_media_relative_path(session);
    let launch_relative_path = collaboration_launch_relative_path(session);
    let runtime_output =
        fetch_live_runtime_output_for_session(state.db.try_sqlite_adapter()?, &session.id).await?;
    let artifacts_expected = runtime_output.as_ref().is_some_and(|output| {
        matches!(
            output.packaging_status.as_str(),
            "ready" | "complete" | "degraded" | "failed"
        ) || matches!(
            output.archive_status.as_str(),
            "finalizing" | "complete" | "failed"
        )
    });
    if !artifacts_expected {
        return Ok(CollaborationArtifactInspection {
            present: false,
            valid: false,
            engine_relative_path: Some(engine_relative_path),
            issues: Vec::new(),
        });
    }
    let runtime_bundle = build_collaboration_runtime_bundle(session, &runtime.topology)?;
    let media_runtime = build_collaboration_media_runtime(&runtime_bundle)?;
    let mut issues = Vec::new();

    validate_execution_plan_consistency(&runtime.topology.engine, &mut issues);
    validate_required_artifact_path(
        state,
        &engine_relative_path,
        "collaboration engine",
        &mut issues,
    )
    .await?;
    validate_required_artifact_path(
        state,
        &bundle_relative_path,
        "collaboration runtime bundle",
        &mut issues,
    )
    .await?;
    validate_required_artifact_path(
        state,
        &media_relative_path,
        "collaboration media runtime",
        &mut issues,
    )
    .await?;
    validate_required_artifact_path(
        state,
        &launch_relative_path,
        "collaboration launch runtime",
        &mut issues,
    )
    .await?;

    for program in runtime
        .topology
        .programs
        .iter()
        .filter(|program| program.target_broadcast_id.is_some())
    {
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
    for route in &media_runtime.return_targets {
        let relative_path = collaboration_return_relative_path(session, route);
        validate_required_artifact_path(
            state,
            &relative_path,
            &format!("collaboration return {}", route.participant_id),
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
            "archive" => {
                route.recording_enabled
                    && runtime_output
                        .as_ref()
                        .is_some_and(|output| output.archive_status == "complete")
            }
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
        engine_relative_path: Some(engine_relative_path),
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
    let node_ids = engine
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<HashSet<_>>();
    let bus_ids = engine
        .buses
        .iter()
        .map(|bus| bus.id.clone())
        .collect::<HashSet<_>>();
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
