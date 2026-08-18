use super::discovery::fetch_user;
use super::*;

mod metrics;
mod series;
mod subscriptions;
mod tiers;

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/api/v1/creator/me/subscriber-tiers",
            get(tiers::list_creator_subscriber_tiers).post(tiers::create_creator_subscriber_tier),
        )
        .route(
            "/api/v1/creator/me/subscriber-tiers/:tier_id",
            patch(tiers::update_creator_subscriber_tier),
        )
        .route(
            "/api/v1/creator/me/subscriber-tiers/:tier_id/retire",
            post(tiers::retire_creator_subscriber_tier),
        )
        .route(
            "/api/v1/creator/me/series",
            get(series::list_creator_series).post(series::create_creator_series),
        )
        .route(
            "/api/v1/creator/me/series/:id",
            patch(series::update_creator_series),
        )
        .route(
            "/api/v1/creator/subscriptions/:creator_id/tiers/:tier_id",
            post(subscriptions::subscribe_to_creator_tier),
        )
        .route(
            "/api/v1/creator/subscriptions/:creator_id",
            delete(subscriptions::cancel_creator_subscription),
        )
        .route("/api/v1/creator/me/analytics", get(metrics::list_analytics))
        .route("/api/v1/creator/me/revenue", get(metrics::list_revenue))
        .route(
            "/api/v1/creator/me/notifications",
            get(metrics::list_notifications),
        )
        .route(
            "/api/v1/creator/me/notifications/:notification_id/read",
            post(metrics::mark_creator_notification_read),
        )
}
