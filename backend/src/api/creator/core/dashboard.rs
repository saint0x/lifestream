use super::*;

pub(super) async fn creator_dashboard(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorDashboard>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_creator_scope()?;
    let payload = creator_dashboard_payload(&state.pool, &identity).await?;
    Ok(Json(payload))
}

pub(crate) async fn get_creator_state(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorAppState>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_creator_app_state(
            &state,
            &identity,
            &CreatorContentQuery {
                kind: None,
                status: None,
                q: None,
                sort: None,
            },
        )
        .await?,
    ))
}
