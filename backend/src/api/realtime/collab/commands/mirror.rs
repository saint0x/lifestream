use super::*;

pub(super) async fn execute_issue_mirror_grant(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    session: &CollaborationSessionView,
    participant_id: &str,
) -> AppResult<CollaborationSocketCommandOutcome> {
    let creator_id = helpers::require_creator_identity(identity)?;
    helpers::require_host_role(session)?;
    ensure_creator_collaboration_enabled(state.db.sqlite_adapter(), creator_id).await?;
    let host_session =
        fetch_collaboration_session_for_host(state.db.sqlite_adapter(), creator_id, session_id)
            .await?;
    if host_session.status == "ended" {
        return Err(AppError::BadRequest(
            "cannot issue collaboration grants for an ended session".to_string(),
        ));
    }
    let participant =
        fetch_collaboration_participant_by_id(state.db.sqlite_adapter(), participant_id).await?;
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

pub(super) async fn execute_revoke_mirror_grants(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    session: &CollaborationSessionView,
    participant_id: &str,
) -> AppResult<CollaborationSocketCommandOutcome> {
    let host_session = helpers::require_host_session(state, session_id, identity, session).await?;
    let participant =
        fetch_collaboration_participant_by_id(state.db.sqlite_adapter(), participant_id).await?;
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
