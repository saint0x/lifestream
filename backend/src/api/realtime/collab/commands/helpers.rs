use super::*;

pub(super) fn require_host_role(session: &CollaborationSessionView) -> AppResult<()> {
    if session.participant.role != "host" {
        return Err(AppError::BadRequest(
            "only the collaboration host can perform this realtime control action".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn require_creator_identity(identity: &RequestIdentity) -> AppResult<&str> {
    identity.creator_id.as_deref().ok_or_else(|| {
        AppError::BadRequest(
            "creator scope is required for host collaboration controls".to_string(),
        )
    })
}

pub(super) async fn require_host_session(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    session: &CollaborationSessionView,
) -> AppResult<CollaborationSession> {
    require_host_role(session)?;
    let creator_id = require_creator_identity(identity)?;
    fetch_collaboration_session_for_host(state.db.sqlite_adapter(), creator_id, session_id).await
}
