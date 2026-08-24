use super::*;

pub(crate) async fn purchase_upload_access(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(upload_id): Path<String>,
) -> AppResult<Json<ContentPurchase>> {
    purchase_content_access_for_id(state, headers, upload_id).await
}

pub(crate) async fn purchase_content_access(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(content_id): Path<String>,
) -> AppResult<Json<ContentPurchase>> {
    purchase_content_access_for_id(state, headers, content_id).await
}

async fn purchase_content_access_for_id(
    state: SharedState,
    headers: HeaderMap,
    content_id: String,
) -> AppResult<Json<ContentPurchase>> {
    let identity = require_identity(&state.db, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("purchase-upload:{}", identity.user_id),
        20,
        Duration::from_secs(60),
    )
    .await?;
    let target = fetch_upload_playback_target(state.db.sqlite_adapter(), &content_id).await?;
    let terms = resolve_upload_access_terms(
        Some(target.upload.access_policy.clone()),
        target.upload.access_tier_id.clone(),
        target.upload.price_cents,
        target.upload.currency.clone(),
        target.upload.rental_window_hours,
    )?;
    if terms.access_policy != "purchase" && terms.access_policy != "subscription_or_purchase" {
        return Err(AppError::BadRequest(
            "content is not configured for direct purchase".to_string(),
        ));
    }
    ensure_creator_can_accept_paid_transactions(state.db.sqlite_adapter(), &target.creator_id)
        .await?;
    if let Some(existing_purchase) = fetch_current_content_purchase(
        state.db.sqlite_adapter(),
        &identity.user_id,
        &target.upload.id,
    )
    .await?
    {
        return Ok(Json(existing_purchase));
    }
    let now = Utc::now();
    let purchased_at = now.to_rfc3339();
    let expires_at = terms
        .rental_window_hours
        .map(|hours| (now + chrono::Duration::hours(hours)).to_rfc3339());
    let purchase_id = format!("pur-{}", Uuid::new_v4().simple());

    sqlx::query(
        r#"
        INSERT INTO content_purchases (
            id, user_id, creator_id, upload_id, access_policy, amount_cents, currency,
            status, purchased_at, expires_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, 'active', ?, ?)
        "#,
    )
    .bind(&purchase_id)
    .bind(&identity.user_id)
    .bind(&target.creator_id)
    .bind(&target.upload.id)
    .bind(&terms.access_policy)
    .bind(terms.price_cents.unwrap_or_default())
    .bind(terms.currency.clone().unwrap_or_else(|| "USD".to_string()))
    .bind(&purchased_at)
    .bind(expires_at)
    .execute(state.db.sqlite_adapter())
    .await?;
    let buyer = fetch_user(state.db.sqlite_adapter(), &identity.user_id).await?;
    enqueue_notification_event(
        state.db.sqlite_adapter(),
        "content_purchase",
        &format!("{} purchased {}.", buyer.display_name, target.upload.title),
        Some(&identity.user_id),
        Some(&buyer.display_name),
        Some(&target.creator_id),
        None,
        Some(terms.price_cents.unwrap_or_default() as f64 / 100.0),
        json!({
            "purchaseId": purchase_id,
            "uploadId": target.upload.id,
            "accessPolicy": terms.access_policy,
        }),
        &[],
        &[target.creator_id.clone()],
    )
    .await?;

    Ok(Json(
        fetch_content_purchase_by_id(state.db.sqlite_adapter(), &purchase_id).await?,
    ))
}
