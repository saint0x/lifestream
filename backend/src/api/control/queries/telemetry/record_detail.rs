use super::record_collab::LiveRuntimeTelemetryCollaboration;
use super::*;
use crate::api::control::build_live_runtime_advisory;
use crate::api::control::{
    canonical_live_runtime_archive_relative_path, canonical_live_runtime_manifest_relative_path,
};
use crate::models::LiveRuntimeTarget;

pub(super) fn build_live_runtime_telemetry_detail(
    session: &LiveIngestSession,
    sample_kind: &str,
    runtime_state: &str,
    packaging_status: &str,
    archive_status: &str,
    cpu_percent: Option<i64>,
    free_disk_gb: Option<f64>,
    output: Option<&LiveRuntimeOutput>,
    targets: &[LiveRuntimeTarget],
    collaboration: Option<&LiveRuntimeTelemetryCollaboration>,
    reported: Value,
) -> Value {
    let mut detail = match reported {
        Value::Object(map) => map,
        other => {
            let mut map = serde_json::Map::new();
            map.insert("reportedValue".to_string(), other);
            map
        }
    };

    maybe_insert_output_path(
        &mut detail,
        "manifestRelativePath",
        output.and_then(|item| item.manifest_relative_path.clone()),
    );
    maybe_insert_output_path(
        &mut detail,
        "archiveRelativePath",
        output.and_then(|item| item.archive_relative_path.clone()),
    );
    maybe_insert_output_path(
        &mut detail,
        "lastError",
        output.and_then(|item| item.last_error.clone()),
    );

    detail.insert(
        "session".to_string(),
        json!({
            "sampleKind": sample_kind,
            "protocol": session.protocol.clone(),
            "contributionClass": session.contribution_class.clone(),
            "contributionState": session.contribution_state.clone(),
            "reconnectSession": session.previous_session_id.is_some(),
            "previousSessionId": session.previous_session_id.clone(),
            "status": session.status.clone(),
            "ingestServer": session.ingest_server.clone(),
            "ingestLatencyMs": session.ingest_latency_ms,
            "sourceProbePresent": session.source_probe.is_some(),
            "sourceProbe": session.source_probe.clone(),
            "sourceValidation": session.source_validation.clone(),
        }),
    );
    detail.insert(
        "advisory".to_string(),
        json!(build_live_runtime_advisory(Some(session), output, None)),
    );
    detail.insert(
        "artifacts".to_string(),
        build_live_runtime_telemetry_artifact_detail(
            session,
            packaging_status,
            archive_status,
            output,
        ),
    );
    detail.insert(
        "runtimeOutput".to_string(),
        build_runtime_output_detail(runtime_state, packaging_status, archive_status, output),
    );
    detail.insert(
        "metrics".to_string(),
        build_metric_detail(session, cpu_percent, free_disk_gb),
    );
    detail.insert("delivery".to_string(), build_delivery_detail(output));
    detail.insert(
        "collaboration".to_string(),
        build_collaboration_detail(collaboration),
    );
    detail.insert("targets".to_string(), build_target_detail(targets));
    detail.insert(
        "outputs".to_string(),
        json!({
            "activeRouteCount": collaboration.map(|item| item.active_route_count).unwrap_or(0),
            "armedArchiveRouteCount": collaboration
                .map(|item| item.armed_archive_route_count)
                .unwrap_or(0),
            "playbackReady": matches!(packaging_status, "ready" | "complete")
                && output.and_then(|item| item.manifest_relative_path.as_ref()).is_some(),
            "archiveReady": archive_status == "complete"
                && output.and_then(|item| item.archive_relative_path.as_ref()).is_some(),
        }),
    );

    Value::Object(detail)
}

fn maybe_insert_output_path(
    detail: &mut serde_json::Map<String, Value>,
    key: &str,
    value: Option<String>,
) {
    if !detail.contains_key(key) {
        detail.insert(
            key.to_string(),
            value.map(Value::String).unwrap_or(Value::Null),
        );
    }
}

