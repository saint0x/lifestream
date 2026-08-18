use super::*;
use super::runtime::{
    fetch_creator_live_control_response, fetch_creator_live_runtime_response,
};

pub(crate) fn creator_live_channel_id(creator_id: &str) -> String {
    format!("creator-live:{creator_id}")
}

pub(crate) async fn publish_current_creator_live_state(
    state: &SharedState,
    creator_id: &str,
) -> AppResult<()> {
    let event = WsEvent::CreatorLiveState {
        control: fetch_creator_live_control_response(&state.pool, creator_id).await?,
        runtime: fetch_creator_live_runtime_response(&state.pool, creator_id).await?,
    };
    state
        .realtime
        .publish(&creator_live_channel_id(creator_id), event)
        .await;
    Ok(())
}

pub(crate) async fn publish_authoritative_creator_live_state(
    state: &SharedState,
    creator_id: &str,
) -> AppResult<()> {
    let event = WsEvent::CreatorLiveState {
        control: fetch_authoritative_creator_live_control_response(state, creator_id).await?,
        runtime: fetch_authoritative_creator_live_runtime_response(state, creator_id).await?,
    };
    state
        .realtime
        .publish(&creator_live_channel_id(creator_id), event)
        .await;
    Ok(())
}

pub(crate) async fn publish_creator_live_state(
    state: &SharedState,
    creator_id: &str,
) -> AppResult<()> {
    publish_authoritative_creator_live_state(state, creator_id).await
}
