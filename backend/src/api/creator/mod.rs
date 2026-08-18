pub(super) use super::*;
pub(super) use super::{discovery, ingest, moderation};

mod business;
mod catalog;
mod commerce;
mod core;
mod data;
mod live;

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .merge(business::routes())
        .merge(core::routes())
        .merge(live::routes())
}

pub(crate) use catalog::{
    fetch_creator_catalog_film_by_id, fetch_creator_catalog_film_by_slug,
    fetch_creator_catalog_films, fetch_creator_catalog_series, fetch_creator_catalog_series_by_id,
    fetch_creator_catalog_series_by_slug, fetch_creator_series, fetch_creator_series_by_id,
};
pub(crate) use commerce::{
    fetch_content_purchase_by_id, fetch_creator_membership, fetch_current_content_purchase,
    fetch_user_entitlements, purchase_belongs_to_user, reconcile_single_membership_entitlement,
    reconcile_single_purchase_entitlement,
};
#[cfg(test)]
pub(crate) use core::{
    get_admin_creator_enforcement_action, get_creator_state,
    reconcile_admin_creator_enforcement_action,
};
pub(crate) use data::{
    fetch_creator_enforcement_action_by_id, fetch_creator_enforcement_action_by_id_raw,
    fetch_creator_enforcement_state, fetch_creator_live_health, fetch_creator_live_settings,
    fetch_creator_operational_state, fetch_creator_profile, fetch_creator_profile_by_stream_key,
    fetch_creator_subscriber_tier_by_id, fetch_creator_subscriber_tiers,
    next_creator_subscriber_tier_rank, normalize_creator_subscriber_tier_ranks,
    validate_creator_subscriber_tier_input,
};
pub(crate) use live::{
    build_creator_live_snapshot, contract_broadcast, contract_broadcasts, contract_creator_profile,
    contract_live_status, creator_live_channel_id,
    fetch_authoritative_creator_live_control_response,
    fetch_authoritative_creator_live_runtime_response,
    fetch_creator_live_socket_presence_by_id_raw, get_creator_live, normalize_creator_live_profile,
    publish_authoritative_creator_live_state, publish_current_creator_live_state,
};
#[cfg(test)]
pub(crate) use live::{
    fetch_creator_live_control_response, fetch_creator_live_runtime_response,
    get_creator_live_socket_session, publish_creator_live_state,
    reconcile_creator_live_socket_session,
};
