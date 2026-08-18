use super::*;

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
) -> AppResult<Json<ViewerAppState>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_viewer_app_state(&state.pool, &identity.user_id, &identity.session_id).await?,
    ))
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
