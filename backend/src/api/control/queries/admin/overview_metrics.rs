use super::*;
use sqlx::sqlite::SqliteRow;

pub(super) async fn fetch_overview_row(
    pool: &SqlitePool,
    scoped_filter: Option<&str>,
) -> AppResult<SqliteRow> {
    sqlx::query(
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
    .await
    .map_err(Into::into)
}

pub(super) async fn fetch_telemetry_row(
    pool: &SqlitePool,
    scoped_filter: Option<&str>,
) -> AppResult<SqliteRow> {
    sqlx::query(
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
                SELECT CAST(json_extract(t.detail_json, '$.targets.sharedProgramMirrorChannelCount') AS INTEGER)
                FROM live_runtime_telemetry t
                WHERE (? IS NULL OR t.creator_id = ?)
                ORDER BY t.collected_at DESC
                LIMIT 1
            ) AS last_shared_program_mirror_channel_count,
            (
                SELECT CAST(json_extract(t.detail_json, '$.targets.guestIsolatedMirrorChannelCount') AS INTEGER)
                FROM live_runtime_telemetry t
                WHERE (? IS NULL OR t.creator_id = ?)
                ORDER BY t.collected_at DESC
                LIMIT 1
            ) AS last_guest_isolated_mirror_channel_count,
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
    .bind(scoped_filter)
    .bind(scoped_filter)
    .bind(scoped_filter)
    .bind(scoped_filter)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

pub(super) async fn fetch_latency_row(
    pool: &SqlitePool,
    scoped_filter: Option<&str>,
) -> AppResult<SqliteRow> {
    sqlx::query(
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
    .await
    .map_err(Into::into)
}
