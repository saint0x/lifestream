use super::*;

pub(crate) async fn ensure_creator_live_streaming_enabled(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<()> {
    let profile = fetch_creator_profile(pool, creator_id).await?;
    let state = fetch_creator_operational_state(pool, &profile).await?;
    if state.live_streaming_enabled {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "creator is not currently allowed to start or connect live streams".to_string(),
        ))
    }
}

pub(crate) async fn ensure_creator_upload_ingest_enabled(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<()> {
    let profile = fetch_creator_profile(pool, creator_id).await?;
    let state = fetch_creator_operational_state(pool, &profile).await?;
    if state.upload_ingest_enabled {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "creator is not currently allowed to ingest or publish uploads".to_string(),
        ))
    }
}

pub(crate) async fn ensure_creator_collaboration_enabled(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<()> {
    let profile = fetch_creator_profile(pool, creator_id).await?;
    let state = fetch_creator_operational_state(pool, &profile).await?;
    if state.collaboration_enabled {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "creator is not currently allowed to manage collaboration sessions".to_string(),
        ))
    }
}

pub(crate) async fn ensure_creator_can_manage_subscription_tiers(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<()> {
    let profile = fetch_creator_profile(pool, creator_id).await?;
    let state = fetch_creator_operational_state(pool, &profile).await?;
    if state.can_monetize {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "creator is not cleared to manage subscription tiers".to_string(),
        ))
    }
}

pub(crate) async fn validate_creator_access_tier(
    pool: &SqlitePool,
    creator_id: &str,
    access_policy: &str,
    access_tier_id: Option<&str>,
) -> AppResult<()> {
    if !matches!(access_policy, "subscription" | "subscription_or_purchase") {
        return Ok(());
    }
    let tier_id = access_tier_id.ok_or_else(|| {
        AppError::BadRequest(
            "subscription-based access requires an active subscriber tier".to_string(),
        )
    })?;
    let tier = fetch_creator_subscriber_tier_by_id(pool, creator_id, tier_id).await?;
    if tier.status != "active" {
        return Err(AppError::BadRequest(
            "subscription-based access requires an active subscriber tier".to_string(),
        ));
    }
    Ok(())
}

pub(crate) async fn ensure_creator_can_publish_paid_content(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<()> {
    let profile = fetch_creator_profile(pool, creator_id).await?;
    let state = fetch_creator_operational_state(pool, &profile).await?;
    if state.can_publish_paid_content {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "creator is not cleared to publish paid content".to_string(),
        ))
    }
}

pub(crate) async fn ensure_creator_can_accept_paid_transactions(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<()> {
    let profile = fetch_creator_profile(pool, creator_id).await?;
    let state = fetch_creator_operational_state(pool, &profile).await?;
    if state.can_receive_payouts {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "creator is not cleared to accept paid transactions".to_string(),
        ))
    }
}
