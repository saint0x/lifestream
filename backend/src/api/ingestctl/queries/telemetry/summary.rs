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
                WHEN json_extract(detail_json, '$.advisory.status') = 'critical'
                THEN 1 ELSE 0 END
            ) AS advisory_critical_samples,
            SUM(CASE
                WHEN json_extract(detail_json, '$.advisory.status') = 'repairable'
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
    let total_row = sqlx::query(&summary_query)
        .bind(scope_value)
        .fetch_one(pool)
        .await?;

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
        last_active_output_routes: latest_row
            .as_ref()
            .and_then(|row| row.get("active_output_routes")),
        last_audio_mix_mode: latest_row
            .as_ref()
            .and_then(|row| row.get("audio_mix_mode")),
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
        last_latency_profile: latest_row.as_ref().and_then(|row| row.get("latency_profile")),
        last_ladder_policy: latest_row.as_ref().and_then(|row| row.get("ladder_policy")),
        last_content_class: latest_row.as_ref().and_then(|row| row.get("content_class")),
        last_failure_at: failure_row.as_ref().map(|row| row.get("collected_at")),
        last_failure_state: failure_row.as_ref().map(|row| row.get("failure_state")),
        last_error: failure_row.as_ref().and_then(|row| row.get("last_error")),
    })
}

async fn fetch_latest_telemetry_row(
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
            CAST(json_extract(detail_json, '$.outputs.activeRouteCount') AS INTEGER) AS active_output_routes,
            json_extract(detail_json, '$.collaboration.audioMixMode') AS audio_mix_mode,
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

async fn fetch_latest_failure_row(
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
