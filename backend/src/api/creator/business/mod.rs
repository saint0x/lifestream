use super::discovery::fetch_user;
use super::*;

mod credits;
mod marketplace;
mod metrics;
mod series;
mod subscriptions;
mod tiers;

#[cfg(test)]
pub(crate) use marketplace::{accept_ad_offer, get_ad_hub, submit_ad_offer_review};

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
            "/api/v1/creator/me/content/:content_kind/:content_id/credits",
            put(credits::replace_project_credits),
        )
        .route("/api/v1/creator/me/ad-hub", get(marketplace::get_ad_hub))
        .route(
            "/api/v1/creator/me/ad-offers/:offer_id/accept",
            post(marketplace::accept_ad_offer),
        )
        .route(
            "/api/v1/creator/me/ad-offers/:offer_id/decline",
            post(marketplace::decline_ad_offer),
        )
        .route(
            "/api/v1/creator/me/ad-offers/:offer_id/submissions",
            post(marketplace::submit_ad_offer_review),
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
