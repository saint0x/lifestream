use super::*;

pub(crate) async fn execute_collaboration_socket_command(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    session: &CollaborationSessionView,
    command: CollaborationSocketCommand,
) -> AppResult<CollaborationSocketCommandOutcome> {
    match command {
        CollaborationSocketCommand::Heartbeat => Ok(CollaborationSocketCommandOutcome {
            command_type: "heartbeat",
            participant_id: None,
            state: None,
        }),
        CollaborationSocketCommand::RevokeInvite { invite_id } => {
            execute_revoke_invite(state, session_id, identity, session, &invite_id).await
        }
        CollaborationSocketCommand::RequestStateChange {
            state: requested_state,
        } => {
            execute_request_state_change(state, session_id, identity, session, requested_state)
                .await
        }
        CollaborationSocketCommand::UpdateParticipant {
            participant_id,
            state: requested_state,
            publish_to_host,
            mirror_to_guest_channel,
            can_speak_in_chat,
        } => {
            execute_update_participant(
                state,
                session_id,
                identity,
                session,
                participant_id,
                UpdateCollaborationParticipantRequest {
                    state: requested_state,
                    publish_to_host,
                    mirror_to_guest_channel,
                    can_speak_in_chat,
                },
            )
            .await
        }
        CollaborationSocketCommand::RemoveParticipant { participant_id } => {
            execute_remove_participant(state, session_id, identity, session, &participant_id).await
        }
        CollaborationSocketCommand::IssueMirrorGrant { participant_id } => {
            execute_issue_mirror_grant(state, session_id, identity, session, &participant_id).await
        }
        CollaborationSocketCommand::RevokeMirrorGrants { participant_id } => {
            execute_revoke_mirror_grants(state, session_id, identity, session, &participant_id)
                .await
        }
    }
}

async fn execute_revoke_invite(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    session: &CollaborationSessionView,
    invite_id: &str,
) -> AppResult<CollaborationSocketCommandOutcome> {
    let host_session = require_host_session(state, session_id, identity, session).await?;
    let invite = revoke_collaboration_invite_internal(
        state,
        &host_session,
        invite_id,
        &identity.user_id,
        "host_revoked",
    )
    .await?;
    Ok(CollaborationSocketCommandOutcome {
        command_type: "revokeInvite",
        participant_id: None,
        state: Some(invite.state),
    })
}

async fn execute_request_state_change(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    session: &CollaborationSessionView,
    requested_state: String,
) -> AppResult<CollaborationSocketCommandOutcome> {
    if session.participant.role == "host" {
        return Err(AppError::BadRequest(
            "host participants cannot request collaboration state changes".to_string(),
        ));
    }
    let current_participant =
        fetch_collaboration_participant_by_id(&state.pool, &session.participant.id).await?;
    validate_collaboration_participant_access(&current_participant)?;
    let current_session = fetch_collaboration_session_by_id(&state.pool, session_id).await?;
    if current_session.status == "ended" {
        return Err(AppError::BadRequest(
            "cannot request collaboration state changes for an ended session".to_string(),
        ));
    }
    if requested_state != "backstage" && requested_state != "live" {
        return Err(AppError::BadRequest(
            "requested collaboration state must be backstage or live".to_string(),
        ));
    }
    if current_participant.state == requested_state {
        return Err(AppError::BadRequest(
            "participant is already in the requested collaboration state".to_string(),
        ));
    }
    let requested_at = Utc::now().to_rfc3339();
    publish_collaboration_event(
        state,
        session_id,
        Some(identity.user_id.clone()),
        Some(current_participant.id.clone()),
        "participant_state_requested",
        json!({
            "participantId": current_participant.id,
            "currentState": current_participant.state,
            "requestedState": requested_state,
            "requestedAt": requested_at,
        }),
    )
    .await?;
    Ok(CollaborationSocketCommandOutcome {
        command_type: "requestStateChange",
        participant_id: Some(current_participant.id),
        state: Some(requested_state),
    })
}

async fn execute_update_participant(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    session: &CollaborationSessionView,
    participant_id: String,
    update: UpdateCollaborationParticipantRequest,
) -> AppResult<CollaborationSocketCommandOutcome> {
    let host_session = require_host_session(state, session_id, identity, session).await?;
    let updated = apply_collaboration_participant_update(
        state,
        &host_session,
        &participant_id,
        &identity.user_id,
        &update,
    )
    .await?;
    Ok(CollaborationSocketCommandOutcome {
        command_type: "updateParticipant",
        participant_id: Some(updated.id),
        state: Some(updated.state),
    })
}

