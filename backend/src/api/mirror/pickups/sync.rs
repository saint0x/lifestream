use super::*;
use crate::api::creator_live::publish_current_creator_live_state;

pub(crate) async fn sync_active_collaboration_mirror_pickups_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<()> {
    let pickups = fetch_collaboration_mirror_pickups_for_session(pool, session_id).await?;
    for pickup in pickups
        .into_iter()
        .filter(|pickup| pickup.state == "active")
    {
        sync_collaboration_mirror_pickup_broadcast_state(pool, &pickup).await?;
    }
    Ok(())
}

pub(crate) async fn publish_creator_live_states_for_creators(
    state: &SharedState,
    creator_ids: impl IntoIterator<Item = String>,
) -> AppResult<()> {
    let mut unique = std::collections::BTreeSet::new();
    for creator_id in creator_ids {
        if unique.insert(creator_id.clone()) {
            publish_current_creator_live_state(state, &creator_id).await?;
        }
    }
    Ok(())
}

pub(crate) async fn sync_active_collaboration_mirror_pickups_for_session_and_publish(
    state: &SharedState,
    session_id: &str,
) -> AppResult<()> {
    sync_active_collaboration_mirror_pickups_for_session(&state.pool, session_id).await?;
    let pickups = fetch_collaboration_mirror_pickups_for_session(&state.pool, session_id).await?;
    publish_creator_live_states_for_creators(
        state,
        pickups
            .into_iter()
            .filter(|pickup| pickup.state == "active")
            .map(|pickup| pickup.guest_creator_id),
    )
    .await
}
