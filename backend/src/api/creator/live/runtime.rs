use super::*;
use crate::api::control::{
    apply_collaboration_transport_gap, build_live_runtime_advisory,
    collaboration_transport_gap_from_topology, describe_declared_live_runtime_artifact_health,
    fetch_live_ingest_events_for_session, fetch_live_runtime_output_for_session,
    fetch_live_runtime_targets_for_session, fetch_live_runtime_telemetry_for_session,
    fetch_live_runtime_telemetry_summary, fetch_live_runtime_telemetry_summary_for_session,
    fetch_recent_live_runtime_targets, fetch_recent_live_runtime_telemetry,
};
use crate::api::presence::{
    reconcile_stale_creator_live_socket_sessions_for_read,
    reconcile_stale_creator_live_socket_sessions_for_read_coalesced,
};
use crate::models::{
    LiveRuntimeTelemetrySummary, LiveSourceProbe, LiveSourceValidationIssue,
    LiveSourceValidationReport,
};
use sqlx::postgres::PgRow;
const CREATOR_LIVE_CONTROL_RESPONSE_CACHE_TTL: Duration = Duration::from_millis(1_000);
const CREATOR_LIVE_RUNTIME_RESPONSE_CACHE_TTL: Duration = Duration::from_millis(2_000);
const CREATOR_LIVE_RECENT_SESSION_LIMIT: i64 = 1;
const CREATOR_LIVE_RECENT_RUNTIME_OUTPUT_LIMIT: i64 = 1;
const CREATOR_LIVE_RECENT_RUNTIME_TARGET_LIMIT: i64 = 1;
const CREATOR_LIVE_RECENT_TELEMETRY_LIMIT: i64 = 4;
const CREATOR_LIVE_RECENT_EVENT_LIMIT: i64 = 4;

fn trim_runtime_collaboration_embed_for_creator_runtime(
    collaboration: &mut CreatorLiveCollaborationSummary,
) {
    let Some(active_control) = collaboration.active_control.as_mut() else {
        return;
    };

    active_control.runtime.recent_events.clear();
    active_control.socket_sessions.clear();

    let topology = &mut active_control.runtime.topology;
    topology.contributions.clear();
    topology.outputs.clear();
    topology.programs.clear();
    topology.audio.clear();
    topology.engine.nodes.clear();
    topology.engine.edges.clear();
    topology.engine.buses.clear();
    topology.engine.operations.clear();
}

fn empty_live_collaboration_summary() -> CreatorLiveCollaborationSummary {
    CreatorLiveCollaborationSummary {
        active_session: None,
        active_control: None,
        recent_sessions: Vec::new(),
        total_sessions: 0,
        active_session_count: 0,
        pending_invite_count: 0,
        active_grant_count: 0,
        issued_grant_count: 0,
    }
}

