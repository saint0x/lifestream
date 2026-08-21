use super::*;

const VIEWER_APP_STATE_RESPONSE_CACHE_TTL: Duration = Duration::from_millis(2_000);

pub(crate) async fn get_me(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(fetch_user(&state.pool, &identity.user_id).await?))
}

pub(crate) async fn get_my_state(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let identity = require_identity(&state.pool, &headers).await?;
    let cache_key = format!("viewer-state:session:{}", identity.session_id);
    if let Some(cached) = state
        .bootstrap_cache
        .get(&cache_key, VIEWER_APP_STATE_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok(([(header::CONTENT_TYPE, "application/json")], Body::from(cached)).into_response());
    }
    let _coalesced = state
        .request_coalescer
        .acquire(&cache_key)
        .await;
    if let Some(cached) = state
        .bootstrap_cache
        .get(&cache_key, VIEWER_APP_STATE_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok(([(header::CONTENT_TYPE, "application/json")], Body::from(cached)).into_response());
    }
    let response = fetch_viewer_app_state(&state.pool, &identity.user_id, &identity.session_id).await?;
    let response_body = Bytes::from(serde_json::to_vec(&response)?);
    state.bootstrap_cache.put(&cache_key, response_body.clone()).await;
    Ok(([(header::CONTENT_TYPE, "application/json")], Body::from(response_body)).into_response())
}

pub(crate) async fn get_my_library(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<UserLibrary>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_user_library(&state.pool, &identity.user_id).await?,
    ))
}

pub(crate) async fn get_my_watchlist(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<WatchlistResponse>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_watchlist_response(&state.pool, &identity.user_id).await?,
    ))
}

pub(crate) async fn get_my_entitlements(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<UserEntitlements>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_user_entitlements(&state.pool, &identity.user_id).await?,
    ))
}
