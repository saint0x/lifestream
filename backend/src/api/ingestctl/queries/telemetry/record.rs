use super::*;
use crate::api::collab::fetch_active_collaboration_session_for_broadcast;
use crate::api::ingestctl::queries::fetch_live_runtime_targets_for_session;
use crate::api::ingestctl::build_live_runtime_advisory;
use crate::api::ingestctl::{
    canonical_live_runtime_archive_relative_path, canonical_live_runtime_manifest_relative_path,
};
use crate::models::LiveRuntimeTarget;

pub(crate) async fn record_live_runtime_telemetry(
    pool: &SqlitePool,
    session: &LiveIngestSession,
    sample_kind: &str,
    runtime_state: &str,
    packaging_status: &str,
    archive_status: &str,
    cpu_percent: Option<i64>,
    free_disk_gb: Option<f64>,
    detail: Value,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let output = fetch_live_runtime_output_for_session(pool, &session.id).await?;
    let targets = fetch_live_runtime_targets_for_session(pool, &session.id).await?;
    let collaboration =
        build_live_runtime_telemetry_collaboration(pool, &session.broadcast_id).await?;
    let normalized_detail = build_live_runtime_telemetry_detail(
        session,
        sample_kind,
        runtime_state,
        packaging_status,
        archive_status,
        cpu_percent,
        free_disk_gb,
        output.as_ref(),
        &targets,
        collaboration.as_ref(),
        detail,
    );
    sqlx::query(
        r#"
        INSERT INTO live_runtime_telemetry (
            id, session_id, creator_id, broadcast_id, sample_kind, runtime_state,
            packaging_status, archive_status, bitrate_kbps, viewers, dropped_frames,
            cpu_percent, free_disk_gb, detail_json, collected_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(format!("lrt-{}", Uuid::new_v4().simple()))
    .bind(&session.id)
    .bind(&session.creator_id)
    .bind(&session.broadcast_id)
    .bind(sample_kind)
    .bind(runtime_state)
    .bind(packaging_status)
    .bind(archive_status)
    .bind(session.bitrate_kbps)
    .bind(session.viewers)
    .bind(session.dropped_frames)
    .bind(cpu_percent)
    .bind(free_disk_gb)
    .bind(normalized_detail.to_string())
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Clone, Debug)]
struct LiveRuntimeTelemetryCollaboration {
    session_id: String,
    status: String,
    chat_mode: String,
    recording_policy: String,
    participant_count: i64,
    live_participant_count: i64,
    backstage_participant_count: i64,
    mirror_participant_count: i64,
    active_grant_count: i64,
    issued_grant_count: i64,
    active_pickup_count: i64,
    mix_minus_required: bool,
    audio_mix_mode: &'static str,
    active_route_count: i64,
    armed_archive_route_count: i64,
    shared_program_mirror_route_count: i64,
    guest_isolated_mirror_route_count: i64,
}

async fn build_live_runtime_telemetry_collaboration(
    pool: &SqlitePool,
    broadcast_id: &str,
) -> AppResult<Option<LiveRuntimeTelemetryCollaboration>> {
    let Some(session) =
        fetch_active_collaboration_session_for_broadcast(pool, broadcast_id).await?
    else {
        return Ok(None);
    };

    let participant_count = session.participants.len() as i64;
    let live_participant_count = session
        .participants
        .iter()
        .filter(|participant| participant.state == "live")
        .count() as i64;
    let backstage_participant_count = session
        .participants
        .iter()
        .filter(|participant| participant.state == "backstage")
        .count() as i64;
    let mirror_participant_count = session
        .participants
        .iter()
        .filter(|participant| participant.role != "host" && participant.mirror_to_guest_channel)
        .count() as i64;
    let mix_minus_required = session.participants.iter().any(|participant| {
        participant.role != "host" && participant.publish_to_host && participant.state == "live"
    });
    let active_grant_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM collaboration_mirror_grants WHERE session_id = ? AND state = 'active'",
    )
    .bind(&session.id)
    .fetch_one(pool)
    .await?;
    let issued_grant_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM collaboration_mirror_grants WHERE session_id = ? AND state = 'issued'",
    )
    .bind(&session.id)
    .fetch_one(pool)
    .await?;
    let active_pickup_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM collaboration_mirror_pickups WHERE session_id = ? AND state = 'active'",
    )
    .bind(&session.id)
    .fetch_one(pool)
    .await?;
    let shared_program_mirror_route_count = session
        .participants
        .iter()
        .filter(|participant| {
            participant.role != "host"
                && participant.mirror_to_guest_channel
                && participant.publish_to_host
                && participant.state == "live"
        })
        .count() as i64;
    let guest_isolated_mirror_route_count = session
        .participants
        .iter()
        .filter(|participant| {
            participant.role != "host"
                && participant.mirror_to_guest_channel
                && (!participant.publish_to_host || participant.state != "live")
        })
        .count() as i64;
    let host_route_active = 1_i64;
    let mirror_route_active = active_pickup_count;
    let active_route_count = host_route_active + mirror_route_active;
    let armed_archive_route_count = match session.recording_policy.as_str() {
        "host_archive" => 1,
        "split_archive" => 1 + mirror_participant_count,
        _ => 0,
    };

    Ok(Some(LiveRuntimeTelemetryCollaboration {
        session_id: session.id,
        status: session.status,
        chat_mode: session.chat_mode,
        recording_policy: session.recording_policy,
        participant_count,
        live_participant_count,
        backstage_participant_count,
        mirror_participant_count,
        active_grant_count,
        issued_grant_count,
        active_pickup_count,
        mix_minus_required,
        audio_mix_mode: if mix_minus_required {
            "mix_minus"
        } else {
            "program_only"
        },
        active_route_count,
        armed_archive_route_count,
        shared_program_mirror_route_count,
        guest_isolated_mirror_route_count,
    }))
}

