use super::*;

pub(super) async fn execute_request_state_change(
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
    let current_participant = fetch_collaboration_participant_by_id(
        state.db.try_sqlite_adapter()?,
        &session.participant.id,
    )
    .await?;
    validate_collaboration_participant_access(&current_participant)?;
    let current_session =
        fetch_collaboration_session_by_id(state.db.try_sqlite_adapter()?, session_id).await?;
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
