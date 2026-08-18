use super::*;
use crate::api::control::{build_live_runtime_advisory, describe_live_runtime_artifact_health};
use doc::{
    LiveRuntimeArchiveSpec, LiveRuntimeCollaborationSpec, LiveRuntimePackagingSpec,
    LiveRuntimeReconnectSpec, LiveRuntimeSpecDocument, LiveRuntimeSpecPaths,
    LiveRuntimeSpecRuntime, LiveRuntimeSpecSession, LiveRuntimeTelemetrySpec,
};

pub(super) async fn build_live_runtime_spec(
    state: &SharedState,
    session: &LiveIngestSession,
    output: &LiveRuntimeOutput,
    spec_relative_path: &str,
) -> AppResult<LiveRuntimeSpecDocument> {
    let manifest_relative_path = canonical_live_runtime_manifest_relative_path(session);
    let archive_relative_path = canonical_live_runtime_archive_relative_path(session);
    let archive_staging_relative_path =
        canonical_live_runtime_archive_staging_relative_path(session);
    let output_root_relative_path = FsPath::new(&manifest_relative_path)
        .parent()
        .map(|path| path.to_string_lossy().to_string())
        .ok_or_else(|| {
            AppError::Internal("live runtime manifest path missing parent".to_string())
        })?;
    let session_ordinal = count_live_ingest_sessions_for_broadcast(
        &state.pool,
        &session.creator_id,
        &session.broadcast_id,
    )
    .await?;
    let (current_cpu_percent, current_free_disk_gb) =
        fetch_current_operational_telemetry(&state.pool, &session.creator_id).await?;
    let health = build_live_runtime_health_spec(session, current_cpu_percent, current_free_disk_gb);
    let variants = build_live_runtime_variant_specs(session, output)?;
    let collaboration = build_live_runtime_collaboration_spec(state, session).await?;
    let archive = build_live_runtime_archive_plan(
        session,
        output,
        archive_staging_relative_path,
        archive_relative_path.clone(),
        collaboration.as_ref(),
    );
    let advisory = build_live_runtime_advisory(Some(session), Some(output), None);
    let artifact_health = describe_live_runtime_artifact_health(state, session, output).await?;

    Ok(LiveRuntimeSpecDocument {
        session: LiveRuntimeSpecSession {
            id: session.id.clone(),
            creator_id: session.creator_id.clone(),
            broadcast_id: session.broadcast_id.clone(),
            previous_session_id: session.previous_session_id.clone(),
            protocol: session.protocol.clone(),
            contribution_class: session.contribution_class.clone(),
            contribution_state: session.contribution_state.clone(),
            ingest_server: session.ingest_server.clone(),
            status: session.status.clone(),
            bitrate_kbps: session.bitrate_kbps,
            viewers: session.viewers,
            dropped_frames: session.dropped_frames,
            ingest_latency_ms: session.ingest_latency_ms,
            connected_at: session.connected_at.clone(),
            last_heartbeat_at: session.last_heartbeat_at.clone(),
            disconnected_at: session.disconnected_at.clone(),
            session_ordinal,
            reconnect_session: session.previous_session_id.is_some(),
            source_probe: session.source_probe.clone(),
            source_validation: session.source_validation.clone(),
        },
        runtime: LiveRuntimeSpecRuntime {
            state: output.runtime_state.clone(),
            packaging_status: output.packaging_status.clone(),
            archive_status: output.archive_status.clone(),
            runtime_class: output.runtime_class.clone(),
            latency_profile: output.latency_profile.clone(),
            segment_format: output.segment_format.clone(),
            partial_segments_enabled: output.partial_segments_enabled,
            blocking_reload_enabled: output.blocking_reload_enabled,
            target_segment_duration_sec: output.target_segment_duration_sec,
            hold_back_segments: output.hold_back_segments,
            discontinuity_sequence: output.discontinuity_sequence,
            ladder_policy: output.ladder_policy.clone(),
            content_class: output.content_class.clone(),
            manifest_relative_path: output.manifest_relative_path.clone(),
            archive_relative_path: output.archive_relative_path.clone(),
            last_error: output.last_error.clone(),
            last_runtime_event_at: output.last_runtime_event_at.clone(),
            updated_at: output.updated_at.clone(),
        },
        advisory,
        artifact_health,
        expected_paths: LiveRuntimeSpecPaths {
            manifest_relative_path: manifest_relative_path.clone(),
            archive_relative_path: archive_relative_path.clone(),
            spec_relative_path: spec_relative_path.to_string(),
        },
        packaging: LiveRuntimePackagingSpec {
            runtime_class: output.runtime_class.clone(),
            latency_profile: output.latency_profile.clone(),
            playlist_mode: "event".to_string(),
            segment_format: output.segment_format.clone(),
            segment_duration_sec: output.target_segment_duration_sec,
            status: output.packaging_status.clone(),
            master_manifest_relative_path: manifest_relative_path,
            output_root_relative_path,
            live_edge_hold_back_segments: output.hold_back_segments,
            partial_segments_enabled: output.partial_segments_enabled,
            blocking_reload_enabled: output.blocking_reload_enabled,
            target_latency_ms: output.target_segment_duration_sec
                * output.hold_back_segments
                * 1000,
            variant_strategy: if variants.is_empty() {
                "awaiting_probe".to_string()
            } else {
                "probe_derived".to_string()
            },
            ladder_policy: output.ladder_policy.clone(),
            content_class: output.content_class.clone(),
            discontinuity_sequence: output.discontinuity_sequence,
            variants,
        },
        archive,
        collaboration,
        reconnect_policy: LiveRuntimeReconnectSpec {
            grace_window_sec: 20,
            session_ordinal,
            replacement_mode: "new_session_per_reconnect".to_string(),
            requires_discontinuity_on_reconnect: session.previous_session_id.is_some(),
        },
        health,
        telemetry: LiveRuntimeTelemetrySpec {
            heartbeat_sample_kind: "heartbeat".to_string(),
            runtime_report_sample_kind: "runtime_report".to_string(),
            repair_sample_kind: "runtime_repair".to_string(),
            reconciliation_sample_kinds: vec![
                "runtime_artifact_reconciled".to_string(),
                "runtime_archive_completed".to_string(),
                "session_state".to_string(),
            ],
        },
    })
}