fn empty_live_runtime_telemetry_summary() -> LiveRuntimeTelemetrySummary {
    LiveRuntimeTelemetrySummary {
        total_samples: 0,
        degraded_samples: 0,
        packaging_degraded_samples: 0,
        failure_samples: 0,
        archive_failure_samples: 0,
        reconnect_events: 0,
        probe_samples: 0,
        validation_issue_samples: 0,
        repairable_validation_samples: 0,
        advisory_critical_samples: 0,
        advisory_repairable_samples: 0,
        runtime_artifact_reconciliation_samples: 0,
        runtime_archive_completion_samples: 0,
        artifact_attention_samples: 0,
        manifest_path_missing_samples: 0,
        archive_path_missing_samples: 0,
        collaboration_samples: 0,
        mix_minus_samples: 0,
        collaboration_transport_gap_samples: 0,
        packaging_ready_samples: 0,
        archive_complete_samples: 0,
        avg_bitrate_kbps: None,
        peak_bitrate_kbps: None,
        avg_viewers: None,
        peak_viewers: None,
        total_dropped_frames: 0,
        peak_collaboration_participants: 0,
        peak_active_output_routes: 0,
        peak_engine_node_count: 0,
        peak_engine_edge_count: 0,
        peak_mix_minus_edge_count: 0,
        peak_mirror_fanout_edge_count: 0,
        peak_bundle_attachment_count: 0,
        peak_bundle_mixer_count: 0,
        peak_bundle_fanout_count: 0,
        peak_bundle_return_count: 0,
        peak_media_stage_count: 0,
        peak_media_output_target_count: 0,
        peak_media_return_target_count: 0,
        peak_media_input_participant_count: 0,
        peak_media_mix_minus_participant_count: 0,
        peak_runtime_target_count: 0,
        peak_playback_target_count: 0,
        peak_recording_target_count: 0,
        peak_variant_target_count: 0,
        peak_collaboration_target_count: 0,
        peak_program_target_count: 0,
        peak_audio_target_count: 0,
        peak_engine_target_count: 0,
        peak_host_channel_count: 0,
        peak_mirror_channel_count: 0,
        peak_shared_program_mirror_channel_count: 0,
        peak_guest_isolated_mirror_channel_count: 0,
        peak_archive_target_count: 0,
        peak_active_target_count: 0,
        peak_degraded_target_count: 0,
        peak_armed_target_count: 0,
        peak_pending_source_target_count: 0,
        ll_hls_samples: 0,
        peak_discontinuity_sequence: 0,
        last_collected_at: None,
        last_runtime_state: None,
        last_packaging_status: None,
        last_archive_status: None,
        last_contribution_state: None,
        last_ingest_latency_ms: None,
        last_source_probe_present: false,
        last_source_validation_state: None,
        last_advisory_status: None,
        last_manifest_artifact_state: None,
        last_archive_artifact_state: None,
        last_collaboration_session_id: None,
        last_collaboration_participant_count: None,
        last_collaboration_transport_gap_present: false,
        last_active_output_routes: None,
        last_audio_mix_mode: None,
        last_engine_node_count: None,
        last_engine_edge_count: None,
        last_mix_minus_edge_count: None,
        last_mirror_fanout_edge_count: None,
        last_bundle_attachment_count: None,
        last_bundle_mixer_count: None,
        last_bundle_fanout_count: None,
        last_bundle_return_count: None,
        last_media_stage_count: None,
        last_media_output_target_count: None,
        last_media_return_target_count: None,
        last_media_input_participant_count: None,
        last_media_mix_minus_participant_count: None,
        last_runtime_target_count: None,
        last_playback_target_count: None,
        last_recording_target_count: None,
        last_variant_target_count: None,
        last_collaboration_target_count: None,
        last_program_target_count: None,
        last_audio_target_count: None,
        last_engine_target_count: None,
        last_host_channel_count: None,
        last_mirror_channel_count: None,
        last_shared_program_mirror_channel_count: None,
        last_guest_isolated_mirror_channel_count: None,
        last_archive_target_count: None,
        last_active_target_count: None,
        last_degraded_target_count: None,
        last_armed_target_count: None,
        last_pending_source_target_count: None,
        last_runtime_class: None,
        last_latency_profile: None,
        last_ladder_policy: None,
        last_content_class: None,
        last_failure_at: None,
        last_failure_state: None,
        last_error: None,
    }
}

pub(crate) async fn fetch_creator_live_snapshot_for_database(
    state: &SharedState,
    identity: &RequestIdentity,
) -> AppResult<CreatorLiveSnapshot> {
    let creator_id = identity.require_creator_scope()?;
    if let Ok(pool) = state.db.try_postgres_adapter() {
        let (dashboard, ingest_session) = tokio::try_join!(
            crate::api::dashboard::creator_dashboard_payload_for_database(&state.db, identity),
            fetch_postgres_active_live_ingest_session(pool, creator_id),
        )?;
        let mut profile = dashboard.profile;
        let current_broadcast = dashboard.current_broadcast;
        let pending_broadcast = dashboard.scheduled_broadcasts.into_iter().next();
        profile.current_broadcast_id = current_broadcast
            .as_ref()
            .map(|item| item.id.clone())
            .or_else(|| pending_broadcast.as_ref().map(|item| item.id.clone()));
        profile.live_status = if current_broadcast.is_some() {
            "live".to_string()
        } else if pending_broadcast.is_some() {
            "ready".to_string()
        } else {
            "offline".to_string()
        };
        return Ok(CreatorLiveSnapshot {
            profile: contract_creator_profile(profile),
            current_broadcast,
            pending_broadcast,
            ingest_session,
        });
    }
    build_creator_live_snapshot(state.db.try_sqlite_adapter()?, creator_id).await
}

