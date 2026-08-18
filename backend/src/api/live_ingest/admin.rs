use super::*;

pub(crate) async fn list_admin_live_ingest_sessions(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<AdminLiveIngestQuery>,
) -> AppResult<Json<Vec<AdminLiveIngestSessionRecord>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    Ok(Json(
        fetch_admin_live_ingest_sessions(
            &state.pool,
            query.creator_id.as_deref(),
            query.status.as_deref(),
            query.limit.unwrap_or(100),
        )
        .await?,
    ))
}

pub(crate) async fn get_admin_live_ingest_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<AdminLiveIngestSessionRecord>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    Ok(Json(
        fetch_admin_live_ingest_session_record(&state.pool, &session_id).await?,
    ))
}

pub(crate) async fn reconcile_admin_live_ingest_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<LiveIngestReconciliationReport>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    fetch_live_ingest_session_by_id_global_unreconciled(&state.pool, &session_id).await?;
    Ok(Json(
        reconcile_single_live_ingest_session(state, &session_id).await?,
    ))
}

pub(crate) async fn terminate_admin_live_ingest_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Json(input): Json<TerminateLiveIngestRequest>,
) -> AppResult<Json<AdminLiveIngestSessionRecord>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    let session = fetch_live_ingest_session_by_id_global(&state.pool, &session_id).await?;
    if session.status != "connected" {
        return Err(AppError::BadRequest(
            "only connected live ingest sessions can be terminated by operators".to_string(),
        ));
    }
    close_live_ingest_session(
        &state,
        &session,
        "terminated",
        "admin_terminated",
        json!({
            "reason": input.reason.unwrap_or_else(|| "operator requested termination".to_string()),
            "actorUserId": identity.user_id,
        }),
    )
    .await?;
    Ok(Json(
        fetch_admin_live_ingest_session_record(&state.pool, &session_id).await?,
    ))
}
