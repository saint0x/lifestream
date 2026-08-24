use super::*;

pub(super) async fn fetch_summary_row(
    pool: &SqlitePool,
    scope_column: &str,
    scope_value: &str,
) -> AppResult<sqlx::sqlite::SqliteRow> {
    let summary_query = format!(
        r#"
        SELECT
            COUNT(*) AS total_samples,
            SUM(CASE
                WHEN runtime_state IN ('degraded', 'stale', 'disconnected', 'packaging_degraded')
                  OR packaging_status = 'degraded'
                THEN 1 ELSE 0 END
            ) AS degraded_samples,
            SUM(CASE
                WHEN packaging_status = 'degraded'
                THEN 1 ELSE 0 END
            ) AS packaging_degraded_samples,
            SUM(CASE
                WHEN runtime_state = 'failed'
                  OR packaging_status = 'failed'
                  OR archive_status = 'failed'
                THEN 1 ELSE 0 END
            ) AS failure_samples,
            SUM(CASE
                WHEN archive_status = 'failed'
                THEN 1 ELSE 0 END
            ) AS archive_failure_samples,
            SUM(CASE
                WHEN json_extract(detail_json, '$.session.reconnectSession') = 1
                THEN 1 ELSE 0 END
            ) AS reconnect_events,
            SUM(CASE
                WHEN json_extract(detail_json, '$.session.sourceProbePresent') = 1
                THEN 1 ELSE 0 END
            ) AS probe_samples,
            SUM(CASE
                WHEN json_extract(detail_json, '$.session.sourceValidation.state') IS NOT NULL
                  AND json_extract(detail_json, '$.session.sourceValidation.state') != 'valid'
                THEN 1 ELSE 0 END
            ) AS validation_issue_samples,
            SUM(CASE
                WHEN json_extract(detail_json, '$.session.sourceValidation.state') = 'repairable'
                THEN 1 ELSE 0 END
            ) AS repairable_validation_samples,
            SUM(CASE
                WHEN sample_kind != 'session_connected'
                  AND json_extract(detail_json, '$.advisory.status') = 'critical'
                THEN 1 ELSE 0 END
            ) AS advisory_critical_samples,
            SUM(CASE
                WHEN sample_kind != 'session_connected'
                  AND json_extract(detail_json, '$.advisory.status') = 'repairable'
                THEN 1 ELSE 0 END
            ) AS advisory_repairable_samples,
            SUM(CASE
                WHEN sample_kind = 'runtime_artifact_reconciled'
                THEN 1 ELSE 0 END
            ) AS runtime_artifact_reconciliation_samples,
            SUM(CASE
                WHEN sample_kind = 'runtime_archive_completed'
                THEN 1 ELSE 0 END
            ) AS runtime_archive_completion_samples,
            SUM(CASE
                WHEN json_extract(detail_json, '$.artifacts.status') = 'attention'
                THEN 1 ELSE 0 END
            ) AS artifact_attention_samples,
            SUM(CASE
                WHEN json_extract(detail_json, '$.artifacts.manifest.state') = 'missing'
                THEN 1 ELSE 0 END
            ) AS manifest_path_missing_samples,
            SUM(CASE
                WHEN json_extract(detail_json, '$.artifacts.archive.state') = 'missing'
                THEN 1 ELSE 0 END
            ) AS archive_path_missing_samples,
            SUM(CASE
                WHEN json_extract(detail_json, '$.collaboration.present') = 1
                THEN 1 ELSE 0 END
            ) AS collaboration_samples,
            SUM(CASE
                WHEN json_extract(detail_json, '$.collaboration.mixMinusRequired') = 1
                THEN 1 ELSE 0 END
            ) AS mix_minus_samples,
            SUM(CASE
                WHEN json_extract(detail_json, '$.collaboration.transportGapPresent') = 1
                THEN 1 ELSE 0 END
            ) AS collaboration_transport_gap_samples,
            SUM(CASE
                WHEN packaging_status IN ('ready', 'complete')
                THEN 1 ELSE 0 END
            ) AS packaging_ready_samples,
            SUM(CASE
                WHEN archive_status = 'complete'
                THEN 1 ELSE 0 END
            ) AS archive_complete_samples,
            AVG(CAST(bitrate_kbps AS REAL)) AS avg_bitrate_kbps,
            MAX(bitrate_kbps) AS peak_bitrate_kbps,
            AVG(CAST(viewers AS REAL)) AS avg_viewers,
            MAX(viewers) AS peak_viewers,
            SUM(dropped_frames) AS total_dropped_frames,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.collaboration.participantCount') AS INTEGER), 0))
                AS peak_collaboration_participants,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.outputs.activeRouteCount') AS INTEGER), 0))
                AS peak_active_output_routes,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.collaboration.engineNodeCount') AS INTEGER), 0))
                AS peak_engine_node_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.collaboration.engineEdgeCount') AS INTEGER), 0))
                AS peak_engine_edge_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.collaboration.mixMinusEdgeCount') AS INTEGER), 0))
                AS peak_mix_minus_edge_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.collaboration.mirrorFanoutEdgeCount') AS INTEGER), 0))
                AS peak_mirror_fanout_edge_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.collaboration.bundleAttachmentCount') AS INTEGER), 0))
                AS peak_bundle_attachment_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.collaboration.bundleMixerCount') AS INTEGER), 0))
                AS peak_bundle_mixer_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.collaboration.bundleFanoutCount') AS INTEGER), 0))
                AS peak_bundle_fanout_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.collaboration.bundleReturnCount') AS INTEGER), 0))
                AS peak_bundle_return_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.collaboration.mediaStageCount') AS INTEGER), 0))
                AS peak_media_stage_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.collaboration.mediaOutputTargetCount') AS INTEGER), 0))
                AS peak_media_output_target_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.collaboration.mediaReturnTargetCount') AS INTEGER), 0))
                AS peak_media_return_target_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.collaboration.mediaInputParticipantCount') AS INTEGER), 0))
                AS peak_media_input_participant_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.collaboration.mediaMixMinusParticipantCount') AS INTEGER), 0))
                AS peak_media_mix_minus_participant_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.targets.count') AS INTEGER), 0))
                AS peak_runtime_target_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.targets.playbackEnabledCount') AS INTEGER), 0))
                AS peak_playback_target_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.targets.recordingEnabledCount') AS INTEGER), 0))
                AS peak_recording_target_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.targets.variantCount') AS INTEGER), 0))
                AS peak_variant_target_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.targets.collaborationCount') AS INTEGER), 0))
                AS peak_collaboration_target_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.targets.programCount') AS INTEGER), 0))
                AS peak_program_target_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.targets.audioCount') AS INTEGER), 0))
                AS peak_audio_target_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.targets.engineCount') AS INTEGER), 0))
                AS peak_engine_target_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.targets.hostChannelCount') AS INTEGER), 0))
                AS peak_host_channel_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.targets.mirrorChannelCount') AS INTEGER), 0))
                AS peak_mirror_channel_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.targets.sharedProgramMirrorChannelCount') AS INTEGER), 0))
                AS peak_shared_program_mirror_channel_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.targets.guestIsolatedMirrorChannelCount') AS INTEGER), 0))
                AS peak_guest_isolated_mirror_channel_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.targets.archiveCount') AS INTEGER), 0))
                AS peak_archive_target_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.targets.activeCount') AS INTEGER), 0))
                AS peak_active_target_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.targets.degradedCount') AS INTEGER), 0))
                AS peak_degraded_target_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.targets.armedCount') AS INTEGER), 0))
                AS peak_armed_target_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.targets.pendingSourceCount') AS INTEGER), 0))
                AS peak_pending_source_target_count,
            SUM(CASE
                WHEN json_extract(detail_json, '$.delivery.runtimeClass') = 'll_hls'
                THEN 1 ELSE 0 END
            ) AS ll_hls_samples,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.delivery.discontinuitySequence') AS INTEGER), 0))
                AS peak_discontinuity_sequence
        FROM live_runtime_telemetry
        WHERE {scope_column} = ?
        "#
    );
    sqlx::query(&summary_query)
        .bind(scope_value)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
}

