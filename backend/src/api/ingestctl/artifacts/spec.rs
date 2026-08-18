use super::*;
use crate::api::mirror::sync_active_collaboration_mirror_pickups_for_session_and_publish;
use crate::api::collab::fetch_active_collaboration_session_for_broadcast;
use crate::api::collaboration_runtime::build_collaboration_runtime_response_for_host;
use crate::api::ingestctl::{
    build_live_runtime_advisory, describe_live_runtime_artifact_health,
};
use crate::api::ingestctl::queries::canonical_live_runtime_spec_relative_path;
use crate::models::{
    CollaborationAudioRoute, CollaborationContributionAttachment, CollaborationOutputRoute,
    CollaborationProgramRoute, CollaborationTopologyMember, LiveRuntimeAdvisory,
    LiveRuntimeArtifactHealth, LiveRuntimeTarget, LiveSourceProbe, LiveSourceValidationReport,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveRuntimeSpecDocument {
    session: LiveRuntimeSpecSession,
    runtime: LiveRuntimeSpecRuntime,
    advisory: LiveRuntimeAdvisory,
    artifact_health: LiveRuntimeArtifactHealth,
    expected_paths: LiveRuntimeSpecPaths,
    packaging: LiveRuntimePackagingSpec,
    archive: LiveRuntimeArchiveSpec,
    collaboration: Option<LiveRuntimeCollaborationSpec>,
    reconnect_policy: LiveRuntimeReconnectSpec,
    health: LiveRuntimeHealthSpec,
    telemetry: LiveRuntimeTelemetrySpec,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveRuntimeSpecSession {
    id: String,
    creator_id: String,
    broadcast_id: String,
    previous_session_id: Option<String>,
    protocol: String,
    contribution_class: String,
    contribution_state: String,
    ingest_server: String,
    status: String,
    bitrate_kbps: i64,
    viewers: i64,
    dropped_frames: i64,
    ingest_latency_ms: Option<i64>,
    connected_at: String,
    last_heartbeat_at: String,
    disconnected_at: Option<String>,
    session_ordinal: i64,
    reconnect_session: bool,
    source_probe: Option<LiveSourceProbe>,
    source_validation: Option<LiveSourceValidationReport>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveRuntimeSpecRuntime {
    state: String,
    packaging_status: String,
    archive_status: String,
    runtime_class: String,
    latency_profile: String,
    segment_format: String,
    partial_segments_enabled: bool,
    blocking_reload_enabled: bool,
    target_segment_duration_sec: i64,
    hold_back_segments: i64,
    discontinuity_sequence: i64,
    ladder_policy: String,
    content_class: String,
    manifest_relative_path: Option<String>,
    archive_relative_path: Option<String>,
    last_error: Option<String>,
    last_runtime_event_at: String,
    updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveRuntimeSpecPaths {
    manifest_relative_path: String,
    archive_relative_path: String,
    spec_relative_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveRuntimePackagingSpec {
    runtime_class: String,
    latency_profile: String,
    playlist_mode: String,
    segment_format: String,
    segment_duration_sec: i64,
    status: String,
    master_manifest_relative_path: String,
    output_root_relative_path: String,
    live_edge_hold_back_segments: i64,
    partial_segments_enabled: bool,
    blocking_reload_enabled: bool,
    target_latency_ms: i64,
    variant_strategy: String,
    ladder_policy: String,
    content_class: String,
    discontinuity_sequence: i64,
    variants: Vec<LiveRuntimeVariantSpec>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LiveRuntimeVariantSpec {
    pub(super) label: String,
    pub(super) width: i64,
    pub(super) height: i64,
    pub(super) video_bitrate_bps: i64,
    pub(super) bandwidth_bps: i64,
    pub(super) output_relative_dir: String,
    pub(super) relative_playlist_path: String,
    pub(super) segment_relative_pattern: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveRuntimeArchiveSpec {
    enabled: bool,
    recording_mode: String,
    target_container: String,
    status: String,
    staging_relative_path: String,
    output_relative_path: String,
    output_count: i64,
    output_relative_paths: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveRuntimeCollaborationSpec {
    session_id: String,
    status: String,
    source_broadcast_id: String,
    chat_mode: String,
    recording_policy: String,
    shared_chat: bool,
    mix_minus_required: bool,
    audio_mix_mode: String,
    connected_participants: i64,
    recording_owner_creator_id: Option<String>,
    host_output_participant_ids: Vec<String>,
    mirrored_creator_ids: Vec<String>,
    contributions: Vec<CollaborationContributionAttachment>,
    outputs: Vec<CollaborationOutputRoute>,
    programs: Vec<CollaborationProgramRoute>,
    audio: Vec<CollaborationAudioRoute>,
    members: Vec<CollaborationTopologyMember>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveRuntimeReconnectSpec {
    grace_window_sec: i64,
    session_ordinal: i64,
    replacement_mode: String,
    requires_discontinuity_on_reconnect: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveRuntimeHealthSpec {
    status: String,
    current_cpu_percent: Option<i64>,
    current_free_disk_gb: Option<f64>,
    current_ingest_latency_ms: Option<i64>,
    current_dropped_frames: i64,
    cpu_warn_percent: i64,
    cpu_critical_percent: i64,
    free_disk_warn_gb: f64,
    free_disk_critical_gb: f64,
    ingest_latency_warn_ms: i64,
    ingest_latency_critical_ms: i64,
    dropped_frames_warn: i64,
    dropped_frames_critical: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveRuntimeTelemetrySpec {
    heartbeat_sample_kind: String,
    runtime_report_sample_kind: String,
    repair_sample_kind: String,
    reconciliation_sample_kinds: Vec<String>,
}

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

async fn sync_runtime_target_dependents(
    state: &SharedState,
    session: &LiveIngestSession,
) -> AppResult<()> {
    let Some(collaboration_session) =
        fetch_active_collaboration_session_for_broadcast(&state.pool, &session.broadcast_id)
            .await?
    else {
        return Ok(());
    };

    sync_active_collaboration_mirror_pickups_for_session_and_publish(
        state,
        &collaboration_session.id,
    )
    .await
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

async fn build_live_runtime_collaboration_spec(
    state: &SharedState,
    session: &LiveIngestSession,
) -> AppResult<Option<LiveRuntimeCollaborationSpec>> {
    let Some(collaboration_session) =
        fetch_active_collaboration_session_for_broadcast(&state.pool, &session.broadcast_id)
            .await?
    else {
        return Ok(None);
    };
    let runtime =
        build_collaboration_runtime_response_for_host(&state.pool, collaboration_session).await?;
    let topology = runtime.topology;

    Ok(Some(LiveRuntimeCollaborationSpec {
        session_id: runtime.session.id,
        status: runtime.session.status,
        source_broadcast_id: runtime.session.source_broadcast_id,
        chat_mode: runtime.session.chat_mode,
        recording_policy: runtime.session.recording_policy,
        shared_chat: topology.shared_chat,
        mix_minus_required: topology.mix_minus_required,
        audio_mix_mode: if topology.mix_minus_required {
            "mix_minus".to_string()
        } else {
            "program_only".to_string()
        },
        connected_participants: topology.connected_participants,
        recording_owner_creator_id: topology.recording_owner_creator_id,
        host_output_participant_ids: topology.host_output_participant_ids,
        mirrored_creator_ids: topology.mirrored_creator_ids,
        contributions: topology.contributions,
        outputs: topology.outputs,
        programs: topology.programs,
        audio: topology.audio,
        members: topology.members,
    }))
}

pub(super) fn collaboration_route_relative_path(
    session: &LiveIngestSession,
    route: &CollaborationOutputRoute,
) -> Option<String> {
    match route.output_kind.as_str() {
        "host_channel" => Some(canonical_live_runtime_manifest_relative_path(session)),
        "mirror_channel" => route
            .target_creator_id
            .as_ref()
            .zip(route.target_broadcast_id.as_ref())
            .map(|(creator_id, broadcast_id)| {
                format!("live/{creator_id}/{broadcast_id}/{}/master.m3u8", route.id)
            }),
        "archive" => route
            .target_creator_id
            .as_ref()
            .zip(route.target_broadcast_id.as_ref())
            .map(|(creator_id, broadcast_id)| {
                format!("archive/{creator_id}/{broadcast_id}/{}/final.mp4", route.id)
            }),
        _ => route
            .target_creator_id
            .as_ref()
            .zip(route.target_broadcast_id.as_ref())
            .map(|(creator_id, broadcast_id)| {
                format!("runtime/{creator_id}/{broadcast_id}/{}/target", route.id)
            }),
    }
}

fn build_live_runtime_health_spec(
    session: &LiveIngestSession,
    current_cpu_percent: Option<i64>,
    current_free_disk_gb: Option<f64>,
) -> LiveRuntimeHealthSpec {
    const CPU_WARN_PERCENT: i64 = 85;
    const CPU_CRITICAL_PERCENT: i64 = 95;
    const FREE_DISK_WARN_GB: f64 = 20.0;
    const FREE_DISK_CRITICAL_GB: f64 = 5.0;
    const INGEST_LATENCY_WARN_MS: i64 = 1500;
    const INGEST_LATENCY_CRITICAL_MS: i64 = 3000;
    const DROPPED_FRAMES_WARN: i64 = 100;
    const DROPPED_FRAMES_CRITICAL: i64 = 1000;

    let status = if current_cpu_percent.is_some_and(|value| value >= CPU_CRITICAL_PERCENT)
        || current_free_disk_gb.is_some_and(|value| value <= FREE_DISK_CRITICAL_GB)
        || session
            .ingest_latency_ms
            .is_some_and(|value| value >= INGEST_LATENCY_CRITICAL_MS)
        || session.dropped_frames >= DROPPED_FRAMES_CRITICAL
    {
        "critical"
    } else if current_cpu_percent.is_some_and(|value| value >= CPU_WARN_PERCENT)
        || current_free_disk_gb.is_some_and(|value| value <= FREE_DISK_WARN_GB)
        || session
            .ingest_latency_ms
            .is_some_and(|value| value >= INGEST_LATENCY_WARN_MS)
        || session.dropped_frames >= DROPPED_FRAMES_WARN
    {
        "warn"
    } else {
        "ok"
    };

    LiveRuntimeHealthSpec {
        status: status.to_string(),
        current_cpu_percent,
        current_free_disk_gb,
        current_ingest_latency_ms: session.ingest_latency_ms,
        current_dropped_frames: session.dropped_frames,
        cpu_warn_percent: CPU_WARN_PERCENT,
        cpu_critical_percent: CPU_CRITICAL_PERCENT,
        free_disk_warn_gb: FREE_DISK_WARN_GB,
        free_disk_critical_gb: FREE_DISK_CRITICAL_GB,
        ingest_latency_warn_ms: INGEST_LATENCY_WARN_MS,
        ingest_latency_critical_ms: INGEST_LATENCY_CRITICAL_MS,
        dropped_frames_warn: DROPPED_FRAMES_WARN,
        dropped_frames_critical: DROPPED_FRAMES_CRITICAL,
    }
}

pub(super) fn build_live_runtime_variant_specs(
    session: &LiveIngestSession,
    output: &LiveRuntimeOutput,
) -> AppResult<Vec<LiveRuntimeVariantSpec>> {
    let Some(source_probe) = session.source_probe.as_ref() else {
        return Ok(Vec::new());
    };
    let Some(width) = source_probe.width else {
        return Ok(Vec::new());
    };
    let Some(height) = source_probe.height else {
        return Ok(Vec::new());
    };

    let probed = ProbedMedia {
        container_format: source_probe.container_format.clone(),
        duration_sec: 0.0,
        width: Some(width),
        height: Some(height),
        frame_rate: source_probe.frame_rate,
        video_codec: source_probe.video_codec.clone(),
        audio_codec: source_probe.audio_codec.clone(),
        audio_sample_rate_hz: source_probe.audio_sample_rate_hz,
        audio_channels: source_probe.audio_channels,
        has_video: true,
        has_audio: source_probe.audio_codec.is_some(),
        bitrate_bps: session.bitrate_kbps.checked_mul(1000),
        audio_streams: if source_probe.audio_codec.is_some() {
            vec![ProbedAudioStream {
                stream_index: 1,
                codec: source_probe.audio_codec.clone(),
                language: Some("und".to_string()),
                sample_rate_hz: source_probe.audio_sample_rate_hz,
                channels: source_probe.audio_channels,
            }]
        } else {
            Vec::new()
        },
        subtitle_streams: Vec::new(),
    };

    let refined = refine_live_variant_plans(
        plan_hls_variants(&probed)?,
        session,
        output.runtime_class.as_str(),
        output.ladder_policy.as_str(),
    );
    Ok(refined
            .into_iter()
            .map(|plan| {
                live_runtime_variant_spec_from_plan(session, plan, &output.segment_format)
            })
            .collect()
    )
}

fn refine_live_variant_plans(
    mut plans: Vec<HlsVariantPlan>,
    session: &LiveIngestSession,
    runtime_class: &str,
    ladder_policy: &str,
) -> Vec<HlsVariantPlan> {
    let handheld_profile = runtime_class == "ll_hls";
    if handheld_profile {
        plans.retain(|plan| plan.height <= 720);
    }
    if ladder_policy.contains("general_sd") {
        plans.retain(|plan| plan.height <= 480);
    }
    if ladder_policy.contains("cinematic") {
        plans.retain(|plan| plan.height >= 360);
    }
    if plans.is_empty() {
        return plans;
    }

    let source_bitrate_bps = session.bitrate_kbps.saturating_mul(1000);
    let device_multiplier = if handheld_profile { 0.82 } else { 1.0 };

    for plan in &mut plans {
        let content_multiplier = if ladder_policy.contains("high_motion") {
            match plan.height {
                0..=240 => 0.90,
                241..=360 => 0.96,
                361..=480 => 1.00,
                481..=720 => 1.10,
                _ => 1.18,
            }
        } else if ladder_policy.contains("cinematic") {
            match plan.height {
                0..=360 => 0.82,
                361..=480 => 0.90,
                481..=720 => 1.00,
                _ => 1.08,
            }
        } else if ladder_policy.contains("general_sd") {
            match plan.height {
                0..=240 => 0.72,
                241..=360 => 0.82,
                _ => 0.90,
            }
        } else {
            match plan.height {
                0..=240 => 0.76,
                241..=360 => 0.86,
                361..=480 => 0.94,
                481..=720 => 1.00,
                _ => 1.04,
            }
        };

        let tuned_video_bitrate = ((plan.video_bitrate_bps as f64)
            * content_multiplier
            * device_multiplier)
            .round() as i64;
        let bounded_video_bitrate = if source_bitrate_bps > 0 {
            tuned_video_bitrate.min((source_bitrate_bps as f64 * 0.92).round() as i64)
        } else {
            tuned_video_bitrate
        };
        let floor_video_bitrate = match plan.height {
            0..=240 => 350_000,
            241..=360 => 600_000,
            361..=480 => 1_000_000,
            481..=720 => 2_200_000,
            _ => 3_500_000,
        };
        plan.video_bitrate_bps = bounded_video_bitrate.max(floor_video_bitrate);
        let audio_bitrate_bps = match plan.height {
            0..=360 => 96_000,
            361..=720 => 128_000,
            _ => 192_000,
        };
        plan.bandwidth_bps = plan.video_bitrate_bps + audio_bitrate_bps;
    }

    plans
}

fn live_runtime_variant_spec_from_plan(
    session: &LiveIngestSession,
    plan: HlsVariantPlan,
    segment_format: &str,
) -> LiveRuntimeVariantSpec {
    let segment_extension = if segment_format == "fmp4" { "m4s" } else { "ts" };
    LiveRuntimeVariantSpec {
        label: plan.label.clone(),
        width: plan.width,
        height: plan.height,
        video_bitrate_bps: plan.video_bitrate_bps,
        bandwidth_bps: plan.bandwidth_bps,
        output_relative_dir: format!(
            "live/{}/{}/{}/{}",
            session.creator_id, session.broadcast_id, session.id, plan.label
        ),
        relative_playlist_path: format!(
            "live/{}/{}/{}/{}/playlist.m3u8",
            session.creator_id, session.broadcast_id, session.id, plan.label
        ),
        segment_relative_pattern: format!(
            "live/{}/{}/{}/{}/segment_%03d.{}",
            session.creator_id, session.broadcast_id, session.id, plan.label, segment_extension
        ),
    }
}
