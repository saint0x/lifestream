use super::*;

pub(crate) async fn build_creator_live_snapshot(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorLiveSnapshot> {
    let mut ingest_session = fetch_active_live_ingest_session(pool, creator_id).await?;
    if let Some(session) = ingest_session.as_ref() {
        if is_live_ingest_session_stale(&session) {
            mark_live_ingest_session_stale_in_db(pool, &session).await?;
            ingest_session = fetch_active_live_ingest_session_unreconciled(pool, creator_id).await?;
        }
    }
    let broadcasts = fetch_broadcasts(pool, creator_id).await?;
    let profile = build_effective_creator_live_profile(pool, creator_id, &broadcasts).await?;
    let current_broadcast = broadcasts
        .iter()
        .find(|item| item.status == "live")
        .cloned();
    let pending_broadcast = broadcasts
        .iter()
        .find(|item| item.status == "ready")
        .cloned();
    Ok(CreatorLiveSnapshot {
        profile: contract_creator_profile(profile),
        current_broadcast: current_broadcast.map(contract_broadcast),
        pending_broadcast: pending_broadcast.map(contract_broadcast),
        ingest_session,
    })
}

async fn build_effective_creator_live_profile(
    pool: &SqlitePool,
    creator_id: &str,
    broadcasts: &[Broadcast],
) -> AppResult<CreatorProfile> {
    let mut profile = fetch_creator_profile_persisted(pool, creator_id).await?;
    let current_broadcast = broadcasts
        .iter()
        .find(|item| item.status == "live")
        .map(|item| item.id.clone());
    let pending_broadcast = broadcasts
        .iter()
        .find(|item| item.status == "ready")
        .map(|item| item.id.clone());
    profile.current_broadcast_id = current_broadcast.or(pending_broadcast);
    profile.live_status = if broadcasts.iter().any(|item| item.status == "live") {
        "live".to_string()
    } else if broadcasts.iter().any(|item| item.status == "ready") {
        "ready".to_string()
    } else {
        "offline".to_string()
    };
    Ok(profile)
}

pub(crate) fn contract_live_status(status: &str) -> String {
    match status {
        "ready" => "starting".to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn contract_broadcast_status(status: &str) -> String {
    match status {
        "ready" => "scheduled".to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn contract_creator_profile(mut profile: CreatorProfile) -> CreatorProfile {
    profile.live_status = contract_live_status(&profile.live_status);
    profile
}

pub(crate) fn contract_broadcast(mut broadcast: Broadcast) -> Broadcast {
    broadcast.status = contract_broadcast_status(&broadcast.status);
    broadcast
}

pub(crate) fn contract_broadcasts(broadcasts: Vec<Broadcast>) -> Vec<Broadcast> {
    broadcasts.into_iter().map(contract_broadcast).collect()
}

pub(crate) async fn normalize_creator_live_profile(
    pool: &SqlitePool,
    creator_id: &str,
    broadcasts: Vec<Broadcast>,
) -> AppResult<CreatorProfile> {
    let mut profile = fetch_creator_profile_persisted(pool, creator_id).await?;
    let current_broadcast = broadcasts
        .iter()
        .find(|item| item.status == "live")
        .map(|item| item.id.clone());
    let pending_broadcast = broadcasts
        .iter()
        .find(|item| item.status == "ready")
        .map(|item| item.id.clone());
    let desired_current_broadcast_id = current_broadcast.or(pending_broadcast);
    let desired_live_status = if broadcasts.iter().any(|item| item.status == "live") {
        "live"
    } else if broadcasts.iter().any(|item| item.status == "ready") {
        "ready"
    } else {
        "offline"
    };

    if profile.current_broadcast_id != desired_current_broadcast_id
        || profile.live_status != desired_live_status
    {
        sqlx::query(
            "UPDATE creator_profiles SET live_status = ?, current_broadcast_id = ? WHERE id = ?",
        )
        .bind(desired_live_status)
        .bind(desired_current_broadcast_id.clone())
        .bind(creator_id)
        .execute(pool)
        .await?;
        profile = fetch_creator_profile_persisted(pool, creator_id).await?;
    }

    Ok(profile)
}