pub(crate) async fn fetch_creator_live_control_response(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorLiveControlResponse> {
    reconcile_stale_creator_live_socket_sessions_for_read(pool, Some(creator_id), None).await?;
    let (snapshot, settings, health, subscriber_tiers) = tokio::try_join!(
        build_creator_live_snapshot(pool, creator_id),
        fetch_creator_live_settings(pool, creator_id),
        fetch_creator_live_health(pool, creator_id),
        fetch_creator_subscriber_tiers(pool, creator_id),
    )?;
    let collaboration =
        fetch_creator_live_collaboration_summary(pool, creator_id, &snapshot).await?;
    let viewer_history = health.samples.iter().map(|sample| sample.viewers).collect();
    let bitrate_history = health
        .samples
        .iter()
        .map(|sample| sample.bitrate_kbps)
        .collect();
    let current_viewers = if let Some(session) = snapshot.ingest_session.as_ref() {
        session.viewers
    } else if snapshot.current_broadcast.is_some() {
        fetch_live_stream_by_id(pool, &format!("lv-{}-live", snapshot.profile.handle))
            .await
            .map(|stream| stream.viewers)
            .or_else(|_| {
                health
                    .samples
                    .last()
                    .map(|sample| sample.viewers)
                    .ok_or(AppError::NotFound)
            })
            .unwrap_or(0)
    } else {
        0
    };

    Ok(CreatorLiveControlResponse {
        is_live: snapshot.current_broadcast.is_some(),
        current_viewers,
        snapshot,
        settings,
        health,
        collaboration,
        subscriber_tiers,
        viewer_history,
        bitrate_history,
    })
}

pub(crate) async fn fetch_creator_live_control_response_for_database(
    state: &SharedState,
    identity: &RequestIdentity,
) -> AppResult<CreatorLiveControlResponse> {
    let creator_id = identity.require_creator_scope()?;
    if let Ok(pool) = state.db.try_postgres_adapter() {
        let (snapshot, settings, health, dashboard) = tokio::try_join!(
            fetch_creator_live_snapshot_for_database(state, identity),
            fetch_postgres_creator_live_settings(pool, creator_id),
            fetch_postgres_creator_live_health(pool, creator_id),
            crate::api::dashboard::creator_dashboard_payload_for_database(&state.db, identity),
        )?;
        let viewer_history = health.samples.iter().map(|sample| sample.viewers).collect();
        let bitrate_history = health
            .samples
            .iter()
            .map(|sample| sample.bitrate_kbps)
            .collect();
        let current_viewers = snapshot
            .ingest_session
            .as_ref()
            .map(|session| session.viewers)
            .or_else(|| {
                health
                    .samples
                    .last()
                    .map(|sample| sample.viewers)
                    .filter(|viewers| *viewers > 0)
            })
            .or_else(|| {
                snapshot
                    .current_broadcast
                    .as_ref()
                    .map(|broadcast| broadcast.average_viewers)
            })
            .unwrap_or(0);

        return Ok(CreatorLiveControlResponse {
            is_live: snapshot.current_broadcast.is_some(),
            current_viewers,
            snapshot,
            settings,
            health,
            collaboration: empty_live_collaboration_summary(),
            subscriber_tiers: dashboard.subscriber_tiers,
            viewer_history,
            bitrate_history,
        });
    }

    fetch_creator_live_control_response(state.db.try_sqlite_adapter()?, creator_id).await
}

pub(crate) async fn fetch_creator_live_runtime_response(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorLiveRuntimeResponse> {
    reconcile_stale_creator_live_socket_sessions_for_read(pool, Some(creator_id), None).await?;
    let (snapshot, health) = tokio::try_join!(
        build_creator_live_snapshot(pool, creator_id),
        fetch_creator_live_health(pool, creator_id),
    )?;
    let collaboration =
        fetch_creator_live_collaboration_summary(pool, creator_id, &snapshot).await?;
    let active_session = snapshot.ingest_session.clone();
    let (
        active_runtime_output,
        active_runtime_targets,
        telemetry_summary,
        recent_telemetry,
        recent_events,
        recent_sessions,
        recent_runtime_outputs,
        recent_runtime_targets,
    ) = if let Some(session) = active_session.as_ref() {
        let session_id = session.id.as_str();
        let (output, targets, recent_telemetry, recent_events) = tokio::try_join!(
            fetch_live_runtime_output_for_session(pool, session_id),
            fetch_live_runtime_targets_for_session(pool, session_id),
            fetch_live_runtime_telemetry_for_session(
                pool,
                session_id,
                CREATOR_LIVE_RECENT_TELEMETRY_LIMIT,
            ),
            fetch_live_ingest_events_for_session(pool, session_id, CREATOR_LIVE_RECENT_EVENT_LIMIT),
        )?;
        let telemetry_summary =
            fetch_live_runtime_telemetry_summary_for_session(pool, session_id).await?;
        let recent_runtime_targets = targets
            .iter()
            .max_by(|left, right| {
                left.updated_at
                    .cmp(&right.updated_at)
                    .then_with(|| left.target_kind.cmp(&right.target_kind))
                    .then_with(|| left.target_key.cmp(&right.target_key))
            })
            .cloned()
            .into_iter()
            .collect();
        let recent_runtime_outputs = output.clone().into_iter().collect();
        (
            output,
            targets,
            telemetry_summary,
            recent_telemetry,
            recent_events,
            vec![session.clone()],
            recent_runtime_outputs,
            recent_runtime_targets,
        )
    } else {
        let (
            recent_sessions,
            recent_runtime_outputs,
            recent_runtime_targets,
            telemetry_summary,
            recent_telemetry,
            recent_events,
        ) = tokio::try_join!(
            fetch_recent_live_ingest_sessions(pool, creator_id, CREATOR_LIVE_RECENT_SESSION_LIMIT),
            fetch_recent_live_runtime_outputs(
                pool,
                creator_id,
                CREATOR_LIVE_RECENT_RUNTIME_OUTPUT_LIMIT,
            ),
            fetch_recent_live_runtime_targets(
                pool,
                creator_id,
                CREATOR_LIVE_RECENT_RUNTIME_TARGET_LIMIT,
            ),
            fetch_live_runtime_telemetry_summary(pool, creator_id),
            fetch_recent_live_runtime_telemetry(
                pool,
                creator_id,
                CREATOR_LIVE_RECENT_TELEMETRY_LIMIT,
            ),
            fetch_live_ingest_events_for_creator(pool, creator_id, CREATOR_LIVE_RECENT_EVENT_LIMIT),
        )?;
        (
            None,
            Vec::new(),
            telemetry_summary,
            recent_telemetry,
            recent_events,
            recent_sessions,
            recent_runtime_outputs,
            recent_runtime_targets,
        )
    };
    let runtime_advisory = build_live_runtime_advisory(
        active_session.as_ref(),
        active_runtime_output.as_ref(),
        Some(&telemetry_summary),
    );
    let runtime_advisory = if let (Some(session), Some(active_control)) = (
        active_session.as_ref(),
        collaboration.active_control.as_ref(),
    ) {
        apply_collaboration_transport_gap(
            session,
            runtime_advisory,
            collaboration_transport_gap_from_topology(&active_control.runtime.topology),
        )
    } else {
        runtime_advisory
    };
    let artifact_health = match (active_session.as_ref(), active_runtime_output.as_ref()) {
        (Some(session), Some(output)) => Some(describe_declared_live_runtime_artifact_health(
            session, output,
        )),
        _ => None,
    };

    let mut response = CreatorLiveRuntimeResponse {
        snapshot,
        health,
        collaboration,
        active_session,
        active_runtime_output,
        active_runtime_targets,
        telemetry_summary,
        runtime_advisory,
        artifact_health,
        recent_sessions,
        recent_runtime_outputs,
        recent_runtime_targets,
        recent_telemetry,
        recent_events,
    };
    trim_runtime_collaboration_embed_for_creator_runtime(&mut response.collaboration);
    Ok(response)
}

pub(crate) async fn fetch_creator_live_runtime_response_for_database(
    state: &SharedState,
    identity: &RequestIdentity,
) -> AppResult<CreatorLiveRuntimeResponse> {
    let creator_id = identity.require_creator_scope()?;
    if let Ok(pool) = state.db.try_postgres_adapter() {
        let (snapshot, health) = tokio::try_join!(
            fetch_creator_live_snapshot_for_database(state, identity),
            fetch_postgres_creator_live_health(pool, creator_id),
        )?;
        let active_session = snapshot.ingest_session.clone();
        let telemetry_summary = empty_live_runtime_telemetry_summary();
        let runtime_advisory =
            build_live_runtime_advisory(active_session.as_ref(), None, Some(&telemetry_summary));
        return Ok(CreatorLiveRuntimeResponse {
            snapshot,
            health,
            collaboration: empty_live_collaboration_summary(),
            active_session,
            active_runtime_output: None,
            active_runtime_targets: Vec::new(),
            telemetry_summary,
            runtime_advisory,
            artifact_health: None,
            recent_sessions: Vec::new(),
            recent_runtime_outputs: Vec::new(),
            recent_runtime_targets: Vec::new(),
            recent_telemetry: Vec::new(),
            recent_events: Vec::new(),
        });
    }

    fetch_creator_live_runtime_response(state.db.try_sqlite_adapter()?, creator_id).await
}

pub(crate) async fn fetch_authoritative_creator_live_control_response(
    state: &SharedState,
    creator_id: &str,
) -> AppResult<CreatorLiveControlResponse> {
    if let Some(cached) = state
        .live_response_cache
        .get_control(creator_id, CREATOR_LIVE_CONTROL_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok(cached);
    }
    let _coalesced = state
        .request_coalescer
        .acquire(&format!("creator-live-control:{creator_id}"))
        .await;
    if let Some(cached) = state
        .live_response_cache
        .get_control(creator_id, CREATOR_LIVE_CONTROL_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok(cached);
    }
    reconcile_stale_creator_live_socket_sessions_for_read_coalesced(state, Some(creator_id), None)
        .await?;
    reconcile_collaboration_expiry_for_host_read(state, creator_id).await?;
    let response =
        fetch_creator_live_control_response(state.db.try_sqlite_adapter()?, creator_id).await?;
    state
        .live_response_cache
        .put_control(creator_id, response.clone())
        .await;
    Ok(response)
}

pub(crate) async fn fetch_authoritative_creator_live_control_response_for_database(
    state: &SharedState,
    identity: &RequestIdentity,
) -> AppResult<CreatorLiveControlResponse> {
    let creator_id = identity.require_creator_scope()?;
    if let Some(cached) = state
        .live_response_cache
        .get_control(creator_id, CREATOR_LIVE_CONTROL_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok(cached);
    }
    let _coalesced = state
        .request_coalescer
        .acquire(&format!("creator-live-control:{creator_id}"))
        .await;
    if let Some(cached) = state
        .live_response_cache
        .get_control(creator_id, CREATOR_LIVE_CONTROL_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok(cached);
    }
    if state.db.try_sqlite_adapter().is_ok() {
        reconcile_stale_creator_live_socket_sessions_for_read_coalesced(
            state,
            Some(creator_id),
            None,
        )
        .await?;
        reconcile_collaboration_expiry_for_host_read(state, creator_id).await?;
    }
    let response = fetch_creator_live_control_response_for_database(state, identity).await?;
    state
        .live_response_cache
        .put_control(creator_id, response.clone())
        .await;
    Ok(response)
}

pub(crate) async fn fetch_authoritative_creator_live_runtime_response(
    state: &SharedState,
    creator_id: &str,
) -> AppResult<CreatorLiveRuntimeResponse> {
    if let Some(cached) = state
        .live_response_cache
        .get_runtime(creator_id, CREATOR_LIVE_RUNTIME_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok(cached);
    }
    let _coalesced = state
        .request_coalescer
        .acquire(&format!("creator-live-runtime:{creator_id}"))
        .await;
    if let Some(cached) = state
        .live_response_cache
        .get_runtime(creator_id, CREATOR_LIVE_RUNTIME_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok(cached);
    }
    reconcile_stale_creator_live_socket_sessions_for_read_coalesced(state, Some(creator_id), None)
        .await?;
    reconcile_collaboration_expiry_for_host_read(state, creator_id).await?;
    let response =
        fetch_creator_live_runtime_response(state.db.try_sqlite_adapter()?, creator_id).await?;
    state
        .live_response_cache
        .put_runtime(creator_id, response.clone())
        .await;
    Ok(response)
}

pub(crate) async fn fetch_authoritative_creator_live_runtime_response_for_database(
    state: &SharedState,
    identity: &RequestIdentity,
) -> AppResult<CreatorLiveRuntimeResponse> {
    let creator_id = identity.require_creator_scope()?;
    if let Some(cached) = state
        .live_response_cache
        .get_runtime(creator_id, CREATOR_LIVE_RUNTIME_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok(cached);
    }
    let _coalesced = state
        .request_coalescer
        .acquire(&format!("creator-live-runtime:{creator_id}"))
        .await;
    if let Some(cached) = state
        .live_response_cache
        .get_runtime(creator_id, CREATOR_LIVE_RUNTIME_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok(cached);
    }
    if state.db.try_sqlite_adapter().is_ok() {
        reconcile_stale_creator_live_socket_sessions_for_read_coalesced(
            state,
            Some(creator_id),
            None,
        )
        .await?;
        reconcile_collaboration_expiry_for_host_read(state, creator_id).await?;
    }
    let response = fetch_creator_live_runtime_response_for_database(state, identity).await?;
    state
        .live_response_cache
        .put_runtime(creator_id, response.clone())
        .await;
    Ok(response)
}

async fn fetch_postgres_creator_live_settings(
    pool: &sqlx::PgPool,
    creator_id: &str,
) -> AppResult<CreatorLiveSettings> {
    let row = sqlx::query(
        r#"
        SELECT subscriber_only, slow_mode_seconds::BIGINT AS slow_mode_seconds,
               auto_mod_level, notify_followers_default, delivery_class,
               active_scene_id, scenes_json
        FROM creator_live_settings
        WHERE creator_id = $1
        "#,
    )
    .bind(creator_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(CreatorLiveSettings {
        subscriber_only: row.get::<i64, _>("subscriber_only") != 0,
        slow_mode_seconds: row.get("slow_mode_seconds"),
        auto_mod_level: row.get("auto_mod_level"),
        notify_followers_default: row.get::<i64, _>("notify_followers_default") != 0,
        delivery_class: row.get("delivery_class"),
        active_scene_id: row.get("active_scene_id"),
        scenes: from_json(row.get::<String, _>("scenes_json"))?,
    })
}

async fn fetch_postgres_creator_live_health(
    pool: &sqlx::PgPool,
    creator_id: &str,
) -> AppResult<CreatorLiveHealth> {
    let settings_row = sqlx::query(
        r#"
        SELECT bitrate_kbps::BIGINT AS bitrate_kbps, cpu_percent::BIGINT AS cpu_percent,
               dropped_frames::BIGINT AS dropped_frames, free_disk_gb::DOUBLE PRECISION AS free_disk_gb
        FROM creator_live_settings
        WHERE creator_id = $1
        "#,
    )
    .bind(creator_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let sample_rows = sqlx::query(
        r#"
        SELECT collected_at, bitrate_kbps::BIGINT AS bitrate_kbps, viewers::BIGINT AS viewers,
               cpu_percent::BIGINT AS cpu_percent, dropped_frames::BIGINT AS dropped_frames,
               free_disk_gb::DOUBLE PRECISION AS free_disk_gb
        FROM creator_stream_health_samples
        WHERE creator_id = $1
        ORDER BY collected_at DESC
        LIMIT 24
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;
    let mut samples = sample_rows
        .into_iter()
        .map(|row| CreatorHealthSample {
            collected_at: row.get("collected_at"),
            bitrate_kbps: row.get("bitrate_kbps"),
            viewers: row.get("viewers"),
            cpu_percent: row.get("cpu_percent"),
            dropped_frames: row.get("dropped_frames"),
            free_disk_gb: row.get("free_disk_gb"),
        })
        .collect::<Vec<_>>();
    samples.reverse();

    Ok(CreatorLiveHealth {
        current_bitrate_kbps: settings_row.get("bitrate_kbps"),
        current_cpu_percent: settings_row.get("cpu_percent"),
        current_dropped_frames: settings_row.get("dropped_frames"),
        current_free_disk_gb: settings_row.get("free_disk_gb"),
        samples,
    })
}

async fn fetch_postgres_active_live_ingest_session(
    pool: &sqlx::PgPool,
    creator_id: &str,
) -> AppResult<Option<LiveIngestSession>> {
    let row = sqlx::query(
        r#"
        SELECT id, creator_id, broadcast_id, previous_session_id, protocol, contribution_class,
               contribution_state, ingest_server, ingest_latency_ms::BIGINT AS ingest_latency_ms,
               source_container_format, source_video_codec, source_audio_codec,
               source_width::BIGINT AS source_width, source_height::BIGINT AS source_height,
               source_frame_rate::DOUBLE PRECISION AS source_frame_rate,
               source_audio_sample_rate_hz::BIGINT AS source_audio_sample_rate_hz,
               source_audio_channels::BIGINT AS source_audio_channels,
               last_source_probe_at, source_validation_state, source_validation_issues_json,
               status, bitrate_kbps::BIGINT AS bitrate_kbps, viewers::BIGINT AS viewers,
               dropped_frames::BIGINT AS dropped_frames, connected_at, last_heartbeat_at,
               disconnected_at
        FROM live_ingest_sessions
        WHERE creator_id = $1
          AND status IN ('connected', 'active', 'live')
        ORDER BY connected_at DESC
        LIMIT 1
        "#,
    )
    .bind(creator_id)
    .fetch_optional(pool)
    .await?;

    row.map(postgres_live_ingest_session_from_row).transpose()
}

fn postgres_live_ingest_session_from_row(row: PgRow) -> AppResult<LiveIngestSession> {
    Ok(LiveIngestSession {
        id: row.get("id"),
        creator_id: row.get("creator_id"),
        broadcast_id: row.get("broadcast_id"),
        previous_session_id: row.get("previous_session_id"),
        protocol: row.get("protocol"),
        contribution_class: row.get("contribution_class"),
        contribution_state: row.get("contribution_state"),
        ingest_server: row.get("ingest_server"),
        ingest_latency_ms: row.get("ingest_latency_ms"),
        source_probe: postgres_live_source_probe_from_row(&row),
        source_validation: postgres_live_source_validation_from_row(&row)?,
        status: row.get("status"),
        bitrate_kbps: row.get("bitrate_kbps"),
        viewers: row.get("viewers"),
        dropped_frames: row.get("dropped_frames"),
        connected_at: row.get("connected_at"),
        last_heartbeat_at: row.get("last_heartbeat_at"),
        disconnected_at: row.get("disconnected_at"),
    })
}

fn postgres_live_source_probe_from_row(row: &PgRow) -> Option<LiveSourceProbe> {
    let probed_at = row.get::<Option<String>, _>("last_source_probe_at")?;
    Some(LiveSourceProbe {
        container_format: row.get("source_container_format"),
        video_codec: row.get("source_video_codec"),
        audio_codec: row.get("source_audio_codec"),
        width: row.get("source_width"),
        height: row.get("source_height"),
        frame_rate: row.get("source_frame_rate"),
        audio_sample_rate_hz: row.get("source_audio_sample_rate_hz"),
        audio_channels: row.get("source_audio_channels"),
        probed_at,
    })
}

fn postgres_live_source_validation_from_row(
    row: &PgRow,
) -> AppResult<Option<LiveSourceValidationReport>> {
    let Some(validated_at) = row.get::<Option<String>, _>("last_source_probe_at") else {
        return Ok(None);
    };
    let issues_json = row
        .get::<Option<String>, _>("source_validation_issues_json")
        .unwrap_or_else(|| "[]".to_string());
    let issues = serde_json::from_str::<Vec<LiveSourceValidationIssue>>(&issues_json)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(Some(LiveSourceValidationReport {
        state: row
            .get::<Option<String>, _>("source_validation_state")
            .unwrap_or_else(|| "unknown".to_string()),
        issues,
        validated_at,
    }))
}
