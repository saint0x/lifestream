use super::*;

pub(crate) async fn list_my_collaboration_mirror_grants(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<Vec<CollaborationMirrorGrant>>> {
    let identity = require_identity(&state.db, &headers).await?;
    reconcile_collaboration_session_expiry_for_read(&state, &session_id).await?;
    let participant = fetch_collaboration_participant_for_user(
        state.db.sqlite_adapter(),
        &session_id,
        &identity.user_id,
    )
    .await?;
    validate_collaboration_participant_access(&participant)?;
    Ok(Json(
        fetch_collaboration_mirror_grants_for_participant(
            state.db.sqlite_adapter(),
            &participant.id,
        )
        .await?,
    ))
}

pub(crate) async fn redeem_collaboration_mirror_grant(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(grant_id): Path<String>,
) -> AppResult<Json<CollaborationMirrorGrant>> {
    let identity = require_identity(&state.db, &headers).await?;
    Ok(Json(
        redeem_collaboration_mirror_grant_internal(&state, &identity, &grant_id).await?,
    ))
}
