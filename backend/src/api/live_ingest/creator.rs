use super::*;

pub(crate) async fn update_creator_live(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<UpdateLiveRequest>,
) -> AppResult<Json<CreatorLiveSnapshot>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-live-update:{}", identity.user_id),
        30,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    let profile = fetch_creator_profile(&state.pool, creator_id).await?;
    let next_category = input
        .category
        .clone()
        .unwrap_or_else(|| profile.default_category.clone());
    let next_tags = input.tags.clone().unwrap_or(profile.default_tags.clone());

    sqlx::query(
        "UPDATE creator_profiles SET default_category = ?, default_tags_json = ? WHERE id = ?",
    )
    .bind(&next_category)
    .bind(to_json(&next_tags)?)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;

    if let Some(current_id) = profile.current_broadcast_id {
        let current = fetch_broadcast_by_id(&state.pool, creator_id, &current_id).await?;
        sqlx::query(
            "UPDATE broadcasts SET title = ?, category = ?, tags_json = ?, is_mature = ? WHERE id = ?",
        )
        .bind(input.title.unwrap_or(current.title))
        .bind(next_category)
        .bind(to_json(&next_tags)?)
        .bind(input.is_mature.unwrap_or(current.is_mature) as i64)
        .bind(current_id)
        .execute(&state.pool)
        .await?;
    }

    get_creator_live(State(state), headers).await
}

pub(crate) async fn start_broadcast(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<StartBroadcastRequest>,
) -> AppResult<Json<Broadcast>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-start-broadcast:{}", identity.user_id),
        10,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    ensure_creator_live_streaming_enabled(&state.pool, creator_id).await?;
    let snapshot = build_creator_live_snapshot(&state.pool, creator_id).await?;
    if snapshot.current_broadcast.is_some() || snapshot.pending_broadcast.is_some() {
        return Err(AppError::BadRequest(
            "an active or pending broadcast already exists".to_string(),
        ));
    }
    if input.title.trim().is_empty() {
        return Err(AppError::BadRequest("title is required".to_string()));
    }

    let broadcast = Broadcast {
        id: Uuid::new_v4().to_string(),
        title: input.title.trim().to_string(),
        category: input.category,
        tags: input.tags,
        status: "ready".to_string(),
        started_at: Utc::now().to_rfc3339(),
        ended_at: None,
        duration_sec: None,
        peak_viewers: 0,
        average_viewers: 0,
        chat_messages: 0,
        new_followers: if input.notify_followers { 3 } else { 0 },
        new_subscribers: 0,
        revenue: 0.0,
        thumbnail: input
            .thumbnail
            .unwrap_or_else(|| "https://cdn.lifestream.local/thumb/live-start.jpg".to_string()),
        is_mature: input.is_mature,
    };

    sqlx::query(
        r#"
            INSERT INTO broadcasts (
                id, creator_id, title, category, tags_json, status, started_at, ended_at, duration_sec,
                peak_viewers, average_viewers, chat_messages, new_followers, new_subscribers,
                revenue, thumbnail, is_mature
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&broadcast.id)
    .bind(creator_id)
    .bind(&broadcast.title)
    .bind(&broadcast.category)
    .bind(to_json(&broadcast.tags)?)
    .bind(&broadcast.status)
    .bind(&broadcast.started_at)
    .bind(&broadcast.ended_at)
    .bind(&broadcast.duration_sec)
    .bind(broadcast.peak_viewers)
    .bind(broadcast.average_viewers)
    .bind(broadcast.chat_messages)
    .bind(broadcast.new_followers)
    .bind(broadcast.new_subscribers)
    .bind(broadcast.revenue)
    .bind(&broadcast.thumbnail)
    .bind(broadcast.is_mature as i64)
    .execute(&state.pool)
    .await?;

    sqlx::query(
        "UPDATE creator_profiles SET live_status = 'ready', current_broadcast_id = ?, default_category = ?, default_tags_json = ? WHERE id = ?",
    )
    .bind(&broadcast.id)
    .bind(&broadcast.category)
    .bind(to_json(&broadcast.tags)?)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;

    publish_creator_live_state(&state, creator_id).await?;
    Ok(Json(broadcast))
}