fn build_runtime_output_detail(
    runtime_state: &str,
    packaging_status: &str,
    archive_status: &str,
    output: Option<&LiveRuntimeOutput>,
) -> Value {
    json!({
        "state": runtime_state,
        "packagingStatus": packaging_status,
        "archiveStatus": archive_status,
        "runtimeClass": output.map(|item| item.runtime_class.clone()),
        "latencyProfile": output.map(|item| item.latency_profile.clone()),
        "segmentFormat": output.map(|item| item.segment_format.clone()),
        "partialSegmentsEnabled": output.map(|item| item.partial_segments_enabled),
        "blockingReloadEnabled": output.map(|item| item.blocking_reload_enabled),
        "targetSegmentDurationSec": output.map(|item| item.target_segment_duration_sec),
        "holdBackSegments": output.map(|item| item.hold_back_segments),
        "discontinuitySequence": output.map(|item| item.discontinuity_sequence),
        "ladderPolicy": output.map(|item| item.ladder_policy.clone()),
        "contentClass": output.map(|item| item.content_class.clone()),
        "manifestRelativePath": output.and_then(|item| item.manifest_relative_path.clone()),
        "archiveRelativePath": output.and_then(|item| item.archive_relative_path.clone()),
        "lastError": output.and_then(|item| item.last_error.clone()),
        "updatedAt": output.map(|item| item.updated_at.clone()),
    })
}

fn build_metric_detail(
    session: &LiveIngestSession,
    cpu_percent: Option<i64>,
    free_disk_gb: Option<f64>,
) -> Value {
    json!({
        "bitrateKbps": session.bitrate_kbps,
        "viewers": session.viewers,
        "droppedFrames": session.dropped_frames,
        "cpuPercent": cpu_percent,
        "freeDiskGb": free_disk_gb,
    })
}

fn build_delivery_detail(output: Option<&LiveRuntimeOutput>) -> Value {
    json!({
        "runtimeClass": output.map(|item| item.runtime_class.clone()),
        "latencyProfile": output.map(|item| item.latency_profile.clone()),
        "segmentFormat": output.map(|item| item.segment_format.clone()),
        "partialSegmentsEnabled": output.map(|item| item.partial_segments_enabled).unwrap_or(false),
        "blockingReloadEnabled": output.map(|item| item.blocking_reload_enabled).unwrap_or(false),
        "targetSegmentDurationSec": output.map(|item| item.target_segment_duration_sec),
        "holdBackSegments": output.map(|item| item.hold_back_segments),
        "discontinuitySequence": output.map(|item| item.discontinuity_sequence).unwrap_or(0),
        "ladderPolicy": output.map(|item| item.ladder_policy.clone()),
        "contentClass": output.map(|item| item.content_class.clone()),
    })
}

fn build_collaboration_detail(collaboration: Option<&LiveRuntimeTelemetryCollaboration>) -> Value {
    collaboration
        .map(|item| {
            json!({
                "present": true,
                "sessionId": item.session_id.clone(),
                "status": item.status.clone(),
                "chatMode": item.chat_mode.clone(),
                "recordingPolicy": item.recording_policy.clone(),
                "participantCount": item.participant_count,
                "liveParticipantCount": item.live_participant_count,
                "backstageParticipantCount": item.backstage_participant_count,
                "mirrorParticipantCount": item.mirror_participant_count,
                "activeGrantCount": item.active_grant_count,
                "issuedGrantCount": item.issued_grant_count,
                "activePickupCount": item.active_pickup_count,
                "mixMinusRequired": item.mix_minus_required,
                "audioMixMode": item.audio_mix_mode,
                "sharedProgramMirrorRouteCount": item.shared_program_mirror_route_count,
                "guestIsolatedMirrorRouteCount": item.guest_isolated_mirror_route_count,
                "engineNodeCount": item.engine_node_count,
                "engineEdgeCount": item.engine_edge_count,
                "mixMinusEdgeCount": item.mix_minus_edge_count,
                "mirrorFanoutEdgeCount": item.mirror_fanout_edge_count,
                "bundleAttachmentCount": item.bundle_attachment_count,
                "bundleMixerCount": item.bundle_mixer_count,
                "bundleFanoutCount": item.bundle_fanout_count,
                "bundleReturnCount": item.bundle_return_count,
                "mediaStageCount": item.media_stage_count,
                "mediaOutputTargetCount": item.media_output_target_count,
                "mediaReturnTargetCount": item.media_return_target_count,
                "mediaInputParticipantCount": item.media_input_participant_count,
                "mediaMixMinusParticipantCount": item.media_mix_minus_participant_count,
            })
        })
        .unwrap_or_else(|| json!({ "present": false }))
}

