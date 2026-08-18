use super::*;
use crate::api::ingestctl::{
    build_live_runtime_advisory, describe_live_runtime_artifact_health,
};
use crate::api::ingestctl::queries::canonical_live_runtime_spec_relative_path;
use crate::models::LiveRuntimeTarget;

mod collab;
mod doc;
mod health;
mod variant;

use collab::{build_live_runtime_collaboration_spec, sync_runtime_target_dependents};
use doc::{
    LiveRuntimeArchiveSpec, LiveRuntimeCollaborationSpec, LiveRuntimePackagingSpec,
    LiveRuntimeReconnectSpec, LiveRuntimeSpecDocument, LiveRuntimeSpecPaths,
    LiveRuntimeSpecRuntime, LiveRuntimeSpecSession, LiveRuntimeTelemetrySpec,
};
use health::build_live_runtime_health_spec;
pub(in crate::api::ingestctl::artifacts) use collab::{
    collaboration_audio_relative_path, collaboration_engine_relative_path,
    collaboration_program_relative_path, collaboration_route_relative_path,
};
pub(in crate::api::ingestctl::artifacts) use variant::{
    LiveRuntimeVariantSpec, build_live_runtime_variant_specs,
};

pub(crate) async fn provision_live_runtime_workspace(
    state: &SharedState,
    session: &LiveIngestSession,
) -> AppResult<String> {
    let manifest_path = media_path_for_relative(
        state,
        &canonical_live_runtime_manifest_relative_path(session),
    );
    let archive_path = media_path_for_relative(
        state,
        &canonical_live_runtime_archive_relative_path(session),
    );
    let archive_staging_path = media_path_for_relative(
        state,
        &canonical_live_runtime_archive_staging_relative_path(session),
    );
    let spec_relative_path = canonical_live_runtime_spec_relative_path(session);
    let spec_path = media_path_for_relative(state, &spec_relative_path);

    ensure_parent_dir(&manifest_path).await?;
    ensure_parent_dir(&archive_path).await?;
    ensure_parent_dir(&archive_staging_path).await?;
    ensure_parent_dir(&spec_path).await?;
    let output = fetch_live_runtime_output_for_session(&state.pool, &session.id).await?;
    let variant_output = output.as_ref().ok_or_else(|| {
        AppError::Internal("missing live runtime output while provisioning workspace".to_string())
    })?;
    for variant in build_live_runtime_variant_specs(session, variant_output)? {
        let playlist_path = media_path_for_relative(state, &variant.relative_playlist_path);
        ensure_parent_dir(&playlist_path).await?;
    }

    Ok(spec_relative_path)
}

pub(crate) async fn persist_live_runtime_spec(
    state: &SharedState,
    session: &LiveIngestSession,
) -> AppResult<String> {
    let spec_relative_path = provision_live_runtime_workspace(state, session).await?;
    let output = fetch_live_runtime_output_for_session(&state.pool, &session.id)
        .await?
        .ok_or_else(|| {
            AppError::Internal("missing live runtime output while persisting spec".to_string())
        })?;
    let spec_path = media_path_for_relative(state, &spec_relative_path);

    let spec = build_live_runtime_spec(state, session, &output, &spec_relative_path).await?;
    let target_sync = sync_live_runtime_targets(
        &state.pool,
        session,
        &build_live_runtime_targets(session, &spec, &output),
    )
    .await?;
    if target_sync.created > 0 || target_sync.updated > 0 || target_sync.removed > 0 {
        write_live_ingest_event(
            &state.pool,
            &session.id,
            &session.creator_id,
            &session.broadcast_id,
            "runtime_targets_synced",
            json!({
                "created": target_sync.created,
                "updated": target_sync.updated,
                "removed": target_sync.removed,
                "runtimeState": output.runtime_state,
                "packagingStatus": output.packaging_status,
                "archiveStatus": output.archive_status,
            }),
        )
        .await?;
        sync_runtime_target_dependents(state, session).await?;
    }

    tokio::fs::write(
        &spec_path,
        serde_json::to_vec_pretty(&spec).map_err(|error| AppError::Internal(error.to_string()))?,
    )
    .await
    .map_err(AppError::Io)?;

    Ok(spec_relative_path)
}

