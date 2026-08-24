use super::*;

pub(crate) async fn issue_collaboration_mirror_grant(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((session_id, participant_id)): Path<(String, String)>,
) -> AppResult<Json<CollaborationMirrorGrant>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    ensure_creator_collaboration_enabled(state.db.sqlite_adapter(), creator_id).await?;
    let session =
        fetch_collaboration_session_for_host(state.db.sqlite_adapter(), creator_id, &session_id)
            .await?;
    if session.status == "ended" {
        return Err(AppError::BadRequest(
            "cannot issue collaboration grants for an ended session".to_string(),
        ));
    }
    let participant =
        fetch_collaboration_participant_by_id(state.db.sqlite_adapter(), &participant_id).await?;
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
    let grant =
        issue_mirror_grant_for_participant(&state, &session, &participant, &identity.user_id)
            .await?;
    Ok(Json(grant))
}
