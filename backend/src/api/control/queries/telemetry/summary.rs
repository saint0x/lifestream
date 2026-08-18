use super::summary_rows::{
    fetch_latest_failure_row, fetch_latest_telemetry_row, fetch_summary_row,
};
use super::*;

pub(crate) async fn fetch_live_runtime_telemetry_summary(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<LiveRuntimeTelemetrySummary> {
    fetch_live_runtime_telemetry_summary_by_scope(pool, "creator_id", creator_id).await
}

pub(crate) async fn fetch_live_runtime_telemetry_summary_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<LiveRuntimeTelemetrySummary> {
    fetch_live_runtime_telemetry_summary_by_scope(pool, "session_id", session_id).await
}

async fn fetch_live_runtime_telemetry_summary_by_scope(
    pool: &SqlitePool,
    scope_column: &str,
    scope_value: &str,
) -> AppResult<LiveRuntimeTelemetrySummary> {
    let total_row = fetch_summary_row(pool, scope_column, scope_value).await?;

    let latest_row = fetch_latest_telemetry_row(pool, scope_column, scope_value).await?;
    let failure_row = fetch_latest_failure_row(pool, scope_column, scope_value).await?;

    Ok(LiveRuntimeTelemetrySummary {
        total_samples: total_row.get("total_samples"),
        degraded_samples: total_row
            .get::<Option<i64>, _>("degraded_samples")
            .unwrap_or(0),
        packaging_degraded_samples: total_row
            .get::<Option<i64>, _>("packaging_degraded_samples")
            .unwrap_or(0),
        failure_samples: total_row
            .get::<Option<i64>, _>("failure_samples")
            .unwrap_or(0),
        archive_failure_samples: total_row
            .get::<Option<i64>, _>("archive_failure_samples")
            .unwrap_or(0),
        reconnect_events: total_row
            .get::<Option<i64>, _>("reconnect_events")
            .unwrap_or(0),
        probe_samples: total_row
            .get::<Option<i64>, _>("probe_samples")
            .unwrap_or(0),
        validation_issue_samples: total_row
            .get::<Option<i64>, _>("validation_issue_samples")
            .unwrap_or(0),
        repairable_validation_samples: total_row
            .get::<Option<i64>, _>("repairable_validation_samples")
            .unwrap_or(0),
        advisory_critical_samples: total_row
            .get::<Option<i64>, _>("advisory_critical_samples")
            .unwrap_or(0),
        advisory_repairable_samples: total_row
            .get::<Option<i64>, _>("advisory_repairable_samples")
            .unwrap_or(0),
        runtime_artifact_reconciliation_samples: total_row
            .get::<Option<i64>, _>("runtime_artifact_reconciliation_samples")
            .unwrap_or(0),
        runtime_archive_completion_samples: total_row
            .get::<Option<i64>, _>("runtime_archive_completion_samples")
            .unwrap_or(0),
        artifact_attention_samples: total_row
            .get::<Option<i64>, _>("artifact_attention_samples")
            .unwrap_or(0),
        manifest_path_missing_samples: total_row
            .get::<Option<i64>, _>("manifest_path_missing_samples")
            .unwrap_or(0),
        archive_path_missing_samples: total_row
            .get::<Option<i64>, _>("archive_path_missing_samples")
            .unwrap_or(0),
        collaboration_samples: total_row
            .get::<Option<i64>, _>("collaboration_samples")
            .unwrap_or(0),
        mix_minus_samples: total_row
            .get::<Option<i64>, _>("mix_minus_samples")
            .unwrap_or(0),
        collaboration_transport_gap_samples: total_row
            .get::<Option<i64>, _>("collaboration_transport_gap_samples")
            .unwrap_or(0),
        packaging_ready_samples: total_row
            .get::<Option<i64>, _>("packaging_ready_samples")
            .unwrap_or(0),
        archive_complete_samples: total_row
            .get::<Option<i64>, _>("archive_complete_samples")
            .unwrap_or(0),
        avg_bitrate_kbps: total_row.get("avg_bitrate_kbps"),
        peak_bitrate_kbps: total_row.get("peak_bitrate_kbps"),
        avg_viewers: total_row.get("avg_viewers"),
        peak_viewers: total_row.get("peak_viewers"),
        total_dropped_frames: total_row
            .get::<Option<i64>, _>("total_dropped_frames")
            .unwrap_or(0),
        peak_collaboration_participants: total_row
            .get::<Option<i64>, _>("peak_collaboration_participants")
            .unwrap_or(0),
        peak_active_output_routes: total_row
            .get::<Option<i64>, _>("peak_active_output_routes")
            .unwrap_or(0),
        peak_engine_node_count: total_row
            .get::<Option<i64>, _>("peak_engine_node_count")
            .unwrap_or(0),
        peak_engine_edge_count: total_row
            .get::<Option<i64>, _>("peak_engine_edge_count")
            .unwrap_or(0),
        peak_mix_minus_edge_count: total_row
            .get::<Option<i64>, _>("peak_mix_minus_edge_count")
            .unwrap_or(0),
        peak_mirror_fanout_edge_count: total_row
            .get::<Option<i64>, _>("peak_mirror_fanout_edge_count")
            .unwrap_or(0),
        peak_bundle_attachment_count: total_row
            .get::<Option<i64>, _>("peak_bundle_attachment_count")
            .unwrap_or(0),
        peak_bundle_mixer_count: total_row
            .get::<Option<i64>, _>("peak_bundle_mixer_count")
            .unwrap_or(0),
        peak_bundle_fanout_count: total_row
            .get::<Option<i64>, _>("peak_bundle_fanout_count")
            .unwrap_or(0),
        peak_bundle_return_count: total_row
            .get::<Option<i64>, _>("peak_bundle_return_count")
            .unwrap_or(0),
        peak_media_stage_count: total_row
            .get::<Option<i64>, _>("peak_media_stage_count")
            .unwrap_or(0),
        peak_media_output_target_count: total_row
            .get::<Option<i64>, _>("peak_media_output_target_count")
            .unwrap_or(0),
        peak_media_return_target_count: total_row
            .get::<Option<i64>, _>("peak_media_return_target_count")
            .unwrap_or(0),
        peak_media_input_participant_count: total_row
            .get::<Option<i64>, _>("peak_media_input_participant_count")
            .unwrap_or(0),
        peak_media_mix_minus_participant_count: total_row
            .get::<Option<i64>, _>("peak_media_mix_minus_participant_count")
            .unwrap_or(0),
        peak_runtime_target_count: total_row
            .get::<Option<i64>, _>("peak_runtime_target_count")
            .unwrap_or(0),
        peak_playback_target_count: total_row
            .get::<Option<i64>, _>("peak_playback_target_count")
            .unwrap_or(0),
        peak_recording_target_count: total_row
            .get::<Option<i64>, _>("peak_recording_target_count")
            .unwrap_or(0),
        peak_variant_target_count: total_row
            .get::<Option<i64>, _>("peak_variant_target_count")
            .unwrap_or(0),
        peak_collaboration_target_count: total_row
            .get::<Option<i64>, _>("peak_collaboration_target_count")
            .unwrap_or(0),
        peak_program_target_count: total_row
            .get::<Option<i64>, _>("peak_program_target_count")
            .unwrap_or(0),
        peak_audio_target_count: total_row
            .get::<Option<i64>, _>("peak_audio_target_count")
            .unwrap_or(0),
        peak_engine_target_count: total_row
            .get::<Option<i64>, _>("peak_engine_target_count")
            .unwrap_or(0),
        peak_host_channel_count: total_row
            .get::<Option<i64>, _>("peak_host_channel_count")
            .unwrap_or(0),
        peak_mirror_channel_count: total_row
            .get::<Option<i64>, _>("peak_mirror_channel_count")
            .unwrap_or(0),
        peak_shared_program_mirror_channel_count: total_row
            .get::<Option<i64>, _>("peak_shared_program_mirror_channel_count")
            .unwrap_or(0),
        peak_guest_isolated_mirror_channel_count: total_row
            .get::<Option<i64>, _>("peak_guest_isolated_mirror_channel_count")
            .unwrap_or(0),
        peak_archive_target_count: total_row
            .get::<Option<i64>, _>("peak_archive_target_count")
            .unwrap_or(0),
        peak_active_target_count: total_row
            .get::<Option<i64>, _>("peak_active_target_count")
            .unwrap_or(0),
        peak_degraded_target_count: total_row
            .get::<Option<i64>, _>("peak_degraded_target_count")
            .unwrap_or(0),
        peak_armed_target_count: total_row
            .get::<Option<i64>, _>("peak_armed_target_count")
            .unwrap_or(0),
        peak_pending_source_target_count: total_row
            .get::<Option<i64>, _>("peak_pending_source_target_count")
            .unwrap_or(0),
        ll_hls_samples: total_row
            .get::<Option<i64>, _>("ll_hls_samples")
            .unwrap_or(0),
        peak_discontinuity_sequence: total_row
            .get::<Option<i64>, _>("peak_discontinuity_sequence")
            .unwrap_or(0),
        last_collected_at: latest_row.as_ref().map(|row| row.get("collected_at")),
        last_runtime_state: latest_row.as_ref().map(|row| row.get("runtime_state")),
        last_packaging_status: latest_row.as_ref().map(|row| row.get("packaging_status")),
        last_archive_status: latest_row.as_ref().map(|row| row.get("archive_status")),
        last_contribution_state: latest_row
            .as_ref()
            .and_then(|row| row.get("contribution_state")),
        last_ingest_latency_ms: latest_row
            .as_ref()
            .and_then(|row| row.get("ingest_latency_ms")),
        last_source_probe_present: latest_row
            .as_ref()
            .and_then(|row| row.get::<Option<i64>, _>("source_probe_present"))
            .unwrap_or(0)
            != 0,
        last_source_validation_state: latest_row
            .as_ref()
            .and_then(|row| row.get("source_validation_state")),
        last_advisory_status: latest_row
            .as_ref()
            .and_then(|row| row.get("advisory_status")),
        last_manifest_artifact_state: latest_row
            .as_ref()
            .and_then(|row| row.get("manifest_artifact_state")),
        last_archive_artifact_state: latest_row
            .as_ref()
            .and_then(|row| row.get("archive_artifact_state")),
        last_collaboration_session_id: latest_row
            .as_ref()
            .and_then(|row| row.get("collaboration_session_id")),
        last_collaboration_participant_count: latest_row
            .as_ref()
            .and_then(|row| row.get("collaboration_participant_count")),
        last_collaboration_transport_gap_present: latest_row
            .as_ref()
            .and_then(|row| row.get::<Option<i64>, _>("collaboration_transport_gap_present"))
            .unwrap_or(0)
            != 0,
        last_active_output_routes: latest_row
            .as_ref()
            .and_then(|row| row.get("active_output_routes")),
        last_audio_mix_mode: latest_row
            .as_ref()
            .and_then(|row| row.get("audio_mix_mode")),
        last_engine_node_count: latest_row
            .as_ref()
            .and_then(|row| row.get("engine_node_count")),
        last_engine_edge_count: latest_row
            .as_ref()
            .and_then(|row| row.get("engine_edge_count")),
        last_mix_minus_edge_count: latest_row
            .as_ref()
            .and_then(|row| row.get("mix_minus_edge_count")),
        last_mirror_fanout_edge_count: latest_row
            .as_ref()
            .and_then(|row| row.get("mirror_fanout_edge_count")),
        last_bundle_attachment_count: latest_row
            .as_ref()
            .and_then(|row| row.get("bundle_attachment_count")),
        last_bundle_mixer_count: latest_row
            .as_ref()
            .and_then(|row| row.get("bundle_mixer_count")),
        last_bundle_fanout_count: latest_row
            .as_ref()
            .and_then(|row| row.get("bundle_fanout_count")),
        last_bundle_return_count: latest_row
            .as_ref()
            .and_then(|row| row.get("bundle_return_count")),
        last_media_stage_count: latest_row
            .as_ref()
            .and_then(|row| row.get("media_stage_count")),
        last_media_output_target_count: latest_row
            .as_ref()
            .and_then(|row| row.get("media_output_target_count")),
        last_media_return_target_count: latest_row
            .as_ref()
            .and_then(|row| row.get("media_return_target_count")),
        last_media_input_participant_count: latest_row
            .as_ref()
            .and_then(|row| row.get("media_input_participant_count")),
        last_media_mix_minus_participant_count: latest_row
            .as_ref()
            .and_then(|row| row.get("media_mix_minus_participant_count")),
        last_runtime_target_count: latest_row
            .as_ref()
            .and_then(|row| row.get("runtime_target_count")),
        last_playback_target_count: latest_row
            .as_ref()
            .and_then(|row| row.get("playback_target_count")),
        last_recording_target_count: latest_row
            .as_ref()
            .and_then(|row| row.get("recording_target_count")),
        last_variant_target_count: latest_row
            .as_ref()
            .and_then(|row| row.get("variant_target_count")),
        last_collaboration_target_count: latest_row
            .as_ref()
            .and_then(|row| row.get("collaboration_target_count")),
        last_program_target_count: latest_row
            .as_ref()
            .and_then(|row| row.get("program_target_count")),
        last_audio_target_count: latest_row
            .as_ref()
            .and_then(|row| row.get("audio_target_count")),
        last_engine_target_count: latest_row
            .as_ref()
            .and_then(|row| row.get("engine_target_count")),
        last_host_channel_count: latest_row
            .as_ref()
            .and_then(|row| row.get("host_channel_count")),
        last_mirror_channel_count: latest_row
            .as_ref()
            .and_then(|row| row.get("mirror_channel_count")),
        last_shared_program_mirror_channel_count: latest_row
            .as_ref()
            .and_then(|row| row.get("shared_program_mirror_channel_count")),
        last_guest_isolated_mirror_channel_count: latest_row
            .as_ref()
            .and_then(|row| row.get("guest_isolated_mirror_channel_count")),
        last_archive_target_count: latest_row
            .as_ref()
            .and_then(|row| row.get("archive_target_count")),
        last_active_target_count: latest_row
            .as_ref()
            .and_then(|row| row.get("active_target_count")),
        last_degraded_target_count: latest_row
            .as_ref()
            .and_then(|row| row.get("degraded_target_count")),
        last_armed_target_count: latest_row
            .as_ref()
            .and_then(|row| row.get("armed_target_count")),
        last_pending_source_target_count: latest_row
            .as_ref()
            .and_then(|row| row.get("pending_source_target_count")),
        last_runtime_class: latest_row.as_ref().and_then(|row| row.get("runtime_class")),
        last_latency_profile: latest_row
            .as_ref()
            .and_then(|row| row.get("latency_profile")),
        last_ladder_policy: latest_row.as_ref().and_then(|row| row.get("ladder_policy")),
        last_content_class: latest_row.as_ref().and_then(|row| row.get("content_class")),
        last_failure_at: failure_row.as_ref().map(|row| row.get("collected_at")),
        last_failure_state: failure_row.as_ref().map(|row| row.get("failure_state")),
        last_error: failure_row.as_ref().and_then(|row| row.get("last_error")),
    })
}
