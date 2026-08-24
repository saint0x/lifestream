use super::*;

const CREATOR_APP_STATE_RESPONSE_CACHE_TTL: Duration = Duration::from_millis(2_000);

pub(super) async fn creator_dashboard(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorDashboard>> {
    let identity = require_identity(&state.db, &headers).await?;
    identity.require_creator_scope()?;
    let payload = creator_dashboard_payload(state.db.sqlite_adapter(), &identity).await?;
    Ok(Json(payload))
}

pub(crate) async fn get_creator_state(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let identity = require_identity(&state.db, &headers).await?;
    let cache_key = format!("creator-state:session:{}", identity.session_id);
    if let Some(cached) = state
        .bootstrap_cache
        .get(&cache_key, CREATOR_APP_STATE_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok((
            [(header::CONTENT_TYPE, "application/json")],
            Body::from(cached),
        )
            .into_response());
    }
    let _coalesced = state.request_coalescer.acquire(&cache_key).await;
    if let Some(cached) = state
        .bootstrap_cache
        .get(&cache_key, CREATOR_APP_STATE_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok((
            [(header::CONTENT_TYPE, "application/json")],
            Body::from(cached),
        )
            .into_response());
    }
    let response = fetch_creator_app_state(
        &state,
        &identity,
        &CreatorContentQuery {
            kind: None,
            status: None,
            q: None,
            sort: None,
        },
    )
    .await?;
    let response_body = Bytes::from(serde_json::to_vec(&response)?);
    state
        .bootstrap_cache
        .put(&cache_key, response_body.clone())
        .await;
    Ok((
        [(header::CONTENT_TYPE, "application/json")],
        Body::from(response_body),
    )
        .into_response())
}
