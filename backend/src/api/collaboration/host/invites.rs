use super::*;

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
