use super::*;
use crate::api::control::{
    fetch_live_runtime_output_for_session, persist_live_runtime_spec, record_live_runtime_telemetry,
};

use super::probe::{assess_source_validation, determine_contribution_state, merge_source_probe};

pub(crate) async fn heartbeat_live_ingest(
    State(state): State<SharedState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<IngestHeartbeatRequest>,
) -> AppResult<Json<LiveIngestSession>> {
    let ingest_token = require_ingest_token(&headers)?;
    let session = validate_live_ingest_session(&state.pool, &session_id, &ingest_token).await?;
    if input.bitrate_kbps < 0 || input.viewers < 0 || input.dropped_frames < 0 {
        return Err(AppError::BadRequest(
            "heartbeat counters must be non-negative".to_string(),
        ));
    }
    if let Some(ingest_latency_ms) = input.ingest_latency_ms {
        if ingest_latency_ms < 0 {
            return Err(AppError::BadRequest(
                "ingestLatencyMs must be non-negative".to_string(),
            ));
        }
    }

    let creator = fetch_creator_profile(&state.pool, &session.creator_id).await?;
    let broadcast =
        fetch_broadcast_by_id(&state.pool, &session.creator_id, &session.broadcast_id).await?;
    let now = Utc::now().to_rfc3339();
    let source_probe = merge_source_probe(
        session.source_probe.as_ref(),
        input.source_probe.as_ref(),
        &now,
    )?;
    let source_validation = assess_source_validation(source_probe.as_ref(), &now);
    let contribution_state = determine_contribution_state(
        &session,
        &input,
        source_probe.is_some(),
        source_validation.as_ref(),
    );
    let effective_ingest_latency_ms = input.ingest_latency_ms.or(session.ingest_latency_ms);

    sqlx::query(
        r#"
        UPDATE live_ingest_sessions
        SET bitrate_kbps = ?,
            viewers = ?,
            dropped_frames = ?,
            contribution_state = ?,
            ingest_latency_ms = ?,
            source_container_format = ?,
            source_video_codec = ?,
            source_audio_codec = ?,
            source_width = ?,
            source_height = ?,
            source_frame_rate = ?,
            source_audio_sample_rate_hz = ?,
            source_audio_channels = ?,
            last_source_probe_at = ?,
            source_validation_state = ?,
            source_validation_issues_json = ?,
            last_heartbeat_at = ?,
            status = 'connected'
        WHERE id = ?
        "#,
    )
    .bind(input.bitrate_kbps)
    .bind(input.viewers)
    .bind(input.dropped_frames)
    .bind(&contribution_state)
    .bind(effective_ingest_latency_ms)
    .bind(
        source_probe
            .as_ref()
            .and_then(|item| item.container_format.clone()),
    )
    .bind(
        source_probe
            .as_ref()
            .and_then(|item| item.video_codec.clone()),
    )
    .bind(
        source_probe
            .as_ref()
            .and_then(|item| item.audio_codec.clone()),
    )
    .bind(source_probe.as_ref().and_then(|item| item.width))
    .bind(source_probe.as_ref().and_then(|item| item.height))
    .bind(source_probe.as_ref().and_then(|item| item.frame_rate))
    .bind(
        source_probe
            .as_ref()
            .and_then(|item| item.audio_sample_rate_hz),
    )
    .bind(source_probe.as_ref().and_then(|item| item.audio_channels))
    .bind(source_probe.as_ref().map(|item| item.probed_at.clone()))
    .bind(
        source_validation
            .as_ref()
            .map(|item| item.state.as_str())
            .unwrap_or("awaiting_probe"),
    )
    .bind(
        serde_json::to_string(
            &source_validation
                .as_ref()
                .map(|item| item.issues.clone())
                .unwrap_or_default(),
        )
        .map_err(|error| AppError::Internal(error.to_string()))?,
    )
    .bind(&now)
    .bind(&session_id)
    .execute(&state.pool)
    .await?;

    crate::api::creator::ensure_creator_live_settings_row(&state.pool, &session.creator_id)
        .await?;
    sqlx::query(
        r#"
        UPDATE creator_live_settings
        SET bitrate_kbps = ?, cpu_percent = ?, dropped_frames = ?, free_disk_gb = ?
        WHERE creator_id = ?
        "#,
    )
    .bind(input.bitrate_kbps)
    .bind(input.cpu_percent.unwrap_or(0))
    .bind(input.dropped_frames)
    .bind(input.free_disk_gb.unwrap_or(0.0))
    .bind(&session.creator_id)
    .execute(&state.pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO creator_stream_health_samples (
            id, creator_id, collected_at, bitrate_kbps, viewers, cpu_percent, dropped_frames, free_disk_gb
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(format!("hls-{}", Uuid::new_v4().simple()))
    .bind(&session.creator_id)
    .bind(&now)
    .bind(input.bitrate_kbps)
    .bind(input.viewers)
    .bind(input.cpu_percent.unwrap_or(0))
    .bind(input.dropped_frames)
    .bind(input.free_disk_gb.unwrap_or(0.0))
    .execute(&state.pool)
    .await?;
    write_live_ingest_event(
        &state.pool,
        &session_id,
        &session.creator_id,
        &session.broadcast_id,
        "heartbeat_recorded",
        json!({
            "bitrateKbps": input.bitrate_kbps,
            "viewers": input.viewers,
            "droppedFrames": input.dropped_frames,
            "cpuPercent": input.cpu_percent,
            "freeDiskGb": input.free_disk_gb,
            "ingestLatencyMs": effective_ingest_latency_ms,
            "contributionState": contribution_state.clone(),
            "sourceProbe": source_probe.clone(),
            "sourceValidation": source_validation.clone(),
        }),
    )
    .await?;

    sqlx::query(
        "UPDATE broadcasts SET peak_viewers = MAX(peak_viewers, ?), average_viewers = ? WHERE id = ?",
    )
    .bind(input.viewers)
    .bind(input.viewers)
    .bind(&session.broadcast_id)
    .execute(&state.pool)
    .await?;

    ensure_live_stream_row(&state.pool, &creator, &broadcast, input.viewers).await?;
    let refreshed_session =
        fetch_live_ingest_session_by_id(&state.pool, &session.creator_id, &session_id).await?;
    persist_live_runtime_spec(&state, &refreshed_session).await?;
    let runtime_output = fetch_live_runtime_output_for_session(&state.pool, &session_id).await?;
    record_live_runtime_telemetry(
        &state.pool,
        &refreshed_session,
        "heartbeat",
        runtime_output
            .as_ref()
            .map(|item| item.runtime_state.as_str())
            .unwrap_or("pending_attach"),
        runtime_output
            .as_ref()
            .map(|item| item.packaging_status.as_str())
            .unwrap_or("pending"),
        runtime_output
            .as_ref()
            .map(|item| item.archive_status.as_str())
            .unwrap_or("not_started"),
        input.cpu_percent,
        input.free_disk_gb,
        json!({
            "source": "heartbeat",
            "contributionState": contribution_state,
            "ingestLatencyMs": effective_ingest_latency_ms,
            "sourceProbe": source_probe.clone(),
            "sourceValidation": source_validation.clone(),
        }),
    )
    .await?;
    publish_current_creator_live_state(&state, &session.creator_id).await?;
    Ok(Json(refreshed_session))
}
