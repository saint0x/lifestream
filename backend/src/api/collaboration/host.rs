use super::super::discovery::{fetch_creator_id_for_user, fetch_user};
use super::super::realtime::{
    reconcile_collaboration_expiry_for_host_read, reconcile_collaboration_session_expiry_for_read,
};
use super::*;

pub(crate) async fn list_creator_collaboration_sessions(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<CollaborationSession>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    reconcile_collaboration_expiry_for_host_read(&state, creator_id).await?;
    Ok(Json(
        fetch_collaboration_sessions_for_host(&state.pool, creator_id).await?,
    ))
}

pub(crate) async fn create_collaboration_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<CreateCollaborationSessionRequest>,
) -> AppResult<Json<CollaborationSession>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-collab-session:{}", identity.user_id),
        20,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    ensure_creator_collaboration_enabled(&state.pool, creator_id).await?;
    let broadcast =
        resolve_collaboration_broadcast(&state.pool, creator_id, input.broadcast_id.as_deref())
            .await?;
    if let Some(existing) =
        fetch_active_collaboration_session_for_broadcast(&state.pool, &broadcast.id).await?
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
    .execute(&state.pool)
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
    .execute(&state.pool)
    .await?;

    sqlx::query(
        "UPDATE collaboration_sessions SET status = 'active', activated_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(&session_id)
    .execute(&state.pool)
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
        fetch_collaboration_session_for_host(&state.pool, creator_id, &session_id).await?,
    ))
}

pub(crate) async fn get_creator_collaboration_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<CollaborationSession>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    reconcile_collaboration_session_expiry_for_read(&state, &session_id).await?;
    Ok(Json(
        fetch_collaboration_session_for_host(&state.pool, creator_id, &session_id).await?,
    ))
}

pub(crate) async fn get_creator_collaboration_runtime(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<CollaborationRuntimeResponse>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    reconcile_collaboration_session_expiry_for_read(&state, &session_id).await?;
    let session =
        fetch_collaboration_session_for_host(&state.pool, creator_id, &session_id).await?;
    Ok(Json(
        build_collaboration_runtime_response_for_host(&state.pool, session).await?,
    ))
}

pub(crate) async fn get_creator_collaboration_control(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<CreatorCollaborationControlResponse>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    reconcile_collaboration_session_expiry_for_read(&state, &session_id).await?;
    let session =
        fetch_collaboration_session_for_host(&state.pool, creator_id, &session_id).await?;
    Ok(Json(
        build_creator_collaboration_control_response_for_host(&state.pool, session).await?,
    ))
}

pub(crate) async fn get_creator_collaboration_socket_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((session_id, socket_id)): Path<(String, String)>,
) -> AppResult<Json<CollaborationSocketPresence>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let session =
        fetch_collaboration_session_for_host(&state.pool, creator_id, &session_id).await?;
    let socket_session =
        fetch_collaboration_socket_presence_by_id_raw(&state.pool, &session.id, &socket_id).await?;
    Ok(Json(socket_session))
}

pub(crate) async fn reconcile_creator_collaboration_socket_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((session_id, socket_id)): Path<(String, String)>,
) -> AppResult<Json<CollaborationSocketPresenceReconciliationReport>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let session =
        fetch_collaboration_session_for_host(&state.pool, creator_id, &session_id).await?;
    let socket_session =
        fetch_collaboration_socket_presence_by_id_raw(&state.pool, &session.id, &socket_id).await?;
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
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    fetch_collaboration_session_for_host(&state.pool, creator_id, &session_id).await?;
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
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    reconcile_collaboration_session_expiry_for_read(&state, &session_id).await?;
    fetch_collaboration_session_for_host(&state.pool, creator_id, &session_id).await?;
    Ok(Json(
        fetch_collaboration_events(
            &state.pool,
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
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let session =
        fetch_collaboration_session_for_host(&state.pool, creator_id, &session_id).await?;
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
        fetch_collaboration_session_for_host(&state.pool, creator_id, &session_id).await?,
    ))
}

