use super::discovery::fetch_user;
use super::*;

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/api/v1/creator/me/subscriber-tiers",
            get(list_creator_subscriber_tiers).post(create_creator_subscriber_tier),
        )
        .route(
            "/api/v1/creator/me/subscriber-tiers/:tier_id",
            patch(update_creator_subscriber_tier),
        )
        .route(
            "/api/v1/creator/me/subscriber-tiers/:tier_id/retire",
            post(retire_creator_subscriber_tier),
        )
        .route(
            "/api/v1/creator/me/series",
            get(list_creator_series).post(create_creator_series),
        )
        .route(
            "/api/v1/creator/me/series/:id",
            patch(update_creator_series),
        )
        .route(
            "/api/v1/creator/subscriptions/:creator_id/tiers/:tier_id",
            post(subscribe_to_creator_tier),
        )
        .route(
            "/api/v1/creator/subscriptions/:creator_id",
            delete(cancel_creator_subscription),
        )
        .route("/api/v1/creator/me/analytics", get(list_analytics))
        .route("/api/v1/creator/me/revenue", get(list_revenue))
        .route("/api/v1/creator/me/notifications", get(list_notifications))
        .route(
            "/api/v1/creator/me/notifications/:notification_id/read",
            post(mark_creator_notification_read),
        )
}

async fn list_creator_subscriber_tiers(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<CreatorSubscriberTier>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_creator_subscriber_tiers(&state.pool, creator_id).await?,
    ))
}

async fn create_creator_subscriber_tier(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<CreateCreatorSubscriberTierRequest>,
) -> AppResult<Json<CreatorSubscriberTier>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-subscriber-tier-create:{}", identity.user_id),
        20,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    ensure_creator_can_manage_subscription_tiers(&state.pool, creator_id).await?;
    validate_creator_subscriber_tier_input(
        input.tier_name.trim(),
        input.rank,
        input.monthly_price,
        input.accent_color.trim(),
    )?;
    let tier_id = format!("tier-{}", Uuid::new_v4().simple());
    let rank = match input.rank {
        Some(rank) => rank,
        None => next_creator_subscriber_tier_rank(&state.pool, creator_id).await?,
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
    .execute(&state.pool)
    .await?;
    normalize_creator_subscriber_tier_ranks(&state.pool, creator_id).await?;
    let _ = enqueue_notification_event(
        &state.pool,
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
        fetch_creator_subscriber_tier_by_id(&state.pool, creator_id, &tier_id).await?,
    ))
}

async fn update_creator_subscriber_tier(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(tier_id): Path<String>,
    Json(input): Json<UpdateCreatorSubscriberTierRequest>,
) -> AppResult<Json<CreatorSubscriberTier>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-subscriber-tier-update:{}", identity.user_id),
        40,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    ensure_creator_can_manage_subscription_tiers(&state.pool, creator_id).await?;
    let current = fetch_creator_subscriber_tier_by_id(&state.pool, creator_id, &tier_id).await?;
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
    .execute(&state.pool)
    .await?;
    normalize_creator_subscriber_tier_ranks(&state.pool, creator_id).await?;
    Ok(Json(
        fetch_creator_subscriber_tier_by_id(&state.pool, creator_id, &tier_id).await?,
    ))
}

async fn retire_creator_subscriber_tier(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(tier_id): Path<String>,
) -> AppResult<Json<CreatorSubscriberTier>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let current = fetch_creator_subscriber_tier_by_id(&state.pool, creator_id, &tier_id).await?;
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
    .execute(&state.pool)
    .await?;
    Ok(Json(
        fetch_creator_subscriber_tier_by_id(&state.pool, creator_id, &tier_id).await?,
    ))
}

async fn list_creator_series(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<CreatorSeriesProject>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(fetch_creator_series(&state.pool, creator_id).await?))
}

async fn create_creator_series(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<CreateCreatorSeriesRequest>,
) -> AppResult<Json<CreatorSeriesProject>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-series-create:{}", identity.user_id),
        20,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    if input.slug.trim().is_empty() || input.title.trim().is_empty() {
        return Err(AppError::BadRequest(
            "slug and title are required".to_string(),
        ));
    }

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO creator_series_projects (
            id, creator_id, slug, title, synopsis, rating, genres_json, hero_color,
            poster_url, backdrop_url, status, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(creator_id)
    .bind(input.slug.trim())
    .bind(input.title.trim())
    .bind(input.synopsis.trim())
    .bind(input.rating.trim())
    .bind(to_json(&input.genres)?)
    .bind(input.hero_color.trim())
    .bind(input.poster_url.trim())
    .bind(input.backdrop_url.trim())
    .bind(input.status.trim())
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;

    Ok(Json(
        fetch_creator_series_by_id(&state.pool, creator_id, &id).await?,
    ))
}

async fn update_creator_series(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<UpdateCreatorSeriesRequest>,
) -> AppResult<Json<CreatorSeriesProject>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-series-update:{}", identity.user_id),
        40,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    let current = fetch_creator_series_by_id(&state.pool, creator_id, &id).await?;

    sqlx::query(
        r#"
        UPDATE creator_series_projects
        SET title = ?, synopsis = ?, rating = ?, genres_json = ?, hero_color = ?,
            poster_url = ?, backdrop_url = ?, status = ?, updated_at = ?
        WHERE id = ? AND creator_id = ?
        "#,
    )
    .bind(input.title.unwrap_or(current.title))
    .bind(input.synopsis.unwrap_or(current.synopsis))
    .bind(input.rating.unwrap_or(current.rating))
    .bind(to_json(&input.genres.unwrap_or(current.genres))?)
    .bind(input.hero_color.unwrap_or(current.hero_color))
    .bind(input.poster_url.unwrap_or(current.poster_url))
    .bind(input.backdrop_url.unwrap_or(current.backdrop_url))
    .bind(input.status.unwrap_or(current.status))
    .bind(Utc::now().to_rfc3339())
    .bind(&id)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;

    Ok(Json(
        fetch_creator_series_by_id(&state.pool, creator_id, &id).await?,
    ))
}

async fn subscribe_to_creator_tier(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((creator_id, tier_id)): Path<(String, String)>,
) -> AppResult<Json<CreatorMembership>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-subscription:{}", identity.user_id),
        12,
        Duration::from_secs(60),
    )
    .await?;
    ensure_creator_can_accept_paid_transactions(&state.pool, &creator_id).await?;
    let tier = fetch_creator_subscriber_tier_by_id(&state.pool, &creator_id, &tier_id).await?;
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
    .execute(&state.pool)
    .await?;
    let subscriber = fetch_user(&state.pool, &identity.user_id).await?;
    enqueue_notification_event(
        &state.pool,
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
        fetch_creator_membership(&state.pool, &identity.user_id, &creator_id).await?,
    ))
}

async fn cancel_creator_subscription(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(creator_id): Path<String>,
) -> AppResult<StatusCode> {
    let identity = require_identity(&state.pool, &headers).await?;
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
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn list_analytics(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<AnalyticsPoint>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(fetch_analytics(&state.pool, creator_id).await?))
}

async fn list_revenue(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<RevenueEntry>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(fetch_revenue_entries(&state.pool, creator_id).await?))
}

async fn list_notifications(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<CreatorNotification>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_notifications_rows(&state.pool, creator_id).await?,
    ))
}

async fn mark_creator_notification_read(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(notification_id): Path<String>,
) -> AppResult<StatusCode> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        "UPDATE notification_deliveries SET read_at = COALESCE(read_at, ?) WHERE id = ? AND recipient_creator_id = ?",
    )
    .bind(&now)
    .bind(&notification_id)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
