use super::analytics::{
    fetch_analytics, fetch_revenue_entries, fetch_top_content, fetch_traffic_sources,
    summarize_creator_analytics, summarize_creator_revenue,
};
use super::content::{
    fetch_broadcasts, fetch_creator_upload_operations_response, fetch_uploads,
    filter_creator_uploads, summarize_creator_content,
};
use super::*;

pub(crate) async fn creator_dashboard_payload(
    pool: &SqlitePool,
    identity: &RequestIdentity,
) -> AppResult<CreatorDashboard> {
    let creator_id = identity.require_creator_scope()?;
    let profile = fetch_creator_profile(pool, creator_id).await?;
    let operational_state = fetch_creator_operational_state(pool, &profile).await?;
    let broadcasts = fetch_broadcasts(pool, creator_id).await?;
    let analytics = fetch_analytics(pool, creator_id).await?;
    let analytics_summary = summarize_creator_analytics(&analytics);
    let traffic_sources = fetch_traffic_sources(pool, creator_id).await?;
    let top_content = fetch_top_content(pool, creator_id).await?;
    let revenue = fetch_revenue_entries(pool, creator_id).await?;
    let subscriber_tiers = fetch_creator_subscriber_tiers(pool, creator_id).await?;
    let revenue_summary = summarize_creator_revenue(&analytics, &revenue, &subscriber_tiers);
    let notifications = fetch_notifications_rows(pool, creator_id).await?;
    let uploads = fetch_uploads(pool, creator_id).await?;

    let current_broadcast = broadcasts
        .iter()
        .find(|item| item.status == "live")
        .cloned();
    let scheduled_broadcasts = broadcasts
        .iter()
        .filter(|item| item.status == "scheduled" || item.status == "ready")
        .cloned()
        .collect();
    let recent_broadcasts = broadcasts
        .iter()
        .filter(|item| item.status == "ended")
        .cloned()
        .collect();

    Ok(CreatorDashboard {
        profile: contract_creator_profile(profile),
        current_broadcast: current_broadcast.map(contract_broadcast),
        scheduled_broadcasts: contract_broadcasts(scheduled_broadcasts),
        recent_broadcasts: contract_broadcasts(recent_broadcasts),
        analytics,
        traffic_sources,
        top_content,
        revenue,
        analytics_summary,
        revenue_summary,
        subscriber_tiers,
        operational_state,
        notifications,
        uploads,
    })
}

pub(crate) async fn fetch_creator_app_state(
    state: &SharedState,
    identity: &RequestIdentity,
    content_query: &CreatorContentQuery,
) -> AppResult<CreatorAppState> {
    let creator_id = identity.require_creator_scope()?;
    let pool = &state.pool;
    let dashboard = creator_dashboard_payload(pool, identity).await?;
    let live_control = fetch_authoritative_creator_live_control_response(state, creator_id).await?;
    let live_runtime = fetch_authoritative_creator_live_runtime_response(state, creator_id).await?;
    let uploads = fetch_uploads(pool, creator_id).await?;
    let filtered_uploads = filter_creator_uploads(uploads.clone(), content_query)?;
    let content = CreatorContentResponse {
        summary: summarize_creator_content(&uploads, filtered_uploads.len() as i64),
        uploads: filtered_uploads,
    };
    let upload_operations = fetch_creator_upload_operations_response(pool, creator_id).await?;

    Ok(CreatorAppState {
        dashboard,
        live_control,
        live_runtime,
        content,
        upload_operations,
    })
}