async fn execute_remove_participant(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    session: &CollaborationSessionView,
    participant_id: &str,
) -> AppResult<CollaborationSocketCommandOutcome> {
    let host_session = require_host_session(state, session_id, identity, session).await?;
    if host_session.status == "ended" {
        return Err(AppError::BadRequest(
            "cannot remove participants from an ended collaboration session".to_string(),
        ));
    }
    let participant = fetch_collaboration_participant_by_id(&state.pool, participant_id).await?;
    if participant.session_id != *session_id {
        return Err(AppError::NotFound);
    }
    if participant.role == "host" {
        return Err(AppError::BadRequest(
            "the host cannot be removed from a collaboration session".to_string(),
        ));
    }
    if participant.state != "removed" {
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
        .bind(participant_id)
        .bind(session_id)
        .execute(&state.pool)
        .await?;
        revoke_collaboration_mirror_grants_for_participant(
            state,
            session_id,
            participant_id,
            Some(identity.user_id.clone()),
            &now,
            "participant_removed",
        )
        .await?;
        publish_collaboration_event(
            state,
            session_id,
            Some(identity.user_id.clone()),
            Some(participant_id.to_string()),
            "participant_removed",
            json!({
                "participantId": participant_id,
                "removedAt": now,
            }),
        )
        .await?;
    }
    let updated = fetch_collaboration_participant_by_id(&state.pool, participant_id).await?;
    Ok(CollaborationSocketCommandOutcome {
        command_type: "removeParticipant",
        participant_id: Some(updated.id),
        state: Some(updated.state),
    })
}

async fn execute_issue_mirror_grant(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    session: &CollaborationSessionView,
    participant_id: &str,
) -> AppResult<CollaborationSocketCommandOutcome> {
    let creator_id = require_creator_identity(identity)?;
    require_host_role(session)?;
    ensure_creator_collaboration_enabled(&state.pool, creator_id).await?;
    let host_session =
        fetch_collaboration_session_for_host(&state.pool, creator_id, session_id).await?;
    if host_session.status == "ended" {
        return Err(AppError::BadRequest(
            "cannot issue collaboration grants for an ended session".to_string(),
        ));
    }
    let participant = fetch_collaboration_participant_by_id(&state.pool, participant_id).await?;
    if participant.session_id != *session_id {
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
    let grant =
        issue_mirror_grant_for_participant(state, &host_session, &participant, &identity.user_id)
            .await?;
    Ok(CollaborationSocketCommandOutcome {
        command_type: "issueMirrorGrant",
        participant_id: Some(grant.participant_id),
        state: None,
    })
}

async fn execute_revoke_mirror_grants(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    session: &CollaborationSessionView,
    participant_id: &str,
) -> AppResult<CollaborationSocketCommandOutcome> {
    let host_session = require_host_session(state, session_id, identity, session).await?;
    let participant = fetch_collaboration_participant_by_id(&state.pool, participant_id).await?;
    if participant.session_id != host_session.id {
        return Err(AppError::NotFound);
    }
    let now = Utc::now().to_rfc3339();
    revoke_collaboration_mirror_grants_for_participant(
        state,
        session_id,
        participant_id,
        Some(identity.user_id.clone()),
        &now,
        "host_revoked",
    )
    .await?;
    Ok(CollaborationSocketCommandOutcome {
        command_type: "revokeMirrorGrants",
        participant_id: Some(participant_id.to_string()),
        state: None,
    })
}

fn require_host_role(session: &CollaborationSessionView) -> AppResult<()> {
    if session.participant.role != "host" {
        return Err(AppError::BadRequest(
            "only the collaboration host can perform this realtime control action".to_string(),
        ));
    }
    Ok(())
}

fn require_creator_identity(identity: &RequestIdentity) -> AppResult<&str> {
    identity.creator_id.as_deref().ok_or_else(|| {
        AppError::BadRequest(
            "creator scope is required for host collaboration controls".to_string(),
        )
    })
}

async fn require_host_session(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    session: &CollaborationSessionView,
) -> AppResult<CollaborationSession> {
    require_host_role(session)?;
    let creator_id = require_creator_identity(identity)?;
    fetch_collaboration_session_for_host(&state.pool, creator_id, session_id).await
}