pub(super) async fn fetch_latest_telemetry_row(
    pool: &SqlitePool,
    scope_column: &str,
    scope_value: &str,
) -> AppResult<Option<sqlx::sqlite::SqliteRow>> {
    let latest_query = format!(
        r#"
        SELECT
            collected_at,
            runtime_state,
            packaging_status,
            archive_status,
            json_extract(detail_json, '$.session.contributionState') AS contribution_state,
            CAST(json_extract(detail_json, '$.session.ingestLatencyMs') AS INTEGER) AS ingest_latency_ms,
            CAST(json_extract(detail_json, '$.session.sourceProbePresent') AS INTEGER) AS source_probe_present,
            json_extract(detail_json, '$.session.sourceValidation.state') AS source_validation_state,
            json_extract(detail_json, '$.advisory.status') AS advisory_status,
            json_extract(detail_json, '$.artifacts.manifest.state') AS manifest_artifact_state,
            json_extract(detail_json, '$.artifacts.archive.state') AS archive_artifact_state,
            json_extract(detail_json, '$.collaboration.sessionId') AS collaboration_session_id,
            CAST(json_extract(detail_json, '$.collaboration.participantCount') AS INTEGER)
                AS collaboration_participant_count,
            CAST(json_extract(detail_json, '$.collaboration.transportGapPresent') AS INTEGER)
                AS collaboration_transport_gap_present,
            CAST(json_extract(detail_json, '$.outputs.activeRouteCount') AS INTEGER) AS active_output_routes,
            json_extract(detail_json, '$.collaboration.audioMixMode') AS audio_mix_mode,
            CAST(json_extract(detail_json, '$.collaboration.engineNodeCount') AS INTEGER) AS engine_node_count,
            CAST(json_extract(detail_json, '$.collaboration.engineEdgeCount') AS INTEGER) AS engine_edge_count,
            CAST(json_extract(detail_json, '$.collaboration.mixMinusEdgeCount') AS INTEGER) AS mix_minus_edge_count,
            CAST(json_extract(detail_json, '$.collaboration.mirrorFanoutEdgeCount') AS INTEGER) AS mirror_fanout_edge_count,
            CAST(json_extract(detail_json, '$.collaboration.bundleAttachmentCount') AS INTEGER)
                AS bundle_attachment_count,
            CAST(json_extract(detail_json, '$.collaboration.bundleMixerCount') AS INTEGER)
                AS bundle_mixer_count,
            CAST(json_extract(detail_json, '$.collaboration.bundleFanoutCount') AS INTEGER)
                AS bundle_fanout_count,
            CAST(json_extract(detail_json, '$.collaboration.bundleReturnCount') AS INTEGER)
                AS bundle_return_count,
            CAST(json_extract(detail_json, '$.collaboration.mediaStageCount') AS INTEGER)
                AS media_stage_count,
            CAST(json_extract(detail_json, '$.collaboration.mediaOutputTargetCount') AS INTEGER)
                AS media_output_target_count,
            CAST(json_extract(detail_json, '$.collaboration.mediaReturnTargetCount') AS INTEGER)
                AS media_return_target_count,
            CAST(json_extract(detail_json, '$.collaboration.mediaInputParticipantCount') AS INTEGER)
                AS media_input_participant_count,
            CAST(json_extract(detail_json, '$.collaboration.mediaMixMinusParticipantCount') AS INTEGER)
                AS media_mix_minus_participant_count,
            CAST(json_extract(detail_json, '$.targets.count') AS INTEGER) AS runtime_target_count,
            CAST(json_extract(detail_json, '$.targets.playbackEnabledCount') AS INTEGER) AS playback_target_count,
            CAST(json_extract(detail_json, '$.targets.recordingEnabledCount') AS INTEGER) AS recording_target_count,
            CAST(json_extract(detail_json, '$.targets.variantCount') AS INTEGER) AS variant_target_count,
            CAST(json_extract(detail_json, '$.targets.collaborationCount') AS INTEGER) AS collaboration_target_count,
            CAST(json_extract(detail_json, '$.targets.programCount') AS INTEGER) AS program_target_count,
            CAST(json_extract(detail_json, '$.targets.audioCount') AS INTEGER) AS audio_target_count,
            CAST(json_extract(detail_json, '$.targets.engineCount') AS INTEGER) AS engine_target_count,
            CAST(json_extract(detail_json, '$.targets.hostChannelCount') AS INTEGER) AS host_channel_count,
            CAST(json_extract(detail_json, '$.targets.mirrorChannelCount') AS INTEGER) AS mirror_channel_count,
            CAST(json_extract(detail_json, '$.targets.sharedProgramMirrorChannelCount') AS INTEGER)
                AS shared_program_mirror_channel_count,
            CAST(json_extract(detail_json, '$.targets.guestIsolatedMirrorChannelCount') AS INTEGER)
                AS guest_isolated_mirror_channel_count,
            CAST(json_extract(detail_json, '$.targets.archiveCount') AS INTEGER) AS archive_target_count,
            CAST(json_extract(detail_json, '$.targets.activeCount') AS INTEGER) AS active_target_count,
            CAST(json_extract(detail_json, '$.targets.degradedCount') AS INTEGER) AS degraded_target_count,
            CAST(json_extract(detail_json, '$.targets.armedCount') AS INTEGER) AS armed_target_count,
            CAST(json_extract(detail_json, '$.targets.pendingSourceCount') AS INTEGER)
                AS pending_source_target_count,
            json_extract(detail_json, '$.delivery.runtimeClass') AS runtime_class,
            json_extract(detail_json, '$.delivery.latencyProfile') AS latency_profile,
            json_extract(detail_json, '$.delivery.ladderPolicy') AS ladder_policy,
            json_extract(detail_json, '$.delivery.contentClass') AS content_class
        FROM live_runtime_telemetry
        WHERE {scope_column} = ?
        ORDER BY collected_at DESC
        LIMIT 1
        "#
    );
    sqlx::query(&latest_query)
        .bind(scope_value)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)
}

pub(super) async fn fetch_latest_failure_row(
    pool: &SqlitePool,
    scope_column: &str,
    scope_value: &str,
) -> AppResult<Option<sqlx::sqlite::SqliteRow>> {
    let failure_query = format!(
        r#"
        SELECT
            collected_at,
            CASE
                WHEN runtime_state = 'failed' THEN runtime_state
                WHEN packaging_status = 'failed' THEN 'packaging_failed'
                WHEN archive_status = 'failed' THEN 'archive_failed'
                ELSE runtime_state
            END AS failure_state,
            COALESCE(
                json_extract(detail_json, '$.lastError'),
                json_extract(detail_json, '$.runtimeOutput.lastError'),
                json_extract(detail_json, '$.reported.lastError')
            ) AS last_error
        FROM live_runtime_telemetry
        WHERE {scope_column} = ?
          AND (
              runtime_state = 'failed'
              OR packaging_status = 'failed'
              OR archive_status = 'failed'
          )
        ORDER BY collected_at DESC
        LIMIT 1
        "#
    );
    sqlx::query(&failure_query)
        .bind(scope_value)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)
}
