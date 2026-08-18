use super::*;

pub(crate) async fn validate_live_ingest_session(
    pool: &SqlitePool,
    session_id: &str,
    ingest_token: &str,
) -> AppResult<LiveIngestSession> {
    validate_live_ingest_session_internal(pool, session_id, ingest_token, true).await
}

pub(crate) async fn validate_live_ingest_session_any_status(
    pool: &SqlitePool,
    session_id: &str,
    ingest_token: &str,
) -> AppResult<LiveIngestSession> {
    validate_live_ingest_session_internal(pool, session_id, ingest_token, false).await
}

async fn validate_live_ingest_session_internal(
    pool: &SqlitePool,
    session_id: &str,
    ingest_token: &str,
    require_connected: bool,
) -> AppResult<LiveIngestSession> {
    let token_hash = crate::auth::hash_token(ingest_token);
    let query = if require_connected {
        r#"
        SELECT id, creator_id, broadcast_id, previous_session_id, protocol, contribution_class, contribution_state,
               ingest_server, ingest_latency_ms, source_container_format, source_video_codec,
               source_audio_codec, source_width, source_height, source_frame_rate,
               source_audio_sample_rate_hz, source_audio_channels, last_source_probe_at,
               source_validation_state, source_validation_issues_json, status,
               bitrate_kbps, viewers, dropped_frames, connected_at, last_heartbeat_at,
               disconnected_at
        FROM live_ingest_sessions
        WHERE id = ? AND ingest_token_hash = ? AND status = 'connected'
        "#
    } else {
        r#"
        SELECT id, creator_id, broadcast_id, previous_session_id, protocol, contribution_class, contribution_state,
               ingest_server, ingest_latency_ms, source_container_format, source_video_codec,
               source_audio_codec, source_width, source_height, source_frame_rate,
               source_audio_sample_rate_hz, source_audio_channels, last_source_probe_at,
               source_validation_state, source_validation_issues_json, status,
               bitrate_kbps, viewers, dropped_frames, connected_at, last_heartbeat_at,
               disconnected_at
        FROM live_ingest_sessions
        WHERE id = ? AND ingest_token_hash = ?
        "#
    };
    let row = sqlx::query(query)
        .bind(session_id)
        .bind(token_hash)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::Unauthorized)?;

    live_ingest_session_from_row(row)
}
