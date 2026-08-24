use super::*;

pub(crate) async fn list_admin_playback_sessions(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<AdminPlaybackSessionQuery>,
) -> AppResult<Json<Vec<AdminPlaybackSessionRecord>>> {
    let identity = require_identity(&state.db, &headers).await?;
    identity.require_admin_scope()?;
    Ok(Json(
        fetch_admin_playback_sessions(
            &state.db,
            query.creator_id.as_deref(),
            query.content_id.as_deref(),
            query.state.as_deref(),
            query.limit.unwrap_or(100),
        )
        .await?,
    ))
}

pub(crate) async fn get_admin_playback_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<AdminPlaybackSessionRecord>> {
    let identity = require_identity(&state.db, &headers).await?;
    identity.require_admin_scope()?;
    Ok(Json(
        fetch_admin_playback_session_record(&state.db, &session_id).await?,
    ))
}

pub(crate) async fn reconcile_admin_playback_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<PlaybackReconciliationReport>> {
    let identity = require_identity(&state.db, &headers).await?;
    identity.require_admin_scope()?;
    ensure_admin_playback_session_exists(&state.db, &session_id).await?;
    Ok(Json(
        reconcile_single_playback_session(state, &session_id).await?,
    ))
}

pub(super) async fn revoke_admin_playback_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<AdminPlaybackSessionRecord>> {
    let identity = require_identity(&state.db, &headers).await?;
    identity.require_admin_scope()?;
    expire_admin_playback_session(&state.db, &session_id).await?;
    Ok(Json(
        fetch_admin_playback_session_record(&state.db, &session_id).await?,
    ))
}
