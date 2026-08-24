use super::*;

pub(crate) async fn list_my_collaboration_invites(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<CollaborationInvite>>> {
    let identity = require_identity(&state.db, &headers).await?;
    reconcile_collaboration_expiry_for_participant_read(&state, &identity.user_id).await?;
    Ok(Json(
        fetch_collaboration_invites_for_user(state.db.sqlite_adapter(), &identity.user_id).await?,
    ))
}

pub(crate) async fn list_my_collaboration_sessions(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<CollaborationSessionView>>> {
    let identity = require_identity(&state.db, &headers).await?;
    reconcile_collaboration_expiry_for_participant_read(&state, &identity.user_id).await?;
    Ok(Json(
        fetch_collaboration_sessions_for_participant(state.db.sqlite_adapter(), &identity.user_id)
            .await?,
    ))
}

pub(crate) async fn list_my_collaboration_events(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Query(query): Query<CollaborationEventsQuery>,
) -> AppResult<Json<Vec<CollaborationEvent>>> {
    let identity = require_identity(&state.db, &headers).await?;
    reconcile_collaboration_session_expiry_for_read(&state, &session_id).await?;
    let session = fetch_collaboration_session_for_participant(
        state.db.sqlite_adapter(),
        &identity.user_id,
        &session_id,
    )
    .await?;
    Ok(Json(filter_visible_collaboration_events_for_session(
        &session,
        fetch_collaboration_events(
            state.db.sqlite_adapter(),
            &session_id,
            query.after_seq.unwrap_or(0),
            query.limit.unwrap_or(100),
        )
        .await?,
    )))
}

pub(crate) async fn get_my_collaboration_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<CollaborationSessionView>> {
    let identity = require_identity(&state.db, &headers).await?;
    reconcile_collaboration_session_expiry_for_read(&state, &session_id).await?;
    Ok(Json(
        fetch_collaboration_session_for_participant(
            state.db.sqlite_adapter(),
            &identity.user_id,
            &session_id,
        )
        .await?,
    ))
}

pub(crate) async fn get_my_collaboration_runtime(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<CollaborationRuntimeResponse>> {
    let identity = require_identity(&state.db, &headers).await?;
    reconcile_collaboration_session_expiry_for_read(&state, &session_id).await?;
    let session = fetch_collaboration_session_for_participant(
        state.db.sqlite_adapter(),
        &identity.user_id,
        &session_id,
    )
    .await?;
    Ok(Json(
        build_collaboration_runtime_response_for_participant(state.db.sqlite_adapter(), session)
            .await?,
    ))
}
