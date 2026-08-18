use super::*;

pub(crate) async fn fetch_admin_live_ingest_overview(
    pool: &SqlitePool,
    creator_filter: Option<&str>,
) -> AppResult<AdminLiveIngestOverview> {
    let scoped_filter = creator_filter
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(creator_id) = scoped_filter {
        reconcile_stale_live_ingest_sessions_for_read(pool, Some(creator_id), None).await?;
    } else {
        reconcile_stale_live_ingest_sessions_for_read(pool, None, None).await?;
    }

    let overview_row = sqlx::query(
        r#"
        SELECT
            SUM(CASE WHEN s.status = 'connected' THEN 1 ELSE 0 END) AS active_sessions,
            SUM(CASE WHEN s.status = 'stale' THEN 1 ELSE 0 END) AS stale_sessions,
            SUM(CASE WHEN s.status IN ('ended', 'terminated') THEN 1 ELSE 0 END) AS terminal_sessions,
            COUNT(DISTINCT s.creator_id) AS unique_creators,
            SUM(CASE WHEN o.packaging_status IN ('ready', 'complete') THEN 1 ELSE 0 END) AS ready_outputs,
            SUM(CASE
                WHEN o.packaging_status IN ('degraded', 'failed')
                  OR o.runtime_state IN ('degraded', 'failed', 'packaging_degraded')
                THEN 1 ELSE 0 END
            ) AS degraded_outputs,
            SUM(CASE
                WHEN o.runtime_state = 'failed'
                  OR o.packaging_status = 'failed'
                  OR o.archive_status = 'failed'
                THEN 1 ELSE 0 END
            ) AS failed_outputs,
            SUM(CASE WHEN o.archive_status = 'finalizing' THEN 1 ELSE 0 END) AS archive_finalizing_outputs,
            SUM(CASE WHEN o.archive_status = 'complete' THEN 1 ELSE 0 END) AS archive_complete_outputs,
            SUM(CASE
                WHEN (
                    o.packaging_status IN ('ready', 'complete')
                    AND (o.manifest_relative_path IS NULL OR TRIM(o.manifest_relative_path) = '')
                )
                OR (
                    o.archive_status IN ('finalizing', 'complete')
                    AND (o.archive_relative_path IS NULL OR TRIM(o.archive_relative_path) = '')
                )
                THEN 1 ELSE 0 END
            ) AS artifact_attention_outputs,
            SUM(CASE
                WHEN o.packaging_status IN ('ready', 'complete')
                  AND (o.manifest_relative_path IS NULL OR TRIM(o.manifest_relative_path) = '')
                THEN 1 ELSE 0 END
            ) AS manifest_path_missing_outputs,
            SUM(CASE
                WHEN o.archive_status IN ('finalizing', 'complete')
                  AND (o.archive_relative_path IS NULL OR TRIM(o.archive_relative_path) = '')
                THEN 1 ELSE 0 END
            ) AS archive_path_missing_outputs
        FROM live_ingest_sessions s
        LEFT JOIN live_runtime_outputs o ON o.session_id = s.id
        WHERE (? IS NULL OR s.creator_id = ?)
        "#,
    )
    .bind(scoped_filter)
    .bind(scoped_filter)
    .fetch_one(pool)
    .await?;

    let telemetry_row = sqlx::query(
        r#"
        SELECT
            COUNT(*) AS total_samples,
            SUM(CASE
                WHEN runtime_state IN ('degraded', 'stale', 'disconnected', 'packaging_degraded')
                  OR packaging_status = 'degraded'
                THEN 1 ELSE 0 END
            ) AS degraded_samples,
            SUM(CASE
                WHEN runtime_state = 'failed'
                  OR packaging_status = 'failed'
                  OR archive_status = 'failed'
                THEN 1 ELSE 0 END
            ) AS failure_samples,
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
            MAX(COALESCE(CAST(json_extract(detail_json, '$.targets.hostChannelCount') AS INTEGER), 0))
                AS peak_host_channel_count,
            MAX(COALESCE(CAST(json_extract(detail_json, '$.targets.mirrorChannelCount') AS INTEGER), 0))
                AS peak_mirror_channel_count,
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
            (
                SELECT CAST(json_extract(t.detail_json, '$.targets.hostChannelCount') AS INTEGER)
                FROM live_runtime_telemetry t
                WHERE (? IS NULL OR t.creator_id = ?)
                ORDER BY t.collected_at DESC
                LIMIT 1
            ) AS last_host_channel_count,
            (
                SELECT CAST(json_extract(t.detail_json, '$.targets.mirrorChannelCount') AS INTEGER)
                FROM live_runtime_telemetry t
                WHERE (? IS NULL OR t.creator_id = ?)
                ORDER BY t.collected_at DESC
                LIMIT 1
            ) AS last_mirror_channel_count,
            (
                SELECT CAST(json_extract(t.detail_json, '$.targets.archiveCount') AS INTEGER)
                FROM live_runtime_telemetry t
                WHERE (? IS NULL OR t.creator_id = ?)
                ORDER BY t.collected_at DESC
                LIMIT 1
            ) AS last_archive_target_count,
            (
                SELECT CAST(json_extract(t.detail_json, '$.targets.activeCount') AS INTEGER)
                FROM live_runtime_telemetry t
                WHERE (? IS NULL OR t.creator_id = ?)
                ORDER BY t.collected_at DESC
                LIMIT 1
            ) AS last_active_target_count,
            (
                SELECT CAST(json_extract(t.detail_json, '$.targets.degradedCount') AS INTEGER)
                FROM live_runtime_telemetry t
                WHERE (? IS NULL OR t.creator_id = ?)
                ORDER BY t.collected_at DESC
                LIMIT 1
            ) AS last_degraded_target_count,
            (
                SELECT CAST(json_extract(t.detail_json, '$.targets.armedCount') AS INTEGER)
                FROM live_runtime_telemetry t
                WHERE (? IS NULL OR t.creator_id = ?)
                ORDER BY t.collected_at DESC
                LIMIT 1
            ) AS last_armed_target_count,
            (
                SELECT CAST(json_extract(t.detail_json, '$.targets.pendingSourceCount') AS INTEGER)
                FROM live_runtime_telemetry t
                WHERE (? IS NULL OR t.creator_id = ?)
                ORDER BY t.collected_at DESC
                LIMIT 1
            ) AS last_pending_source_target_count
        FROM live_runtime_telemetry
        WHERE (? IS NULL OR creator_id = ?)
        "#,
    )
    .bind(scoped_filter)
    .bind(scoped_filter)
    .bind(scoped_filter)
    .bind(scoped_filter)
    .bind(scoped_filter)
    .bind(scoped_filter)
    .bind(scoped_filter)
    .bind(scoped_filter)
    .bind(scoped_filter)
    .bind(scoped_filter)
    .bind(scoped_filter)
    .bind(scoped_filter)
    .bind(scoped_filter)
    .bind(scoped_filter)
    .bind(scoped_filter)
    .bind(scoped_filter)
    .fetch_one(pool)
    .await?;

    let latency_row = sqlx::query(
        r#"
        WITH ready_latencies AS (
            SELECT
                s.id AS session_id,
                CAST(strftime('%s', MIN(t.collected_at)) AS REAL) - CAST(strftime('%s', s.connected_at) AS REAL) AS ready_seconds
            FROM live_ingest_sessions s
            JOIN live_runtime_telemetry t
              ON t.session_id = s.id
             AND t.packaging_status IN ('ready', 'complete')
            WHERE (? IS NULL OR s.creator_id = ?)
            GROUP BY s.id
        ),
        archive_latencies AS (
            SELECT
                s.id AS session_id,
                CAST(strftime('%s', MIN(t.collected_at)) AS REAL) - CAST(strftime('%s', s.disconnected_at) AS REAL) AS archive_seconds
            FROM live_ingest_sessions s
            JOIN live_runtime_telemetry t
              ON t.session_id = s.id
             AND t.archive_status = 'complete'
            WHERE s.disconnected_at IS NOT NULL
              AND (? IS NULL OR s.creator_id = ?)
            GROUP BY s.id
        )
        SELECT
            (SELECT AVG(ready_seconds) FROM ready_latencies WHERE ready_seconds >= 0) AS avg_ready_latency_seconds,
            (SELECT AVG(archive_seconds) FROM archive_latencies WHERE archive_seconds >= 0) AS avg_archive_completion_seconds
        "#,
    )
    .bind(scoped_filter)
    .bind(scoped_filter)
    .bind(scoped_filter)
    .bind(scoped_filter)
    .fetch_one(pool)
    .await?;

    let creator_rows = sqlx::query(
        r#"
        SELECT
            cp.id AS creator_id,
            cp.handle AS handle,
            cp.display_name AS display_name,
            SUM(CASE WHEN s.status = 'connected' THEN 1 ELSE 0 END) AS active_sessions,
            SUM(CASE WHEN s.status = 'stale' THEN 1 ELSE 0 END) AS stale_sessions,
            SUM(CASE WHEN s.status IN ('ended', 'terminated') THEN 1 ELSE 0 END) AS terminal_sessions,
            SUM(CASE WHEN o.packaging_status IN ('ready', 'complete') THEN 1 ELSE 0 END) AS ready_outputs,
            SUM(CASE
                WHEN o.packaging_status IN ('degraded', 'failed')
                  OR o.runtime_state IN ('degraded', 'failed', 'packaging_degraded')
                THEN 1 ELSE 0 END
            ) AS degraded_outputs,
            SUM(CASE
                WHEN o.runtime_state = 'failed'
                  OR o.packaging_status = 'failed'
                  OR o.archive_status = 'failed'
                THEN 1 ELSE 0 END
            ) AS failed_outputs,
            SUM(CASE WHEN o.archive_status = 'finalizing' THEN 1 ELSE 0 END) AS archive_finalizing_outputs,
            SUM(CASE WHEN o.archive_status = 'complete' THEN 1 ELSE 0 END) AS archive_complete_outputs,
            SUM(CASE
                WHEN (
                    o.packaging_status IN ('ready', 'complete')
                    AND (o.manifest_relative_path IS NULL OR TRIM(o.manifest_relative_path) = '')
                )
                OR (
                    o.archive_status IN ('finalizing', 'complete')
                    AND (o.archive_relative_path IS NULL OR TRIM(o.archive_relative_path) = '')
                )
                THEN 1 ELSE 0 END
            ) AS artifact_attention_outputs,
            SUM(CASE
                WHEN o.packaging_status IN ('ready', 'complete')
                  AND (o.manifest_relative_path IS NULL OR TRIM(o.manifest_relative_path) = '')
                THEN 1 ELSE 0 END
            ) AS manifest_path_missing_outputs,
            SUM(CASE
                WHEN o.archive_status IN ('finalizing', 'complete')
                  AND (o.archive_relative_path IS NULL OR TRIM(o.archive_relative_path) = '')
                THEN 1 ELSE 0 END
            ) AS archive_path_missing_outputs,
            (
                SELECT lr.runtime_state
                FROM live_runtime_outputs lr
                WHERE lr.creator_id = cp.id
                ORDER BY lr.updated_at DESC
                LIMIT 1
            ) AS last_runtime_state,
            (
                SELECT lr.packaging_status
                FROM live_runtime_outputs lr
                WHERE lr.creator_id = cp.id
                ORDER BY lr.updated_at DESC
                LIMIT 1
            ) AS last_packaging_status,
            (
                SELECT lr.archive_status
                FROM live_runtime_outputs lr
                WHERE lr.creator_id = cp.id
                ORDER BY lr.updated_at DESC
                LIMIT 1
            ) AS last_archive_status,
            (
                SELECT CASE
                    WHEN lr.packaging_status IN ('ready', 'complete')
                      AND (lr.manifest_relative_path IS NULL OR TRIM(lr.manifest_relative_path) = '')
                    THEN 'missing'
                    WHEN lr.packaging_status IN ('ready', 'complete')
                      AND lr.manifest_relative_path != (
                          'live/' || lr.creator_id || '/' || lr.broadcast_id || '/' || lr.session_id || '/master.m3u8'
                      )
                    THEN 'drifted'
                    WHEN lr.packaging_status IN ('ready', 'complete')
                    THEN 'declared'
                    ELSE 'pending'
                END
                FROM live_runtime_outputs lr
                WHERE lr.creator_id = cp.id
                ORDER BY lr.updated_at DESC
                LIMIT 1
            ) AS last_manifest_artifact_state,
            (
                SELECT CASE
                    WHEN lr.archive_status IN ('finalizing', 'complete')
                      AND (lr.archive_relative_path IS NULL OR TRIM(lr.archive_relative_path) = '')
                    THEN 'missing'
                    WHEN lr.archive_status IN ('finalizing', 'complete')
                      AND lr.archive_relative_path != (
                          'archive/' || lr.creator_id || '/' || lr.broadcast_id || '/' || lr.session_id || '/final.mp4'
                      )
                    THEN 'drifted'
                    WHEN lr.archive_status IN ('finalizing', 'complete')
                    THEN 'declared'
                    ELSE 'pending'
                END
                FROM live_runtime_outputs lr
                WHERE lr.creator_id = cp.id
                ORDER BY lr.updated_at DESC
                LIMIT 1
            ) AS last_archive_artifact_state,
            (
                SELECT AVG(CAST(strftime('%s', first_ready.collected_at) AS REAL) - CAST(strftime('%s', s2.connected_at) AS REAL))
                FROM live_ingest_sessions s2
                JOIN (
                    SELECT session_id, MIN(collected_at) AS collected_at
                    FROM live_runtime_telemetry
                    WHERE packaging_status IN ('ready', 'complete')
                    GROUP BY session_id
                ) first_ready ON first_ready.session_id = s2.id
                WHERE s2.creator_id = cp.id
            ) AS avg_ready_latency_seconds,
            (
                SELECT AVG(CAST(strftime('%s', first_archive.collected_at) AS REAL) - CAST(strftime('%s', s3.disconnected_at) AS REAL))
                FROM live_ingest_sessions s3
                JOIN (
                    SELECT session_id, MIN(collected_at) AS collected_at
                    FROM live_runtime_telemetry
                    WHERE archive_status = 'complete'
                    GROUP BY session_id
                ) first_archive ON first_archive.session_id = s3.id
                WHERE s3.creator_id = cp.id
                  AND s3.disconnected_at IS NOT NULL
            ) AS avg_archive_completion_seconds,
            COALESCE((
                SELECT COUNT(*)
                FROM live_runtime_telemetry t
                WHERE t.creator_id = cp.id
            ), 0) AS total_samples,
            COALESCE((
                SELECT SUM(CASE
                    WHEN t.runtime_state IN ('degraded', 'stale', 'disconnected', 'packaging_degraded')
                      OR t.packaging_status = 'degraded'
                    THEN 1 ELSE 0 END
                )
                FROM live_runtime_telemetry t
                WHERE t.creator_id = cp.id
            ), 0) AS degraded_samples,
            COALESCE((
                SELECT SUM(CASE
                    WHEN t.runtime_state = 'failed'
                      OR t.packaging_status = 'failed'
                      OR t.archive_status = 'failed'
                    THEN 1 ELSE 0 END
                )
                FROM live_runtime_telemetry t
                WHERE t.creator_id = cp.id
            ), 0) AS failure_samples,
            COALESCE((
                SELECT SUM(CASE
                    WHEN json_extract(t.detail_json, '$.advisory.status') = 'critical'
                    THEN 1 ELSE 0 END
                )
                FROM live_runtime_telemetry t
                WHERE t.creator_id = cp.id
            ), 0) AS advisory_critical_samples,
            COALESCE((
                SELECT SUM(CASE
                    WHEN json_extract(t.detail_json, '$.advisory.status') = 'repairable'
                    THEN 1 ELSE 0 END
                )
                FROM live_runtime_telemetry t
                WHERE t.creator_id = cp.id
            ), 0) AS advisory_repairable_samples,
            COALESCE((
                SELECT SUM(CASE
                    WHEN t.sample_kind = 'runtime_artifact_reconciled'
                    THEN 1 ELSE 0 END
                )
                FROM live_runtime_telemetry t
                WHERE t.creator_id = cp.id
            ), 0) AS runtime_artifact_reconciliation_samples,
            COALESCE((
                SELECT SUM(CASE
                    WHEN t.sample_kind = 'runtime_archive_completed'
                    THEN 1 ELSE 0 END
                )
                FROM live_runtime_telemetry t
                WHERE t.creator_id = cp.id
            ), 0) AS runtime_archive_completion_samples
        FROM creator_profiles cp
        LEFT JOIN live_ingest_sessions s ON s.creator_id = cp.id
        LEFT JOIN live_runtime_outputs o ON o.session_id = s.id
        WHERE EXISTS (
            SELECT 1 FROM live_ingest_sessions sx WHERE sx.creator_id = cp.id
        )
          AND (? IS NULL OR cp.id = ?)
        GROUP BY cp.id, cp.handle, cp.display_name
        ORDER BY active_sessions DESC, stale_sessions DESC, cp.handle ASC
        "#,
    )
    .bind(scoped_filter)
    .bind(scoped_filter)
    .fetch_all(pool)
    .await?;

    let creator_breakdown = creator_rows
        .into_iter()
        .map(|row| AdminLiveIngestCreatorOverview {
            creator_id: row.get("creator_id"),
            handle: row.get("handle"),
            display_name: row.get("display_name"),
            active_sessions: row.get::<Option<i64>, _>("active_sessions").unwrap_or(0),
            stale_sessions: row.get::<Option<i64>, _>("stale_sessions").unwrap_or(0),
            terminal_sessions: row.get::<Option<i64>, _>("terminal_sessions").unwrap_or(0),
            ready_outputs: row.get::<Option<i64>, _>("ready_outputs").unwrap_or(0),
            degraded_outputs: row.get::<Option<i64>, _>("degraded_outputs").unwrap_or(0),
            failed_outputs: row.get::<Option<i64>, _>("failed_outputs").unwrap_or(0),
            archive_finalizing_outputs: row
                .get::<Option<i64>, _>("archive_finalizing_outputs")
                .unwrap_or(0),
            archive_complete_outputs: row
                .get::<Option<i64>, _>("archive_complete_outputs")
                .unwrap_or(0),
            artifact_attention_outputs: row
                .get::<Option<i64>, _>("artifact_attention_outputs")
                .unwrap_or(0),
            manifest_path_missing_outputs: row
                .get::<Option<i64>, _>("manifest_path_missing_outputs")
                .unwrap_or(0),
            archive_path_missing_outputs: row
                .get::<Option<i64>, _>("archive_path_missing_outputs")
                .unwrap_or(0),
            last_runtime_state: row.get("last_runtime_state"),
            last_packaging_status: row.get("last_packaging_status"),
            last_archive_status: row.get("last_archive_status"),
            last_manifest_artifact_state: row.get("last_manifest_artifact_state"),
            last_archive_artifact_state: row.get("last_archive_artifact_state"),
            avg_ready_latency_seconds: row.get("avg_ready_latency_seconds"),
            avg_archive_completion_seconds: row.get("avg_archive_completion_seconds"),
            total_samples: row.get::<Option<i64>, _>("total_samples").unwrap_or(0),
            degraded_samples: row.get::<Option<i64>, _>("degraded_samples").unwrap_or(0),
            failure_samples: row.get::<Option<i64>, _>("failure_samples").unwrap_or(0),
            advisory_critical_samples: row
                .get::<Option<i64>, _>("advisory_critical_samples")
                .unwrap_or(0),
            advisory_repairable_samples: row
                .get::<Option<i64>, _>("advisory_repairable_samples")
                .unwrap_or(0),
            runtime_artifact_reconciliation_samples: row
                .get::<Option<i64>, _>("runtime_artifact_reconciliation_samples")
                .unwrap_or(0),
            runtime_archive_completion_samples: row
                .get::<Option<i64>, _>("runtime_archive_completion_samples")
                .unwrap_or(0),
        })
        .collect();

    Ok(AdminLiveIngestOverview {
        active_sessions: overview_row
            .get::<Option<i64>, _>("active_sessions")
            .unwrap_or(0),
        stale_sessions: overview_row
            .get::<Option<i64>, _>("stale_sessions")
            .unwrap_or(0),
        terminal_sessions: overview_row
            .get::<Option<i64>, _>("terminal_sessions")
            .unwrap_or(0),
        unique_creators: overview_row
            .get::<Option<i64>, _>("unique_creators")
            .unwrap_or(0),
        ready_outputs: overview_row
            .get::<Option<i64>, _>("ready_outputs")
            .unwrap_or(0),
        degraded_outputs: overview_row
            .get::<Option<i64>, _>("degraded_outputs")
            .unwrap_or(0),
        failed_outputs: overview_row
            .get::<Option<i64>, _>("failed_outputs")
            .unwrap_or(0),
        archive_finalizing_outputs: overview_row
            .get::<Option<i64>, _>("archive_finalizing_outputs")
            .unwrap_or(0),
        archive_complete_outputs: overview_row
            .get::<Option<i64>, _>("archive_complete_outputs")
            .unwrap_or(0),
        artifact_attention_outputs: overview_row
            .get::<Option<i64>, _>("artifact_attention_outputs")
            .unwrap_or(0),
        manifest_path_missing_outputs: overview_row
            .get::<Option<i64>, _>("manifest_path_missing_outputs")
            .unwrap_or(0),
        archive_path_missing_outputs: overview_row
            .get::<Option<i64>, _>("archive_path_missing_outputs")
            .unwrap_or(0),
        avg_ready_latency_seconds: latency_row.get("avg_ready_latency_seconds"),
        avg_archive_completion_seconds: latency_row.get("avg_archive_completion_seconds"),
        total_samples: telemetry_row
            .get::<Option<i64>, _>("total_samples")
            .unwrap_or(0),
        degraded_samples: telemetry_row
            .get::<Option<i64>, _>("degraded_samples")
            .unwrap_or(0),
        failure_samples: telemetry_row
            .get::<Option<i64>, _>("failure_samples")
            .unwrap_or(0),
        advisory_critical_samples: telemetry_row
            .get::<Option<i64>, _>("advisory_critical_samples")
            .unwrap_or(0),
        advisory_repairable_samples: telemetry_row
            .get::<Option<i64>, _>("advisory_repairable_samples")
            .unwrap_or(0),
        runtime_artifact_reconciliation_samples: telemetry_row
            .get::<Option<i64>, _>("runtime_artifact_reconciliation_samples")
            .unwrap_or(0),
        runtime_archive_completion_samples: telemetry_row
            .get::<Option<i64>, _>("runtime_archive_completion_samples")
            .unwrap_or(0),
        peak_host_channel_count: telemetry_row
            .get::<Option<i64>, _>("peak_host_channel_count")
            .unwrap_or(0),
        peak_mirror_channel_count: telemetry_row
            .get::<Option<i64>, _>("peak_mirror_channel_count")
            .unwrap_or(0),
        peak_archive_target_count: telemetry_row
            .get::<Option<i64>, _>("peak_archive_target_count")
            .unwrap_or(0),
        peak_active_target_count: telemetry_row
            .get::<Option<i64>, _>("peak_active_target_count")
            .unwrap_or(0),
        peak_degraded_target_count: telemetry_row
            .get::<Option<i64>, _>("peak_degraded_target_count")
            .unwrap_or(0),
        peak_armed_target_count: telemetry_row
            .get::<Option<i64>, _>("peak_armed_target_count")
            .unwrap_or(0),
        peak_pending_source_target_count: telemetry_row
            .get::<Option<i64>, _>("peak_pending_source_target_count")
            .unwrap_or(0),
        last_host_channel_count: telemetry_row.get("last_host_channel_count"),
        last_mirror_channel_count: telemetry_row.get("last_mirror_channel_count"),
        last_archive_target_count: telemetry_row.get("last_archive_target_count"),
        last_active_target_count: telemetry_row.get("last_active_target_count"),
        last_degraded_target_count: telemetry_row.get("last_degraded_target_count"),
        last_armed_target_count: telemetry_row.get("last_armed_target_count"),
        last_pending_source_target_count: telemetry_row
            .get("last_pending_source_target_count"),
        creator_breakdown,
    })
}