async fn build_live_runtime_spec(
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
            target_latency_ms: output.target_segment_duration_sec * output.hold_back_segments * 1000,
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

fn build_live_runtime_targets(
    session: &LiveIngestSession,
    spec: &LiveRuntimeSpecDocument,
    output: &LiveRuntimeOutput,
) -> Vec<LiveRuntimeTarget> {
    let now = Utc::now().to_rfc3339();
    let mut targets = Vec::new();

    for variant in &spec.packaging.variants {
        targets.push(LiveRuntimeTarget {
            id: format!("lrt-variant-{}-{}", session.id, variant.label),
            session_id: session.id.clone(),
            creator_id: session.creator_id.clone(),
            broadcast_id: session.broadcast_id.clone(),
            target_kind: "variant".to_string(),
            target_key: variant.label.clone(),
            target_label: variant.label.clone(),
            route_state: output.packaging_status.clone(),
            target_creator_id: Some(session.creator_id.clone()),
            target_broadcast_id: Some(session.broadcast_id.clone()),
            playback_enabled: matches!(output.packaging_status.as_str(), "ready" | "complete"),
            recording_enabled: false,
            mix_minus_required: false,
            relative_path: Some(variant.relative_playlist_path.clone()),
            source_participant_ids: Vec::new(),
            created_at: now.clone(),
            updated_at: now.clone(),
        });
    }

    if spec.collaboration.is_none() {
        targets.push(LiveRuntimeTarget {
            id: format!("lrt-archive-{}", session.id),
            session_id: session.id.clone(),
            creator_id: session.creator_id.clone(),
            broadcast_id: session.broadcast_id.clone(),
            target_kind: "archive".to_string(),
            target_key: "primary".to_string(),
            target_label: "primary archive".to_string(),
            route_state: output.archive_status.clone(),
            target_creator_id: Some(session.creator_id.clone()),
            target_broadcast_id: Some(session.broadcast_id.clone()),
            playback_enabled: false,
            recording_enabled: spec.archive.enabled,
            mix_minus_required: false,
            relative_path: Some(spec.archive.output_relative_path.clone()),
            source_participant_ids: Vec::new(),
            created_at: now.clone(),
            updated_at: now.clone(),
        });
    }

    if let Some(collaboration) = spec.collaboration.as_ref() {
        for route in &collaboration.outputs {
            targets.push(LiveRuntimeTarget {
                id: format!("lrt-route-{}", route.id),
                session_id: session.id.clone(),
                creator_id: session.creator_id.clone(),
                broadcast_id: session.broadcast_id.clone(),
                target_kind: route.output_kind.clone(),
                target_key: route.id.clone(),
                target_label: route.output_kind.replace('_', " "),
                route_state: route.route_state.clone(),
                target_creator_id: route.target_creator_id.clone(),
                target_broadcast_id: route.target_broadcast_id.clone(),
                playback_enabled: route.playback_enabled,
                recording_enabled: route.recording_enabled,
                mix_minus_required: route.mix_minus_required,
                relative_path: collaboration_route_relative_path(session, route),
                source_participant_ids: route.source_participant_ids.clone(),
                created_at: now.clone(),
                updated_at: now.clone(),
            });
        }
        for program in &collaboration.programs {
            targets.push(LiveRuntimeTarget {
                id: format!("lrt-program-{}", program.id),
                session_id: session.id.clone(),
                creator_id: session.creator_id.clone(),
                broadcast_id: session.broadcast_id.clone(),
                target_kind: "program".to_string(),
                target_key: program.id.clone(),
                target_label: program.program_kind.replace('_', " "),
                route_state: program.route_state.clone(),
                target_creator_id: program.target_creator_id.clone(),
                target_broadcast_id: program.target_broadcast_id.clone(),
                playback_enabled: program.playback_enabled,
                recording_enabled: program.recording_enabled,
                mix_minus_required: program.mix_minus_required,
                relative_path: Some(collaboration_program_relative_path(session, program)),
                source_participant_ids: program.source_participant_ids.clone(),
                created_at: now.clone(),
                updated_at: now.clone(),
            });
        }
        for audio in &collaboration.audio {
            targets.push(LiveRuntimeTarget {
                id: format!("lrt-audio-{}-{}", session.id, audio.participant_id),
                session_id: session.id.clone(),
                creator_id: session.creator_id.clone(),
                broadcast_id: session.broadcast_id.clone(),
                target_kind: "audio".to_string(),
                target_key: audio.participant_id.clone(),
                target_label: format!("audio {}", audio.route_kind.replace('_', " ")),
                route_state: audio.route_state.clone(),
                target_creator_id: audio.creator_id.clone(),
                target_broadcast_id: Some(session.broadcast_id.clone()),
                playback_enabled: audio.receive_program_audio,
                recording_enabled: false,
                mix_minus_required: audio.mix_minus_required,
                relative_path: Some(collaboration_audio_relative_path(session, audio)),
                source_participant_ids: audio.upstream_participant_ids.clone(),
                created_at: now.clone(),
                updated_at: now.clone(),
            });
        }
        targets.push(LiveRuntimeTarget {
            id: format!("lrt-engine-{}", session.id),
            session_id: session.id.clone(),
            creator_id: session.creator_id.clone(),
            broadcast_id: session.broadcast_id.clone(),
            target_kind: "engine".to_string(),
            target_key: collaboration.engine.execution_mode.clone(),
            target_label: "collaboration engine".to_string(),
            route_state: if collaboration.engine.edges.is_empty() {
                "inactive".to_string()
            } else if collaboration
                .engine
                .nodes
                .iter()
                .any(|node| node.route_state == "degraded")
            {
                "degraded".to_string()
            } else if collaboration
                .engine
                .nodes
                .iter()
                .any(|node| matches!(node.route_state.as_str(), "active" | "live" | "attached"))
            {
                "active".to_string()
            } else {
                "armed".to_string()
            },
            target_creator_id: Some(session.creator_id.clone()),
            target_broadcast_id: Some(session.broadcast_id.clone()),
            playback_enabled: false,
            recording_enabled: false,
            mix_minus_required: collaboration.mix_minus_required,
            relative_path: Some(collaboration_engine_relative_path(session)),
            source_participant_ids: collaboration
                .contributions
                .iter()
                .map(|item| item.participant_id.clone())
                .collect(),
            created_at: now.clone(),
            updated_at: now.clone(),
        });
    }

    targets
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