pub(crate) async fn create_collaboration_invite(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Json(input): Json<CreateCollaborationInviteRequest>,
) -> AppResult<Json<CollaborationInvite>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-collab-invite:{}", identity.user_id),
        30,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    ensure_creator_collaboration_enabled(&state.pool, creator_id).await?;
    let session =
        fetch_collaboration_session_for_host(&state.pool, creator_id, &session_id).await?;
    if session.status == "ended" {
        return Err(AppError::BadRequest(
            "cannot invite participants into an ended collaboration session".to_string(),
        ));
    }
    let invitee = fetch_user(&state.pool, &input.invitee_user_id).await?;
    let invitee_creator_id = fetch_creator_id_for_user(&state.pool, &invitee.id).await?;
    if input.mirror_to_guest_channel && invitee_creator_id.is_none() {
        return Err(AppError::BadRequest(
            "mirror-to-channel collaboration requires the invited user to have a creator profile"
                .to_string(),
        ));
    }
    if let Ok(existing_participant) =
        fetch_collaboration_participant_for_user(&state.pool, &session_id, &invitee.id).await
    {
        if existing_participant.state != "left" && existing_participant.state != "removed" {
            return Err(AppError::BadRequest(
                "user is already participating in this collaboration session".to_string(),
            ));
        }
    }
    if has_pending_collaboration_invite_for_user(&state.pool, &session_id, &invitee.id).await? {
        return Err(AppError::BadRequest(
            "user already has a pending collaboration invite for this session".to_string(),
        ));
    }
    validate_collaboration_role(&input.role)?;
    let now = Utc::now();
    let expires_at = (now
        + chrono::Duration::minutes(input.expires_in_minutes.unwrap_or(30).clamp(5, 24 * 60)))
    .to_rfc3339();
    let invite_id = format!("coli-{}", Uuid::new_v4().simple());

    sqlx::query(
        r#"
        INSERT INTO collaboration_invites (
            id, session_id, host_creator_id, invitee_user_id, invitee_creator_id, role, state,
            mirror_to_guest_channel, message, created_at, responded_at, expires_at
        ) VALUES (?, ?, ?, ?, ?, ?, 'pending', ?, ?, ?, NULL, ?)
        "#,
    )
    .bind(&invite_id)
    .bind(&session_id)
    .bind(creator_id)
    .bind(&invitee.id)
    .bind(&invitee_creator_id)
    .bind(&input.role)
    .bind(input.mirror_to_guest_channel as i64)
    .bind(input.message)
    .bind(now.to_rfc3339())
    .bind(&expires_at)
    .execute(&state.pool)
    .await?;
    publish_collaboration_event(
        &state,
        &session_id,
        Some(identity.user_id.clone()),
        None,
        "invite_created",
        json!({
            "inviteId": invite_id,
            "inviteeUserId": invitee.id,
            "inviteeCreatorId": invitee_creator_id,
            "role": input.role,
            "mirrorToGuestChannel": input.mirror_to_guest_channel,
            "expiresAt": expires_at,
        }),
    )
    .await?;
    enqueue_notification_event(
        &state.pool,
        "collaboration_invite",
        &format!(
            "{} invited you to join a collaboration session.",
            session.title
        ),
        Some(&identity.user_id),
        Some(&session.title),
        Some(creator_id),
        None,
        None,
        json!({
            "inviteId": invite_id,
            "sessionId": session_id,
            "role": input.role,
            "mirrorToGuestChannel": input.mirror_to_guest_channel,
        }),
        &[invitee.id],
        &[],
    )
    .await?;

    Ok(Json(
        fetch_collaboration_invite_by_id(&state.pool, &invite_id).await?,
    ))
}

pub(crate) async fn revoke_collaboration_invite(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((session_id, invite_id)): Path<(String, String)>,
) -> AppResult<Json<CollaborationInvite>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let session =
        fetch_collaboration_session_for_host(&state.pool, creator_id, &session_id).await?;
    Ok(Json(
        revoke_collaboration_invite_internal(
            &state,
            &session,
            &invite_id,
            &identity.user_id,
            "host_revoked",
        )
        .await?,
    ))
}

