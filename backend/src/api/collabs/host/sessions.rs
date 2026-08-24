use super::*;

pub(crate) async fn list_creator_collaboration_sessions(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<CollaborationSession>>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    reconcile_collaboration_expiry_for_host_read(&state, creator_id).await?;
    Ok(Json(
        fetch_collaboration_sessions_for_host(state.db.sqlite_adapter(), creator_id).await?,
    ))
}

pub(crate) async fn create_collaboration_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<CreateCollaborationSessionRequest>,
) -> AppResult<Json<CollaborationSession>> {
    let identity = require_identity(&state.db, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-collab-session:{}", identity.user_id),
        20,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    ensure_creator_collaboration_enabled(state.db.sqlite_adapter(), creator_id).await?;
    let broadcast = resolve_collaboration_broadcast(
        state.db.sqlite_adapter(),
        creator_id,
        input.broadcast_id.as_deref(),
    )
    .await?;
    if let Some(existing) =
        fetch_active_collaboration_session_for_broadcast(state.db.sqlite_adapter(), &broadcast.id)
            .await?
    {
        return Err(AppError::BadRequest(format!(
            "a collaboration session is already active for broadcast {}",
            existing.id
        )));
    }
    let now = Utc::now().to_rfc3339();
    let session_id = format!("cols-{}", Uuid::new_v4().simple());
    let title = input
        .title
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("{} collaboration", broadcast.title));
    let chat_mode = input.chat_mode.unwrap_or_else(|| "shared".to_string());
    let recording_policy = input
        .recording_policy
        .unwrap_or_else(|| "host_archive".to_string());
    validate_collaboration_chat_mode(&chat_mode)?;
    validate_collaboration_recording_policy(&recording_policy)?;

    sqlx::query(
        r#"
        INSERT INTO collaboration_sessions (
            id, host_creator_id, source_broadcast_id, title, status, chat_mode,
            recording_policy, created_at, updated_at, activated_at, ended_at
        ) VALUES (?, ?, ?, ?, 'pending', ?, ?, ?, ?, NULL, NULL)
        "#,
    )
    .bind(&session_id)
    .bind(creator_id)
    .bind(&broadcast.id)
    .bind(&title)
    .bind(&chat_mode)
    .bind(&recording_policy)
    .bind(&now)
    .bind(&now)
    .execute(state.db.sqlite_adapter())
    .await?;

    let participant_id = format!("colp-{}", Uuid::new_v4().simple());
    sqlx::query(
        r#"
        INSERT INTO collaboration_participants (
            id, session_id, invite_id, user_id, creator_id, role, state, publish_to_host,
            mirror_to_guest_channel, can_speak_in_chat, joined_at, left_at, created_at, updated_at
        ) VALUES (?, ?, NULL, ?, ?, 'host', 'live', 1, 0, 1, ?, NULL, ?, ?)
        "#,
    )
    .bind(&participant_id)
    .bind(&session_id)
    .bind(&identity.user_id)
    .bind(Some(creator_id.to_string()))
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(state.db.sqlite_adapter())
    .await?;

    sqlx::query(
        "UPDATE collaboration_sessions SET status = 'active', activated_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(&session_id)
    .execute(state.db.sqlite_adapter())
    .await?;
    publish_collaboration_event(
        &state,
        &session_id,
        Some(identity.user_id.clone()),
        Some(participant_id),
        "session_created",
        json!({
            "hostCreatorId": creator_id,
            "sourceBroadcastId": broadcast.id,
            "title": title,
            "chatMode": chat_mode,
            "recordingPolicy": recording_policy,
        }),
    )
    .await?;

    Ok(Json(
        fetch_collaboration_session_for_host(state.db.sqlite_adapter(), creator_id, &session_id)
            .await?,
    ))
}

pub(crate) async fn get_creator_collaboration_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<CollaborationSession>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    reconcile_collaboration_session_expiry_for_read(&state, &session_id).await?;
    Ok(Json(
        fetch_collaboration_session_for_host(state.db.sqlite_adapter(), creator_id, &session_id)
            .await?,
    ))
}

pub(crate) async fn get_creator_collaboration_runtime(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<CollaborationRuntimeResponse>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    reconcile_collaboration_session_expiry_for_read(&state, &session_id).await?;
    let session =
        fetch_collaboration_session_for_host(state.db.sqlite_adapter(), creator_id, &session_id)
            .await?;
    Ok(Json(
        build_collaboration_runtime_response_for_host(state.db.sqlite_adapter(), session).await?,
    ))
}

pub(crate) async fn get_creator_collaboration_control(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<CreatorCollaborationControlResponse>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    reconcile_collaboration_session_expiry_for_read(&state, &session_id).await?;
    let session =
        fetch_collaboration_session_for_host(state.db.sqlite_adapter(), creator_id, &session_id)
            .await?;
    Ok(Json(
        build_creator_collaboration_control_response_for_host(state.db.sqlite_adapter(), session)
            .await?,
    ))
}

pub(crate) async fn get_creator_collaboration_socket_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((session_id, socket_id)): Path<(String, String)>,
) -> AppResult<Json<CollaborationSocketPresence>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let session =
        fetch_collaboration_session_for_host(state.db.sqlite_adapter(), creator_id, &session_id)
            .await?;
    let socket_session = fetch_collaboration_socket_presence_by_id_raw(
        state.db.sqlite_adapter(),
        &session.id,
        &socket_id,
    )
    .await?;
    Ok(Json(socket_session))
}

pub(crate) async fn reconcile_creator_collaboration_socket_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((session_id, socket_id)): Path<(String, String)>,
) -> AppResult<Json<CollaborationSocketPresenceReconciliationReport>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let session =
        fetch_collaboration_session_for_host(state.db.sqlite_adapter(), creator_id, &session_id)
            .await?;
    let socket_session = fetch_collaboration_socket_presence_by_id_raw(
        state.db.sqlite_adapter(),
        &session.id,
        &socket_id,
    )
    .await?;
    if socket_session.session_id != session.id {
        return Err(AppError::NotFound);
    }
    Ok(Json(
        reconcile_single_collaboration_socket_session(state, &session.id, &socket_id).await?,
    ))
}

pub(crate) async fn reconcile_creator_collaboration_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<CollaborationReconciliationReport>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    fetch_collaboration_session_for_host(state.db.sqlite_adapter(), creator_id, &session_id)
        .await?;
    Ok(Json(
        reconcile_single_collaboration_session(state, &session_id).await?,
    ))
}

pub(crate) async fn list_creator_collaboration_events(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Query(query): Query<CollaborationEventsQuery>,
) -> AppResult<Json<Vec<CollaborationEvent>>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    reconcile_collaboration_session_expiry_for_read(&state, &session_id).await?;
    fetch_collaboration_session_for_host(state.db.sqlite_adapter(), creator_id, &session_id)
        .await?;
    Ok(Json(
        fetch_collaboration_events(
            state.db.sqlite_adapter(),
            &session_id,
            query.after_seq.unwrap_or(0),
            query.limit.unwrap_or(100),
        )
        .await?,
    ))
}

pub(crate) async fn end_collaboration_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<CollaborationSession>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let session =
        fetch_collaboration_session_for_host(state.db.sqlite_adapter(), creator_id, &session_id)
            .await?;
    end_collaboration_session_internal(
        &state,
        &session,
        Some(identity.user_id.clone()),
        json!({
            "hostCreatorId": creator_id,
            "reason": "host ended the collaboration session",
        }),
    )
    .await?;

    Ok(Json(
        fetch_collaboration_session_for_host(state.db.sqlite_adapter(), creator_id, &session_id)
            .await?,
    ))
}