fn build_target_detail(targets: &[LiveRuntimeTarget]) -> Value {
    json!({
        "count": targets.len(),
        "playbackEnabledCount": targets.iter().filter(|target| target.playback_enabled).count(),
        "recordingEnabledCount": targets.iter().filter(|target| target.recording_enabled).count(),
        "variantCount": targets.iter().filter(|target| target.target_kind == "variant").count(),
        "programCount": targets.iter().filter(|target| target.target_kind == "program").count(),
        "audioCount": targets.iter().filter(|target| target.target_kind == "audio").count(),
        "engineCount": targets.iter().filter(|target| target.target_kind == "engine").count(),
        "hostChannelCount": targets.iter().filter(|target| target.target_kind == "host_channel").count(),
        "mirrorChannelCount": targets.iter().filter(|target| target.target_kind == "mirror_channel").count(),
        "sharedProgramMirrorChannelCount": targets
            .iter()
            .filter(|target| {
                target.target_kind == "mirror_channel"
                    && target.mix_minus_required
                    && target.source_participant_ids.len() > 1
            })
            .count(),
        "guestIsolatedMirrorChannelCount": targets
            .iter()
            .filter(|target| {
                target.target_kind == "mirror_channel"
                    && !(target.mix_minus_required && target.source_participant_ids.len() > 1)
            })
            .count(),
        "archiveCount": targets.iter().filter(|target| target.target_kind == "archive").count(),
        "collaborationCount": targets
            .iter()
            .filter(|target| matches!(target.target_kind.as_str(), "host_channel" | "mirror_channel" | "archive" | "program" | "audio" | "engine"))
            .count(),
        "activeCount": targets.iter().filter(|target| target.route_state == "active").count(),
        "degradedCount": targets.iter().filter(|target| target.route_state == "degraded").count(),
        "armedCount": targets.iter().filter(|target| target.route_state == "armed").count(),
        "pendingSourceCount": targets.iter().filter(|target| target.route_state == "pending_source").count(),
        "kinds": targets.iter().map(|target| target.target_kind.clone()).collect::<Vec<_>>(),
        "states": targets.iter().map(|target| target.route_state.clone()).collect::<Vec<_>>(),
        "routes": targets
            .iter()
            .filter(|target| matches!(target.target_kind.as_str(), "host_channel" | "mirror_channel" | "archive" | "program" | "audio" | "engine"))
            .map(|target| {
                json!({
                    "kind": target.target_kind,
                    "key": target.target_key,
                    "state": target.route_state,
                    "playbackEnabled": target.playback_enabled,
                    "recordingEnabled": target.recording_enabled,
                    "mixMinusRequired": target.mix_minus_required,
                    "targetCreatorId": target.target_creator_id,
                    "targetBroadcastId": target.target_broadcast_id,
                    "relativePath": target.relative_path,
                    "sourceParticipantIds": target.source_participant_ids,
                })
            })
            .collect::<Vec<_>>(),
    })
}

fn build_live_runtime_telemetry_artifact_detail(
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
