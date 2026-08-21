use super::*;

pub(super) async fn execute_revoke_invite(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    session: &CollaborationSessionView,
    invite_id: &str,
) -> AppResult<CollaborationSocketCommandOutcome> {
    let host_session = helpers::require_host_session(state, session_id, identity, session).await?;
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

pub(super) async fn execute_update_participant(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    session: &CollaborationSessionView,
    participant_id: String,
    update: UpdateCollaborationParticipantRequest,
) -> AppResult<CollaborationSocketCommandOutcome> {
    let host_session = helpers::require_host_session(state, session_id, identity, session).await?;
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

pub(super) async fn execute_remove_participant(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    session: &CollaborationSessionView,
    participant_id: &str,
) -> AppResult<CollaborationSocketCommandOutcome> {
    let host_session = helpers::require_host_session(state, session_id, identity, session).await?;
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
                "reason": "host_removed",
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
