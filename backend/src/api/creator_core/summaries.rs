use super::*;

pub(super) async fn get_creator_analytics_summary(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorAnalyticsSummary>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let analytics = fetch_analytics(&state.pool, creator_id).await?;
    Ok(Json(summarize_creator_analytics(&analytics)))
}

pub(super) async fn get_creator_revenue_summary(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorRevenueSummary>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let analytics = fetch_analytics(&state.pool, creator_id).await?;
    let revenue = fetch_revenue_entries(&state.pool, creator_id).await?;
    let subscriber_tiers = fetch_creator_subscriber_tiers(&state.pool, creator_id).await?;
    Ok(Json(summarize_creator_revenue(
        &analytics,
        &revenue,
        &subscriber_tiers,
    )))
}
