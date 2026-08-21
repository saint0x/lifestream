use super::*;
use crate::api::control::{
    apply_collaboration_transport_gap, build_live_runtime_advisory,
    collaboration_transport_gap_from_topology, describe_declared_live_runtime_artifact_health,
    fetch_live_ingest_events_for_session, fetch_live_runtime_output_for_session,
    fetch_live_runtime_targets_for_session, fetch_live_runtime_telemetry_for_session,
    fetch_live_runtime_telemetry_summary,
    fetch_recent_live_runtime_targets, fetch_recent_live_runtime_telemetry,
};
use crate::models::{LiveRuntimeTelemetry, LiveRuntimeTelemetrySummary};

const CREATOR_LIVE_RECENT_SESSION_LIMIT: i64 = 1;
const CREATOR_LIVE_RECENT_RUNTIME_OUTPUT_LIMIT: i64 = 1;
const CREATOR_LIVE_RECENT_RUNTIME_TARGET_LIMIT: i64 = 1;
const CREATOR_LIVE_RECENT_TELEMETRY_LIMIT: i64 = 1;
const CREATOR_LIVE_RECENT_EVENT_LIMIT: i64 = 4;

async fn fetch_creator_runtime_telemetry_summary_compact(
    pool: &SqlitePool,
    session_id: &str,
    recent_telemetry: &[LiveRuntimeTelemetry],
) -> AppResult<LiveRuntimeTelemetrySummary> {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) AS total_samples,
            SUM(CASE
                WHEN runtime_state = 'failed'
                  OR packaging_status = 'failed'
                  OR archive_status = 'failed'
                THEN 1 ELSE 0 END
            ) AS failure_samples
        FROM live_runtime_telemetry
        WHERE session_id = ?
        "#,
    )
    .bind(session_id)
    .fetch_one(pool)
    .await?;

    let latest = recent_telemetry.first();

    Ok(LiveRuntimeTelemetrySummary {
        total_samples: row.get("total_samples"),
        degraded_samples: 0,
        packaging_degraded_samples: 0,
        failure_samples: row.get::<Option<i64>, _>("failure_samples").unwrap_or(0),
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
        avg_bitrate_kbps: latest.map(|item| item.bitrate_kbps as f64),
        peak_bitrate_kbps: latest.map(|item| item.bitrate_kbps),
        avg_viewers: latest.map(|item| item.viewers as f64),
        peak_viewers: latest.map(|item| item.viewers),
        total_dropped_frames: latest.map(|item| item.dropped_frames).unwrap_or(0),
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
        last_collected_at: latest.map(|item| item.collected_at.clone()),
        last_runtime_state: latest.map(|item| item.runtime_state.clone()),
        last_packaging_status: latest.map(|item| item.packaging_status.clone()),
        last_archive_status: latest.map(|item| item.archive_status.clone()),
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
    })
}

pub(crate) async fn fetch_creator_live_control_response(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorLiveControlResponse> {
    reconcile_stale_creator_live_socket_sessions_for_read(pool, Some(creator_id), None).await?;
    let snapshot = build_creator_live_snapshot(pool, creator_id).await?;
    let settings = fetch_creator_live_settings(pool, creator_id).await?;
    let health = fetch_creator_live_health(pool, creator_id).await?;
    let collaboration =
        fetch_creator_live_collaboration_summary(pool, creator_id, &snapshot).await?;
    let subscriber_tiers = fetch_creator_subscriber_tiers(pool, creator_id).await?;
    let viewer_history = health.samples.iter().map(|sample| sample.viewers).collect();
    let bitrate_history = health
        .samples
        .iter()
        .map(|sample| sample.bitrate_kbps)
        .collect();
    let current_viewers = if let Some(session) = snapshot.ingest_session.as_ref() {
        session.viewers
    } else if snapshot.current_broadcast.is_some() {
        if let Some(viewers) = health.samples.last().map(|sample| sample.viewers) {
            viewers
        } else {
            fetch_live_stream_by_id(pool, &format!("lv-{}-live", snapshot.profile.handle))
                .await
                .map(|stream| stream.viewers)
                .unwrap_or(0)
        }
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

pub(crate) async fn fetch_creator_live_runtime_response(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorLiveRuntimeResponse> {
    reconcile_stale_creator_live_socket_sessions_for_read(pool, Some(creator_id), None).await?;
    let (snapshot, health, recent_sessions, recent_runtime_outputs, recent_runtime_targets) =
        tokio::try_join!(
            build_creator_live_snapshot(pool, creator_id),
            fetch_creator_live_health(pool, creator_id),
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
    ) =
        if let Some(session) = active_session.as_ref() {
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
                fetch_creator_runtime_telemetry_summary_compact(pool, session_id, &recent_telemetry)
                    .await?;
            (
                output,
                targets,
                telemetry_summary,
                recent_telemetry,
                recent_events,
            )
        } else {
            let telemetry_summary = fetch_live_runtime_telemetry_summary(pool, creator_id).await?;
            let recent_telemetry =
                fetch_recent_live_runtime_telemetry(
                    pool,
                    creator_id,
                    CREATOR_LIVE_RECENT_TELEMETRY_LIMIT,
                )
                .await?;
            let recent_events =
                fetch_live_ingest_events_for_creator(pool, creator_id, CREATOR_LIVE_RECENT_EVENT_LIMIT)
                    .await?;
            (
                None,
                Vec::new(),
                telemetry_summary,
                recent_telemetry,
                recent_events,
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

    Ok(CreatorLiveRuntimeResponse {
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
    })
}

pub(crate) async fn fetch_authoritative_creator_live_control_response(
    state: &SharedState,
    creator_id: &str,
) -> AppResult<CreatorLiveControlResponse> {
    reconcile_collaboration_expiry_for_host_read(state, creator_id).await?;
    fetch_creator_live_control_response(&state.pool, creator_id).await
}

pub(crate) async fn fetch_authoritative_creator_live_runtime_response(
    state: &SharedState,
    creator_id: &str,
) -> AppResult<CreatorLiveRuntimeResponse> {
    reconcile_collaboration_expiry_for_host_read(state, creator_id).await?;
    fetch_creator_live_runtime_response(&state.pool, creator_id).await
}