fn build_live_runtime_telemetry_detail(
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

    if !detail.contains_key("manifestRelativePath") {
        detail.insert(
            "manifestRelativePath".to_string(),
            output
                .and_then(|item| item.manifest_relative_path.clone())
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
    }
    if !detail.contains_key("archiveRelativePath") {
        detail.insert(
            "archiveRelativePath".to_string(),
            output
                .and_then(|item| item.archive_relative_path.clone())
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
    }
    if !detail.contains_key("lastError") {
        detail.insert(
            "lastError".to_string(),
            output
                .and_then(|item| item.last_error.clone())
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
    }

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
        build_live_runtime_telemetry_artifact_detail(session, packaging_status, archive_status, output),
    );
    detail.insert(
        "runtimeOutput".to_string(),
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
        }),
    );
    detail.insert(
        "metrics".to_string(),
        json!({
            "bitrateKbps": session.bitrate_kbps,
            "viewers": session.viewers,
            "droppedFrames": session.dropped_frames,
            "cpuPercent": cpu_percent,
            "freeDiskGb": free_disk_gb,
        }),
    );
    detail.insert(
        "delivery".to_string(),
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
        }),
    );
    detail.insert(
        "collaboration".to_string(),
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
                })
            })
            .unwrap_or_else(|| json!({ "present": false })),
    );
    detail.insert(
        "targets".to_string(),
        json!({
            "count": targets.len(),
            "playbackEnabledCount": targets.iter().filter(|target| target.playback_enabled).count(),
            "recordingEnabledCount": targets.iter().filter(|target| target.recording_enabled).count(),
            "variantCount": targets.iter().filter(|target| target.target_kind == "variant").count(),
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
                .filter(|target| matches!(target.target_kind.as_str(), "host_channel" | "mirror_channel" | "archive"))
                .count(),
            "activeCount": targets.iter().filter(|target| target.route_state == "active").count(),
            "degradedCount": targets.iter().filter(|target| target.route_state == "degraded").count(),
            "armedCount": targets.iter().filter(|target| target.route_state == "armed").count(),
            "pendingSourceCount": targets
                .iter()
                .filter(|target| target.route_state == "pending_source")
                .count(),
            "kinds": targets.iter().map(|target| target.target_kind.clone()).collect::<Vec<_>>(),
            "states": targets.iter().map(|target| target.route_state.clone()).collect::<Vec<_>>(),
            "routes": targets
                .iter()
                .filter(|target| matches!(target.target_kind.as_str(), "host_channel" | "mirror_channel" | "archive"))
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
        }),
    );
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
