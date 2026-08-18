use super::*;

pub(crate) async fn connect_live_ingest(
    State(state): State<SharedState>,
    Json(input): Json<IngestConnectRequest>,
) -> AppResult<Json<IngestConnectResponse>> {
    if input.stream_key.trim().is_empty()
        || input.protocol.trim().is_empty()
        || input.ingest_server.trim().is_empty()
    {
        return Err(AppError::BadRequest(
            "streamKey, protocol, and ingestServer are required".to_string(),
        ));
    }

    let creator = fetch_creator_profile_by_stream_key(&state.pool, input.stream_key.trim()).await?;
    ensure_creator_live_streaming_enabled(&state.pool, &creator.id).await?;
    let current_broadcast_id = match input.broadcast_id.as_deref() {
        Some(broadcast_id) => broadcast_id.to_string(),
        None => creator
            .current_broadcast_id
            .clone()
            .ok_or_else(|| AppError::BadRequest("creator has no pending broadcast".to_string()))?,
    };
    let broadcast = fetch_broadcast_by_id(&state.pool, &creator.id, &current_broadcast_id).await?;
    if broadcast.status != "ready" && broadcast.status != "live" {
        return Err(AppError::BadRequest(
            "broadcast is not available for ingest".to_string(),
        ));
    }
    let is_reconnect =
        count_live_ingest_sessions_for_broadcast(&state.pool, &creator.id, &current_broadcast_id)
            .await?
            > 0;

    if let Some(existing) = fetch_active_live_ingest_session(&state.pool, &creator.id).await? {
        if is_live_ingest_session_stale(&existing) {
            mark_live_ingest_session_stale(&state, &existing).await?;
        } else if existing.broadcast_id != current_broadcast_id {
            return Err(AppError::BadRequest(
                "another ingest session is already active".to_string(),
            ));
        } else {
            return Err(AppError::BadRequest(
                "ingest session already active for this broadcast".to_string(),
            ));
        }
    }

    let now = Utc::now().to_rfc3339();
    let session_id = format!("ing-{}", Uuid::new_v4().simple());
    let ingest_token = format!("igt_{}", Uuid::new_v4().simple());
    sqlx::query(
        r#"
        INSERT INTO live_ingest_sessions (
            id, creator_id, broadcast_id, stream_key_hash, ingest_token_hash, protocol,
            ingest_server, status, bitrate_kbps, viewers, dropped_frames, connected_at,
            last_heartbeat_at, disconnected_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&session_id)
    .bind(&creator.id)
    .bind(&current_broadcast_id)
    .bind(hash_token(input.stream_key.trim()))
    .bind(hash_token(&ingest_token))
    .bind(input.protocol.trim())
    .bind(input.ingest_server.trim())
    .bind("connected")
    .bind(0_i64)
    .bind(0_i64)
    .bind(0_i64)
    .bind(&now)
    .bind(&now)
    .bind(Option::<String>::None)
    .execute(&state.pool)
    .await?;
    write_live_ingest_event(
        &state.pool,
        &session_id,
        &creator.id,
        &current_broadcast_id,
        "connected",
        json!({
            "protocol": input.protocol.trim(),
            "ingestServer": input.ingest_server.trim(),
        }),
    )
    .await?;

    transition_broadcast_to_live(
        &state.pool,
        &creator,
        &broadcast,
        !is_reconnect,
        !is_reconnect,
    )
    .await?;
    if let Some(collaboration_session) =
        fetch_active_collaboration_session_for_broadcast(&state.pool, &current_broadcast_id).await?
    {
        let _ = sync_active_collaboration_mirror_pickups_for_session_and_publish(
            &state,
            &collaboration_session.id,
        )
        .await;
    }
    let session = fetch_live_ingest_session_by_id(&state.pool, &creator.id, &session_id).await?;
    publish_creator_live_state(&state, &creator.id).await?;
    Ok(Json(IngestConnectResponse {
        session,
        ingest_token,
        live_stream_id: format!("lv-{}-live", creator.handle),
    }))
}

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

    let creator = fetch_creator_profile(&state.pool, &session.creator_id).await?;
    let broadcast =
        fetch_broadcast_by_id(&state.pool, &session.creator_id, &session.broadcast_id).await?;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE live_ingest_sessions SET bitrate_kbps = ?, viewers = ?, dropped_frames = ?, last_heartbeat_at = ?, status = 'connected' WHERE id = ?",
    )
    .bind(input.bitrate_kbps)
    .bind(input.viewers)
    .bind(input.dropped_frames)
    .bind(&now)
    .bind(&session_id)
    .execute(&state.pool)
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
    publish_creator_live_state(&state, &session.creator_id).await?;
    Ok(Json(
        fetch_live_ingest_session_by_id(&state.pool, &session.creator_id, &session_id).await?,
    ))
}

pub(crate) async fn disconnect_live_ingest(
    State(state): State<SharedState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> AppResult<Json<LiveIngestSession>> {
    let ingest_token = require_ingest_token(&headers)?;
    let session = validate_live_ingest_session(&state.pool, &session_id, &ingest_token).await?;
    close_live_ingest_session(&state, &session, "ended", "disconnected", json!({})).await?;
    Ok(Json(
        fetch_live_ingest_session_by_id(&state.pool, &session.creator_id, &session_id).await?,
    ))
}
