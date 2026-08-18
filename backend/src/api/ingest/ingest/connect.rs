use super::*;
use crate::api::ingestctl::record_live_runtime_telemetry;

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
    let (protocol, contribution_class) = normalize_protocol_and_class(&input.protocol)?;

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
    let mut previous_session_id = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM live_ingest_sessions
        WHERE creator_id = ? AND broadcast_id = ?
        ORDER BY connected_at DESC
        LIMIT 1
        "#,
    )
    .bind(&creator.id)
    .bind(&current_broadcast_id)
    .fetch_optional(&state.pool)
    .await?;

    if let Some(existing) = fetch_active_live_ingest_session(&state.pool, &creator.id).await? {
        if is_live_ingest_session_stale(&existing) {
            previous_session_id = Some(existing.id.clone());
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
            contribution_class, contribution_state, ingest_server, status, bitrate_kbps, viewers,
            dropped_frames, connected_at, last_heartbeat_at, disconnected_at, previous_session_id
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&session_id)
    .bind(&creator.id)
    .bind(&current_broadcast_id)
    .bind(hash_token(input.stream_key.trim()))
    .bind(hash_token(&ingest_token))
    .bind(&protocol)
    .bind(&contribution_class)
    .bind("awaiting_probe")
    .bind(input.ingest_server.trim())
    .bind("connected")
    .bind(0_i64)
    .bind(0_i64)
    .bind(0_i64)
    .bind(&now)
    .bind(&now)
    .bind(Option::<String>::None)
    .bind(&previous_session_id)
    .execute(&state.pool)
    .await?;
    let session = fetch_live_ingest_session_by_id(&state.pool, &creator.id, &session_id).await?;
    initialize_live_runtime_output(&state.pool, &session).await?;
    persist_live_runtime_spec(&state, &session).await?;
    record_live_runtime_telemetry(
        &state.pool,
        &session,
        "session_connected",
        "pending_attach",
        "pending",
        "not_started",
        Some(0),
        Some(0.0),
        json!({
            "protocol": protocol.clone(),
            "contributionClass": contribution_class.clone(),
            "ingestServer": input.ingest_server.trim(),
            "reconnectSession": is_reconnect,
            "previousSessionId": previous_session_id,
        }),
    )
    .await?;
    write_live_ingest_event(
        &state.pool,
        &session_id,
        &creator.id,
        &current_broadcast_id,
        "connected",
        json!({
            "protocol": protocol,
            "contributionClass": contribution_class,
            "ingestServer": input.ingest_server.trim(),
            "reconnectSession": is_reconnect,
            "previousSessionId": previous_session_id,
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
    publish_creator_live_state(&state, &creator.id).await?;
    Ok(Json(IngestConnectResponse {
        session,
        ingest_token,
        live_stream_id: format!("lv-{}-live", creator.handle),
    }))
}

fn normalize_protocol_and_class(protocol: &str) -> AppResult<(String, String)> {
    let normalized = protocol.trim().to_ascii_lowercase();
    let contribution_class = match normalized.as_str() {
        "rtmp" | "rtmps" => "rtmp_push",
        "srt" => "srt_caller",
        _ => {
            return Err(AppError::BadRequest(
                "unsupported ingest protocol; expected rtmp, rtmps, or srt".to_string(),
            ));
        }
    };
    Ok((normalized, contribution_class.to_string()))
}
