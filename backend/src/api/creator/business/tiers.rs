use super::*;

pub(super) async fn list_creator_subscriber_tiers(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<CreatorSubscriberTier>>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_creator_subscriber_tiers(state.db.try_sqlite_adapter()?, creator_id).await?,
    ))
}

pub(super) async fn create_creator_subscriber_tier(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<CreateCreatorSubscriberTierRequest>,
) -> AppResult<Json<CreatorSubscriberTier>> {
    let identity = require_identity(&state.db, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-subscriber-tier-create:{}", identity.user_id),
        20,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    ensure_creator_can_manage_subscription_tiers(state.db.try_sqlite_adapter()?, creator_id)
        .await?;
    validate_creator_subscriber_tier_input(
        input.tier_name.trim(),
        input.rank,
        input.monthly_price,
        input.accent_color.trim(),
    )?;
    let tier_id = format!("tier-{}", Uuid::new_v4().simple());
    let rank = match input.rank {
        Some(rank) => rank,
        None => {
            next_creator_subscriber_tier_rank(state.db.try_sqlite_adapter()?, creator_id).await?
        }
    };

    sqlx::query(
        r#"
        INSERT INTO creator_subscriber_tiers (
            id, creator_id, tier_name, monthly_price, subscriber_count, accent_color, rank, status, retired_at
        ) VALUES (?, ?, ?, ?, 0, ?, ?, 'active', NULL)
        "#,
    )
    .bind(&tier_id)
    .bind(creator_id)
    .bind(input.tier_name.trim())
    .bind(input.monthly_price)
    .bind(input.accent_color.trim())
    .bind(rank)
    .execute(state.db.try_sqlite_adapter()?)
    .await?;
    normalize_creator_subscriber_tier_ranks(state.db.try_sqlite_adapter()?, creator_id).await?;
    let _ = enqueue_notification_event(
        state.db.try_sqlite_adapter()?,
        "subscriber_tier_created",
        &format!(
            "{} was added to your subscription offerings.",
            input.tier_name.trim()
        ),
        Some(&identity.user_id),
        Some("creator"),
        Some(creator_id),
        None,
        Some(input.monthly_price),
        json!({
            "tierId": tier_id,
            "tierName": input.tier_name.trim(),
            "rank": rank,
        }),
        &[],
        &[creator_id.to_string()],
    )
    .await;
    Ok(Json(
        fetch_creator_subscriber_tier_by_id(state.db.try_sqlite_adapter()?, creator_id, &tier_id)
            .await?,
    ))
}

pub(super) async fn update_creator_subscriber_tier(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(tier_id): Path<String>,
    Json(input): Json<UpdateCreatorSubscriberTierRequest>,
) -> AppResult<Json<CreatorSubscriberTier>> {
    let identity = require_identity(&state.db, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-subscriber-tier-update:{}", identity.user_id),
        40,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    ensure_creator_can_manage_subscription_tiers(state.db.try_sqlite_adapter()?, creator_id)
        .await?;
    let current =
        fetch_creator_subscriber_tier_by_id(state.db.try_sqlite_adapter()?, creator_id, &tier_id)
            .await?;
    if current.status != "active" {
        return Err(AppError::BadRequest(
            "retired subscriber tiers cannot be updated".to_string(),
        ));
    }
    let next_tier_name = input.tier_name.unwrap_or(current.tier_name.clone());
    let next_rank = input.rank.unwrap_or(current.rank);
    let next_monthly_price = input.monthly_price.unwrap_or(current.monthly_price);
    let next_accent_color = input.accent_color.unwrap_or(current.accent_color.clone());
    validate_creator_subscriber_tier_input(
        next_tier_name.trim(),
        Some(next_rank),
        next_monthly_price,
        next_accent_color.trim(),
    )?;

    sqlx::query(
        r#"
        UPDATE creator_subscriber_tiers
        SET tier_name = ?, rank = ?, monthly_price = ?, accent_color = ?
        WHERE id = ? AND creator_id = ?
        "#,
    )
    .bind(next_tier_name.trim())
    .bind(next_rank)
    .bind(next_monthly_price)
    .bind(next_accent_color.trim())
    .bind(&tier_id)
    .bind(creator_id)
    .execute(state.db.try_sqlite_adapter()?)
    .await?;
    normalize_creator_subscriber_tier_ranks(state.db.try_sqlite_adapter()?, creator_id).await?;
    Ok(Json(
        fetch_creator_subscriber_tier_by_id(state.db.try_sqlite_adapter()?, creator_id, &tier_id)
            .await?,
    ))
}

pub(super) async fn retire_creator_subscriber_tier(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(tier_id): Path<String>,
) -> AppResult<Json<CreatorSubscriberTier>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let current =
        fetch_creator_subscriber_tier_by_id(state.db.try_sqlite_adapter()?, creator_id, &tier_id)
            .await?;
    if current.status == "retired" {
        return Ok(Json(current));
    }
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE creator_subscriber_tiers SET status = 'retired', retired_at = ? WHERE id = ? AND creator_id = ?",
    )
    .bind(&now)
    .bind(&tier_id)
    .bind(creator_id)
    .execute(state.db.try_sqlite_adapter()?)
    .await?;
    Ok(Json(
        fetch_creator_subscriber_tier_by_id(state.db.try_sqlite_adapter()?, creator_id, &tier_id)
            .await?,
    ))
}
