use super::*;
use crate::models::{LiveSourceProbe, LiveSourceValidationIssue, LiveSourceValidationReport};

pub(crate) async fn fetch_active_live_ingest_session(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Option<LiveIngestSession>> {
    reconcile_stale_live_ingest_sessions_for_read(pool, Some(creator_id), None).await?;
    fetch_active_live_ingest_session_unreconciled(pool, creator_id).await
}

pub(crate) async fn fetch_active_live_ingest_session_unreconciled(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Option<LiveIngestSession>> {
    let row = sqlx::query(
        r#"
        SELECT id, creator_id, broadcast_id, previous_session_id, protocol, contribution_class, contribution_state,
               ingest_server, ingest_latency_ms, source_container_format, source_video_codec,
               source_audio_codec, source_width, source_height, source_frame_rate,
               source_audio_sample_rate_hz, source_audio_channels, last_source_probe_at,
               source_validation_state, source_validation_issues_json, status,
               bitrate_kbps, viewers, dropped_frames, connected_at, last_heartbeat_at,
               disconnected_at
        FROM live_ingest_sessions
        WHERE creator_id = ? AND status = 'connected'
        ORDER BY connected_at DESC
        LIMIT 1
        "#,
    )
    .bind(creator_id)
    .fetch_optional(pool)
    .await?;

    row.map(live_ingest_session_from_row).transpose()
}

pub(crate) async fn count_live_ingest_sessions_for_broadcast(
    pool: &SqlitePool,
    creator_id: &str,
    broadcast_id: &str,
) -> AppResult<i64> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS count
        FROM live_ingest_sessions
        WHERE creator_id = ? AND broadcast_id = ?
        "#,
    )
    .bind(creator_id)
    .bind(broadcast_id)
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}

pub(crate) async fn fetch_live_ingest_session_by_id(
    pool: &SqlitePool,
    creator_id: &str,
    session_id: &str,
) -> AppResult<LiveIngestSession> {
    reconcile_stale_live_ingest_sessions_for_read(pool, Some(creator_id), Some(session_id)).await?;
    fetch_live_ingest_session_by_id_unreconciled(pool, creator_id, session_id).await
}

pub(crate) async fn fetch_live_ingest_session_by_id_unreconciled(
    pool: &SqlitePool,
    creator_id: &str,
    session_id: &str,
) -> AppResult<LiveIngestSession> {
    let row = sqlx::query(
        r#"
        SELECT id, creator_id, broadcast_id, previous_session_id, protocol, contribution_class, contribution_state,
               ingest_server, ingest_latency_ms, source_container_format, source_video_codec,
               source_audio_codec, source_width, source_height, source_frame_rate,
               source_audio_sample_rate_hz, source_audio_channels, last_source_probe_at,
               source_validation_state, source_validation_issues_json, status,
               bitrate_kbps, viewers, dropped_frames, connected_at, last_heartbeat_at,
               disconnected_at
        FROM live_ingest_sessions
        WHERE creator_id = ? AND id = ?
        "#,
    )
    .bind(creator_id)
    .bind(session_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    live_ingest_session_from_row(row)
}

pub(crate) async fn fetch_recent_live_ingest_sessions(
    pool: &SqlitePool,
    creator_id: &str,
    limit: i64,
) -> AppResult<Vec<LiveIngestSession>> {
    reconcile_stale_live_ingest_sessions_for_read(pool, Some(creator_id), None).await?;
    let rows = sqlx::query(
        r#"
        SELECT id, creator_id, broadcast_id, previous_session_id, protocol, contribution_class, contribution_state,
               ingest_server, ingest_latency_ms, source_container_format, source_video_codec,
               source_audio_codec, source_width, source_height, source_frame_rate,
               source_audio_sample_rate_hz, source_audio_channels, last_source_probe_at,
               source_validation_state, source_validation_issues_json, status,
               bitrate_kbps, viewers, dropped_frames, connected_at, last_heartbeat_at,
               disconnected_at
        FROM live_ingest_sessions
        WHERE creator_id = ?
        ORDER BY connected_at DESC
        LIMIT ?
        "#,
    )
    .bind(creator_id)
    .bind(limit.max(1))
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(live_ingest_session_from_row).collect()
}

pub(crate) async fn fetch_live_ingest_session_by_id_global(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<LiveIngestSession> {
    reconcile_stale_live_ingest_sessions_for_read(pool, None, Some(session_id)).await?;
    fetch_live_ingest_session_by_id_global_unreconciled(pool, session_id).await
}

pub(crate) async fn fetch_live_ingest_session_by_id_global_unreconciled(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<LiveIngestSession> {
    let row = sqlx::query(
        r#"
        SELECT id, creator_id, broadcast_id, previous_session_id, protocol, contribution_class, contribution_state,
               ingest_server, ingest_latency_ms, source_container_format, source_video_codec,
               source_audio_codec, source_width, source_height, source_frame_rate,
               source_audio_sample_rate_hz, source_audio_channels, last_source_probe_at,
               source_validation_state, source_validation_issues_json, status,
               bitrate_kbps, viewers, dropped_frames, connected_at, last_heartbeat_at,
               disconnected_at
        FROM live_ingest_sessions
        WHERE id = ?
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    live_ingest_session_from_row(row)
}

pub(crate) async fn fetch_terminalizable_live_ingest_sessions_for_broadcast(
    pool: &SqlitePool,
    creator_id: &str,
    broadcast_id: &str,
) -> AppResult<Vec<LiveIngestSession>> {
    let rows = sqlx::query(
        r#"
        SELECT id, creator_id, broadcast_id, previous_session_id, protocol, contribution_class, contribution_state,
               ingest_server, ingest_latency_ms, source_container_format, source_video_codec,
               source_audio_codec, source_width, source_height, source_frame_rate,
               source_audio_sample_rate_hz, source_audio_channels, last_source_probe_at,
               source_validation_state, source_validation_issues_json, status,
               bitrate_kbps, viewers, dropped_frames, connected_at, last_heartbeat_at,
               disconnected_at
        FROM live_ingest_sessions
        WHERE creator_id = ?
          AND broadcast_id = ?
          AND status IN ('connected', 'stale')
        ORDER BY connected_at DESC
        "#,
    )
    .bind(creator_id)
    .bind(broadcast_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(live_ingest_session_from_row).collect()
}

pub(crate) fn live_ingest_session_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> AppResult<LiveIngestSession> {
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
        source_probe: live_source_probe_from_row(&row),
        source_validation: live_source_validation_from_row(&row)?,
        status: row.get("status"),
        bitrate_kbps: row.get("bitrate_kbps"),
        viewers: row.get("viewers"),
        dropped_frames: row.get("dropped_frames"),
        connected_at: row.get("connected_at"),
        last_heartbeat_at: row.get("last_heartbeat_at"),
        disconnected_at: row.get("disconnected_at"),
    })
}

fn live_source_probe_from_row(row: &sqlx::sqlite::SqliteRow) -> Option<LiveSourceProbe> {
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

fn live_source_validation_from_row(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<Option<LiveSourceValidationReport>> {
    let validated_at = row.get::<Option<String>, _>("last_source_probe_at");
    let Some(validated_at) = validated_at else {
        return Ok(None);
    };
    let issues = serde_json::from_str::<Vec<LiveSourceValidationIssue>>(
        &row.get::<String, _>("source_validation_issues_json"),
    )
    .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(Some(LiveSourceValidationReport {
        state: row.get("source_validation_state"),
        issues,
        validated_at,
    }))
}