pub(crate) async fn end_broadcast(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<Broadcast>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-end-broadcast:{}", identity.user_id),
        20,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    let creator_profile = fetch_creator_profile(&state.pool, creator_id).await?;
    let broadcast = fetch_broadcast_by_id(&state.pool, creator_id, &id).await?;
    if creator_profile.current_broadcast_id.as_deref() != Some(id.as_str()) {
        return Err(AppError::BadRequest(
            "broadcast is not the creator's active or pending broadcast".to_string(),
        ));
    }
    if let Some(session) = fetch_active_live_ingest_session(&state.pool, creator_id).await? {
        if session.broadcast_id == id {
            close_live_ingest_session(
                &state,
                &session,
                "ended",
                "creator_broadcast_ended",
                json!({
                "actorUserId": identity.user_id,
                }),
            )
            .await?;
            return Ok(Json(
                fetch_broadcast_by_id(&state.pool, creator_id, &id).await?,
            ));
        }
    }
    let started_at = chrono::DateTime::parse_from_rfc3339(&broadcast.started_at)
        .map_err(|_| AppError::BadRequest("invalid broadcast timestamp".to_string()))?
        .with_timezone(&Utc);
    let ended_at = Utc::now();
    let duration_sec = (ended_at - started_at).num_seconds().max(0);

    sqlx::query(
        "UPDATE broadcasts SET status = 'ended', ended_at = ?, duration_sec = ? WHERE id = ?",
    )
    .bind(ended_at.to_rfc3339())
    .bind(duration_sec)
    .bind(&id)
    .execute(&state.pool)
    .await?;

    sqlx::query(
        "UPDATE creator_profiles SET live_status = 'offline', current_broadcast_id = NULL WHERE id = ?",
    )
    .bind(creator_id)
    .execute(&state.pool)
    .await?;
    reset_creator_live_operational_metrics(&state.pool, creator_id).await?;

    sqlx::query("UPDATE streamers SET is_live = 0 WHERE handle = ?")
        .bind(&creator_profile.handle)
        .execute(&state.pool)
        .await?;

    sqlx::query("DELETE FROM live_streams WHERE id = ?")
        .bind(format!("lv-{}-live", creator_profile.handle))
        .execute(&state.pool)
        .await?;

    let terminated_ingest_sessions =
        fetch_terminalizable_live_ingest_sessions_for_broadcast(&state.pool, creator_id, &id)
            .await?;
    sqlx::query(
        "UPDATE live_ingest_sessions SET status = 'ended', disconnected_at = ?, last_heartbeat_at = ? WHERE creator_id = ? AND broadcast_id = ? AND status IN ('connected', 'stale')",
    )
    .bind(ended_at.to_rfc3339())
    .bind(ended_at.to_rfc3339())
    .bind(creator_id)
    .bind(&id)
    .execute(&state.pool)
    .await?;
    for session in terminated_ingest_sessions {
        write_live_ingest_event(
            &state.pool,
            &session.id,
            &session.creator_id,
            &session.broadcast_id,
            "creator_broadcast_ended",
            json!({
                "status": "ended",
                "finalViewers": session.viewers,
                "finalBitrateKbps": session.bitrate_kbps,
                "finalDroppedFrames": session.dropped_frames,
                "details": {
                    "actorUserId": identity.user_id,
                },
            }),
        )
        .await?;
    }
    enqueue_creator_broadcast_ended_notification(
        &state.pool,
        &creator_profile,
        &broadcast,
        "ended",
        "creator_broadcast_ended",
    )
    .await?;

    if let Some(session) =
        fetch_active_collaboration_session_for_broadcast(&state.pool, &id).await?
    {
        let _ = end_collaboration_session_internal(
            &state,
            &session,
            Some(identity.user_id.clone()),
            json!({
                "reason": "source broadcast ended",
                "broadcastId": id,
            }),
        )
        .await?;
    }

    publish_creator_live_state(&state, creator_id).await?;
    Ok(Json(
        fetch_broadcast_by_id(&state.pool, creator_id, &id).await?,
    ))
}

pub(crate) async fn rotate_stream_key(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorProfile>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-rotate-key:{}", identity.user_id),
        5,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    let new_key = format!(
        "live_sk_{}{}",
        Uuid::new_v4().simple(),
        &Uuid::new_v4().simple().to_string()[..8]
    );
    let active_session = fetch_active_live_ingest_session(&state.pool, creator_id).await?;

    sqlx::query("UPDATE creator_profiles SET stream_key = ? WHERE id = ?")
        .bind(&new_key)
        .bind(creator_id)
        .execute(&state.pool)
        .await?;

    if let Some(session) = active_session {
        close_live_ingest_session(
            &state,
            &session,
            "terminated",
            "stream_key_rotated",
            json!({
                "reason": "creator rotated the stream key and invalidated the active encoder",
                "actorUserId": identity.user_id,
            }),
        )
        .await?;
    }

    publish_creator_live_state(&state, creator_id).await?;
    Ok(Json(contract_creator_profile(
        fetch_creator_profile(&state.pool, creator_id).await?,
    )))
}

pub(crate) async fn get_creator_live_ingest_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Option<LiveIngestSession>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_active_live_ingest_session(&state.pool, creator_id).await?,
    ))
}

pub(crate) async fn get_creator_live_ingest_session_by_id(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<AdminLiveIngestSessionRecord>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_creator_live_ingest_session_record(&state.pool, creator_id, &session_id).await?,
    ))
}

pub(crate) async fn list_creator_live_ingest_events(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<Vec<LiveIngestEvent>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    fetch_live_ingest_session_by_id(&state.pool, creator_id, &session_id).await?;
    Ok(Json(
        fetch_live_ingest_events_for_session(&state.pool, &session_id, 50).await?,
    ))
}

pub(crate) async fn reconcile_creator_live_ingest_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<LiveIngestReconciliationReport>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    fetch_live_ingest_session_by_id_unreconciled(&state.pool, creator_id, &session_id).await?;
    Ok(Json(
        reconcile_single_live_ingest_session(state, &session_id).await?,
    ))
}

pub(crate) async fn terminate_creator_live_ingest(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Json(input): Json<TerminateLiveIngestRequest>,
) -> AppResult<Json<LiveIngestSession>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-live-ingest-terminate:{}", identity.user_id),
        20,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    let session = fetch_live_ingest_session_by_id(&state.pool, creator_id, &session_id).await?;
    if session.status != "connected" {
        return Err(AppError::BadRequest(
            "only connected live ingest sessions can be terminated".to_string(),
        ));
    }

    close_live_ingest_session(
        &state,
        &session,
        "terminated",
        "creator_terminated",
        json!({
            "reason": input.reason.unwrap_or_else(|| "creator requested termination".to_string()),
            "actorUserId": identity.user_id,
        }),
    )
    .await?;

    publish_creator_live_state(&state, creator_id).await?;
    Ok(Json(
        fetch_live_ingest_session_by_id(&state.pool, creator_id, &session_id).await?,
    ))
}
