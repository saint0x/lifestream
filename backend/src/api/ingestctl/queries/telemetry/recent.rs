use super::*;

pub(crate) async fn fetch_live_runtime_telemetry_for_session(
    pool: &SqlitePool,
    session_id: &str,
    limit: i64,
) -> AppResult<Vec<LiveRuntimeTelemetry>> {
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, creator_id, broadcast_id, sample_kind, runtime_state,
               packaging_status, archive_status, bitrate_kbps, viewers, dropped_frames,
               cpu_percent, free_disk_gb, detail_json, collected_at
        FROM live_runtime_telemetry
        WHERE session_id = ?
        ORDER BY collected_at DESC
        LIMIT ?
        "#,
    )
    .bind(session_id)
    .bind(limit.max(1))
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(live_runtime_telemetry_from_row)
        .collect()
}

pub(crate) async fn fetch_recent_live_runtime_telemetry(
    pool: &SqlitePool,
    creator_id: &str,
    limit: i64,
) -> AppResult<Vec<LiveRuntimeTelemetry>> {
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, creator_id, broadcast_id, sample_kind, runtime_state,
               packaging_status, archive_status, bitrate_kbps, viewers, dropped_frames,
               cpu_percent, free_disk_gb, detail_json, collected_at
        FROM live_runtime_telemetry
        WHERE creator_id = ?
        ORDER BY collected_at DESC
        LIMIT ?
        "#,
    )
    .bind(creator_id)
    .bind(limit.max(1))
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(live_runtime_telemetry_from_row)
        .collect()
}
