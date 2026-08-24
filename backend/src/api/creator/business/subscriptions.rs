use super::*;

pub(super) async fn subscribe_to_creator_tier(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((creator_id, tier_id)): Path<(String, String)>,
) -> AppResult<Json<CreatorMembership>> {
    let identity = require_identity(&state.db, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-subscription:{}", identity.user_id),
        12,
        Duration::from_secs(60),
    )
    .await?;
    ensure_creator_can_accept_paid_transactions(state.db.sqlite_adapter(), &creator_id).await?;
    let tier =
        fetch_creator_subscriber_tier_by_id(state.db.sqlite_adapter(), &creator_id, &tier_id)
            .await?;
    if tier.status != "active" {
        return Err(AppError::BadRequest(
            "subscriber tier is not available for new subscriptions".to_string(),
        ));
    }
    let now = Utc::now();
    let started_at = now.to_rfc3339();
    let renews_at = (now + chrono::Duration::days(30)).to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO creator_memberships (
            user_id, creator_id, tier_id, status, started_at, renews_at, ends_at, canceled_at
        ) VALUES (?, ?, ?, 'active', ?, ?, NULL, NULL)
        ON CONFLICT(user_id, creator_id) DO UPDATE SET
            tier_id = excluded.tier_id,
            status = 'active',
            started_at = excluded.started_at,
            renews_at = excluded.renews_at,
            ends_at = NULL,
            canceled_at = NULL
        "#,
    )
    .bind(&identity.user_id)
    .bind(&creator_id)
    .bind(&tier.id)
    .bind(&started_at)
    .bind(&renews_at)
    .execute(state.db.sqlite_adapter())
    .await?;
    let subscriber = fetch_user(state.db.sqlite_adapter(), &identity.user_id).await?;
    enqueue_notification_event(
        state.db.sqlite_adapter(),
        "creator_subscription",
        &format!(
            "{} subscribed to {}.",
            subscriber.display_name, tier.tier_name
        ),
        Some(&identity.user_id),
        Some(&subscriber.display_name),
        Some(&creator_id),
        None,
        Some(tier.monthly_price),
        json!({
            "tierId": tier.id,
            "tierName": tier.tier_name,
            "membershipStartedAt": started_at,
        }),
        &[],
        &[creator_id.clone()],
    )
    .await?;

    Ok(Json(
        fetch_creator_membership(state.db.sqlite_adapter(), &identity.user_id, &creator_id).await?,
    ))
}

pub(super) async fn cancel_creator_subscription(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(creator_id): Path<String>,
) -> AppResult<StatusCode> {
    let identity = require_identity(&state.db, &headers).await?;
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        r#"
        UPDATE creator_memberships
        SET status = 'canceling', canceled_at = ?, ends_at = COALESCE(ends_at, renews_at, ?)
        WHERE user_id = ? AND creator_id = ? AND status IN ('active', 'canceling')
        "#,
    )
    .bind(&now)
    .bind(&now)
    .bind(&identity.user_id)
    .bind(&creator_id)
    .execute(state.db.sqlite_adapter())
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}