pub(crate) async fn revoke_collaboration_invite_internal(
    state: &SharedState,
    session: &CollaborationSession,
    invite_id: &str,
    actor_user_id: &str,
    reason: &str,
) -> AppResult<CollaborationInvite> {
    if session.status == "ended" {
        return Err(AppError::BadRequest(
            "cannot revoke invites from an ended collaboration session".to_string(),
        ));
    }
    let invite = fetch_collaboration_invite_by_id(&state.pool, invite_id).await?;
    if invite.session_id != session.id || invite.host_creator_id != session.host_creator_id {
        return Err(AppError::NotFound);
    }
    if invite.state == "revoked" {
        return Ok(invite);
    }
    validate_pending_collaboration_invite(&invite)?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE collaboration_invites SET state = 'revoked', responded_at = COALESCE(responded_at, ?) WHERE id = ?",
    )
    .bind(&now)
    .bind(invite_id)
    .execute(&state.pool)
    .await?;
    publish_collaboration_invite_revoked_events(
        state,
        &session.id,
        Some(actor_user_id.to_string()),
        std::slice::from_ref(&invite),
        &now,
        reason,
    )
    .await?;
    fetch_collaboration_invite_by_id(&state.pool, invite_id).await
}

pub(crate) async fn update_collaboration_participant(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((session_id, participant_id)): Path<(String, String)>,
    Json(input): Json<UpdateCollaborationParticipantRequest>,
) -> AppResult<Json<CollaborationParticipant>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    ensure_creator_collaboration_enabled(&state.pool, creator_id).await?;
    let session =
        fetch_collaboration_session_for_host(&state.pool, creator_id, &session_id).await?;
    Ok(Json(
        apply_collaboration_participant_update(
            &state,
            &session,
            &participant_id,
            &identity.user_id,
            &input,
        )
        .await?,
    ))
}

pub(crate) async fn apply_collaboration_participant_update(
    state: &SharedState,
    session: &CollaborationSession,
    participant_id: &str,
    actor_user_id: &str,
    input: &UpdateCollaborationParticipantRequest,
) -> AppResult<CollaborationParticipant> {
    if session.status == "ended" {
        return Err(AppError::BadRequest(
            "cannot update participants for an ended collaboration session".to_string(),
        ));
    }

    let participant = fetch_collaboration_participant_by_id(&state.pool, participant_id).await?;
    if participant.session_id != session.id {
        return Err(AppError::NotFound);
    }
    if participant.role == "host" {
        return Err(AppError::BadRequest(
            "the host participant cannot be mutated through collaboration controls".to_string(),
        ));
    }

    let next_state = match input.state.as_deref() {
        Some(state_value) => {
            validate_collaboration_participant_state(state_value)?;
            validate_collaboration_participant_transition(&participant.state, state_value, true)?;
            state_value.to_string()
        }
        None => participant.state.clone(),
    };
    let publish_to_host = input.publish_to_host.unwrap_or(participant.publish_to_host);
    let mirror_to_guest_channel = input
        .mirror_to_guest_channel
        .unwrap_or(participant.mirror_to_guest_channel);
    let can_speak_in_chat = input
        .can_speak_in_chat
        .unwrap_or(participant.can_speak_in_chat);

    if mirror_to_guest_channel && participant.creator_id.is_none() {
        return Err(AppError::BadRequest(
            "participant must have a creator profile to mirror to their guest channel".to_string(),
        ));
    }

    let now = Utc::now().to_rfc3339();
    let left_at = if matches!(next_state.as_str(), "left" | "removed") {
        Some(participant.left_at.clone().unwrap_or_else(|| now.clone()))
    } else {
        None
    };
    sqlx::query(
        r#"
        UPDATE collaboration_participants
        SET state = ?, publish_to_host = ?, mirror_to_guest_channel = ?, can_speak_in_chat = ?,
            left_at = ?, updated_at = ?
        WHERE id = ? AND session_id = ?
        "#,
    )
    .bind(&next_state)
    .bind(publish_to_host as i64)
    .bind(mirror_to_guest_channel as i64)
    .bind(can_speak_in_chat as i64)
    .bind(left_at)
    .bind(&now)
    .bind(participant_id)
    .bind(&session.id)
    .execute(&state.pool)
    .await?;

    if !matches!(next_state.as_str(), "live") || !mirror_to_guest_channel {
        revoke_collaboration_mirror_grants_for_participant(
            state,
            &session.id,
            participant_id,
            Some(actor_user_id.to_string()),
            &now,
            "participant_updated",
        )
        .await?;
    }

    publish_collaboration_event(
        state,
        &session.id,
        Some(actor_user_id.to_string()),
        Some(participant_id.to_string()),
        "participant_updated",
        json!({
            "participantId": participant_id,
            "previousState": participant.state,
            "state": next_state,
            "publishToHost": publish_to_host,
            "mirrorToGuestChannel": mirror_to_guest_channel,
            "canSpeakInChat": can_speak_in_chat,
            "updatedAt": now,
        }),
    )
    .await?;

    fetch_collaboration_participant_by_id(&state.pool, participant_id).await
}

