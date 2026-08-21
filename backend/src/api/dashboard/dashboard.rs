use super::analytics::{
    fetch_analytics, fetch_revenue_entries, fetch_top_content, fetch_traffic_sources,
    summarize_creator_analytics, summarize_creator_revenue,
};
use super::content::{
    fetch_broadcasts, fetch_creator_upload_operations_response, fetch_uploads,
    filter_creator_uploads, summarize_creator_content,
};
use super::*;

const CREATOR_DASHBOARD_ANALYTICS_LIMIT: usize = 14;
const CREATOR_DASHBOARD_REVENUE_LIMIT: usize = 14;
const CREATOR_DASHBOARD_RECENT_BROADCAST_LIMIT: usize = 12;
const CREATOR_DASHBOARD_NOTIFICATIONS_LIMIT: usize = 20;
const CREATOR_DASHBOARD_UPLOADS_LIMIT: usize = 20;
const CREATOR_APP_STATE_DASHBOARD_NOTIFICATIONS_LIMIT: usize = 10;
const CREATOR_APP_STATE_UPLOADS_LIMIT: usize = 20;
const CREATOR_APP_STATE_LIVE_HEALTH_SAMPLE_LIMIT: usize = 8;
const CREATOR_APP_STATE_SUBSCRIBER_TIER_LIMIT: usize = 6;
const CREATOR_APP_STATE_COLLABORATION_SESSION_LIMIT: usize = 2;
const CREATOR_APP_STATE_ACTIVE_RUNTIME_TARGET_LIMIT: usize = 8;

fn truncate_to_last<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        let drain_count = items.len() - limit;
        items.drain(0..drain_count);
    }
}

fn trim_creator_live_collaboration_summary_for_app_state(
    collaboration: &mut CreatorLiveCollaborationSummary,
) {
    collaboration.active_control = None;
    collaboration
        .recent_sessions
        .truncate(CREATOR_APP_STATE_COLLABORATION_SESSION_LIMIT);
}

fn trim_creator_live_control_for_app_state(response: &mut CreatorLiveControlResponse) {
    trim_creator_live_collaboration_summary_for_app_state(&mut response.collaboration);
    response
        .subscriber_tiers
        .truncate(CREATOR_APP_STATE_SUBSCRIBER_TIER_LIMIT);
    truncate_to_last(
        &mut response.health.samples,
        CREATOR_APP_STATE_LIVE_HEALTH_SAMPLE_LIMIT,
    );
    truncate_to_last(
        &mut response.viewer_history,
        CREATOR_APP_STATE_LIVE_HEALTH_SAMPLE_LIMIT,
    );
    truncate_to_last(
        &mut response.bitrate_history,
        CREATOR_APP_STATE_LIVE_HEALTH_SAMPLE_LIMIT,
    );
}

fn trim_creator_live_runtime_for_app_state(response: &mut CreatorLiveRuntimeResponse) {
    trim_creator_live_collaboration_summary_for_app_state(&mut response.collaboration);
    truncate_to_last(
        &mut response.health.samples,
        CREATOR_APP_STATE_LIVE_HEALTH_SAMPLE_LIMIT,
    );
    response
        .active_runtime_targets
        .truncate(CREATOR_APP_STATE_ACTIVE_RUNTIME_TARGET_LIMIT);
    response.recent_sessions.clear();
    response.recent_runtime_outputs.clear();
    response.recent_runtime_targets.clear();
    response.recent_telemetry.clear();
    response.recent_events.clear();
}

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
        .take(CREATOR_DASHBOARD_RECENT_BROADCAST_LIMIT)
        .collect();

    Ok(CreatorDashboard {
        profile: contract_creator_profile(profile),
        current_broadcast: current_broadcast.map(contract_broadcast),
        scheduled_broadcasts: contract_broadcasts(scheduled_broadcasts),
        recent_broadcasts: contract_broadcasts(recent_broadcasts),
        analytics: analytics
            .into_iter()
            .rev()
            .take(CREATOR_DASHBOARD_ANALYTICS_LIMIT)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect(),
        traffic_sources,
        top_content,
        revenue: revenue
            .into_iter()
            .take(CREATOR_DASHBOARD_REVENUE_LIMIT)
            .collect(),
        analytics_summary,
        revenue_summary,
        subscriber_tiers,
        operational_state,
        notifications: notifications
            .into_iter()
            .take(CREATOR_DASHBOARD_NOTIFICATIONS_LIMIT)
            .collect(),
        uploads: uploads
            .into_iter()
            .take(CREATOR_DASHBOARD_UPLOADS_LIMIT)
            .collect(),
    })
}

pub(crate) async fn fetch_creator_app_state(
    state: &SharedState,
    identity: &RequestIdentity,
    content_query: &CreatorContentQuery,
) -> AppResult<CreatorAppState> {
    let creator_id = identity.require_creator_scope()?;
    let pool = &state.pool;
    let mut dashboard = creator_dashboard_payload(pool, identity).await?;
    dashboard.uploads.clear();
    dashboard
        .notifications
        .truncate(CREATOR_APP_STATE_DASHBOARD_NOTIFICATIONS_LIMIT);
    let mut live_control =
        fetch_authoritative_creator_live_control_response(state, creator_id).await?;
    trim_creator_live_control_for_app_state(&mut live_control);
    let mut live_runtime =
        fetch_authoritative_creator_live_runtime_response(state, creator_id).await?;
    trim_creator_live_runtime_for_app_state(&mut live_runtime);
    let uploads = fetch_uploads(pool, creator_id).await?;
    let filtered_uploads = filter_creator_uploads(uploads.clone(), content_query)?;
    let content = CreatorContentResponse {
        summary: summarize_creator_content(&uploads, filtered_uploads.len() as i64),
        uploads: filtered_uploads
            .into_iter()
            .take(CREATOR_APP_STATE_UPLOADS_LIMIT)
            .collect(),
    };
    let mut upload_operations = fetch_creator_upload_operations_response(pool, creator_id).await?;
    upload_operations.records.clear();

    Ok(CreatorAppState {
        dashboard,
        live_control,
        live_runtime,
        content,
        upload_operations,
    })
}
