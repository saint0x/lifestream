use super::overview_creators::fetch_creator_breakdown;
use super::overview_metrics::{fetch_latency_row, fetch_overview_row, fetch_telemetry_row};
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

    let overview_row = fetch_overview_row(pool, scoped_filter).await?;
    let telemetry_row = fetch_telemetry_row(pool, scoped_filter).await?;
    let latency_row = fetch_latency_row(pool, scoped_filter).await?;
    let creator_breakdown = fetch_creator_breakdown(pool, scoped_filter).await?;

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
        peak_shared_program_mirror_channel_count: telemetry_row
            .get::<Option<i64>, _>("peak_shared_program_mirror_channel_count")
            .unwrap_or(0),
        peak_guest_isolated_mirror_channel_count: telemetry_row
            .get::<Option<i64>, _>("peak_guest_isolated_mirror_channel_count")
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
        last_shared_program_mirror_channel_count: telemetry_row
            .get("last_shared_program_mirror_channel_count"),
        last_guest_isolated_mirror_channel_count: telemetry_row
            .get("last_guest_isolated_mirror_channel_count"),
        last_archive_target_count: telemetry_row.get("last_archive_target_count"),
        last_active_target_count: telemetry_row.get("last_active_target_count"),
        last_degraded_target_count: telemetry_row.get("last_degraded_target_count"),
        last_armed_target_count: telemetry_row.get("last_armed_target_count"),
        last_pending_source_target_count: telemetry_row.get("last_pending_source_target_count"),
        creator_breakdown,
    })
}