pub(crate) async fn remove_collaboration_participant(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((session_id, participant_id)): Path<(String, String)>,
) -> AppResult<Json<CollaborationParticipant>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let session =
        fetch_collaboration_session_for_host(&state.pool, creator_id, &session_id).await?;
    if session.status == "ended" {
        return Err(AppError::BadRequest(
            "cannot remove participants from an ended collaboration session".to_string(),
        ));
    }
    let participant = fetch_collaboration_participant_by_id(&state.pool, &participant_id).await?;
    if participant.session_id != session_id {
        return Err(AppError::NotFound);
    }
    if participant.role == "host" {
        return Err(AppError::BadRequest(
            "the host cannot be removed from a collaboration session".to_string(),
        ));
    }
    if participant.state == "removed" {
        return Ok(Json(participant));
    }
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        UPDATE collaboration_participants
        SET state = 'removed', left_at = COALESCE(left_at, ?), updated_at = ?
        WHERE id = ? AND session_id = ?
        "#,
    )
    .bind(&now)
    .bind(&now)
    .bind(&participant_id)
    .bind(&session_id)
    .execute(&state.pool)
    .await?;
    revoke_collaboration_mirror_grants_for_participant(
        &state,
        &session_id,
        &participant_id,
        Some(identity.user_id.clone()),
        &now,
        "participant_removed",
    )
    .await?;
    publish_collaboration_event(
        &state,
        &session_id,
        Some(identity.user_id.clone()),
        Some(participant_id.clone()),
        "participant_removed",
        json!({
            "participantId": participant_id,
            "removedAt": now,
        }),
    )
    .await?;
    Ok(Json(
        fetch_collaboration_participant_by_id(&state.pool, &participant_id).await?,
    ))
}

pub(crate) async fn issue_collaboration_mirror_grant(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((session_id, participant_id)): Path<(String, String)>,
) -> AppResult<Json<CollaborationMirrorGrant>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    ensure_creator_collaboration_enabled(&state.pool, creator_id).await?;
    let session =
        fetch_collaboration_session_for_host(&state.pool, creator_id, &session_id).await?;
    if session.status == "ended" {
        return Err(AppError::BadRequest(
            "cannot issue collaboration grants for an ended session".to_string(),
        ));
    }
    let participant = fetch_collaboration_participant_by_id(&state.pool, &participant_id).await?;
    if participant.session_id != session_id {
        return Err(AppError::NotFound);
    }
    if participant.state != "live" {
        return Err(AppError::BadRequest(
            "mirror grants can only be issued for live participants".to_string(),
        ));
    }
    if !participant.mirror_to_guest_channel {
        return Err(AppError::BadRequest(
            "participant is not enabled for mirrored guest channel pickup".to_string(),
        ));
    }
    if participant.creator_id.is_none() {
        return Err(AppError::BadRequest(
            "participant must have a creator profile to receive a mirror grant".to_string(),
        ));
    }
    Ok(Json(
        issue_mirror_grant_for_participant(&state, &session, &participant, &identity.user_id)
            .await?,
    ))
}
