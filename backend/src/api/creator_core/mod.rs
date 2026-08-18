use super::moderation::{validate_creator_enforcement_scope, write_moderation_audit_entry};
use super::*;

mod dashboard;
mod enforcement;
mod operations;
mod summaries;

pub(crate) use dashboard::get_creator_state;
pub(crate) use enforcement::{
    get_admin_creator_enforcement_action, reconcile_admin_creator_enforcement_action,
};

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/api/v1/creator/me/dashboard",
            get(dashboard::creator_dashboard),
        )
        .route("/api/v1/creator/me/state", get(get_creator_state))
        .route(
            "/api/v1/creator/me/analytics/summary",
            get(summaries::get_creator_analytics_summary),
        )
        .route(
            "/api/v1/creator/me/revenue/summary",
            get(summaries::get_creator_revenue_summary),
        )
        .route(
            "/api/v1/creator/me/operations",
            get(operations::get_creator_operational_state)
                .patch(operations::update_creator_operational_state),
        )
        .route(
            "/api/v1/admin/creators/:creator_id/enforcement",
            get(enforcement::get_admin_creator_enforcement_state),
        )
        .route(
            "/api/v1/admin/creators/:creator_id/enforcement/actions",
            post(enforcement::create_admin_creator_enforcement_action),
        )
        .route(
            "/api/v1/admin/creators/:creator_id/enforcement/actions/:action_id",
            get(get_admin_creator_enforcement_action),
        )
        .route(
            "/api/v1/admin/creators/:creator_id/enforcement/actions/:action_id/reconcile",
            post(reconcile_admin_creator_enforcement_action),
        )
        .route(
            "/api/v1/admin/creators/:creator_id/enforcement/actions/:action_id/release",
            post(enforcement::release_admin_creator_enforcement_action),
        )
        .route(
            "/api/v1/creator/me/upload-operations",
            get(operations::get_creator_upload_operations),
        )
}