fn build_live_runtime_archive_plan(
    session: &LiveIngestSession,
    output: &LiveRuntimeOutput,
    staging_relative_path: String,
    default_output_relative_path: String,
    collaboration: Option<&LiveRuntimeCollaborationSpec>,
) -> LiveRuntimeArchiveSpec {
    let derived_outputs = collaboration
        .map(|item| {
            item.outputs
                .iter()
                .filter(|route| route.output_kind == "archive" && route.recording_enabled)
                .filter_map(|route| collaboration_route_relative_path(session, route))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let (recording_mode, output_relative_path, output_relative_paths) =
        if let Some(item) = collaboration {
            if !derived_outputs.is_empty() {
                (
                    item.recording_policy.clone(),
                    derived_outputs[0].clone(),
                    derived_outputs,
                )
            } else {
                (
                    item.recording_policy.clone(),
                    default_output_relative_path.clone(),
                    vec![default_output_relative_path.clone()],
                )
            }
        } else {
            (
                "single_output".to_string(),
                default_output_relative_path.clone(),
                vec![default_output_relative_path.clone()],
            )
        };

    LiveRuntimeArchiveSpec {
        enabled: true,
        recording_mode,
        target_container: "mp4".to_string(),
        status: output.archive_status.clone(),
        staging_relative_path,
        output_relative_path,
        output_count: output_relative_paths.len() as i64,
        output_relative_paths,
    }
}
