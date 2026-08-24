use super::*;

pub(crate) async fn update_upload(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<UpdateUploadRequest>,
) -> AppResult<Json<Upload>> {
    let identity = require_identity(&state.db, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-update-upload:{}", identity.user_id),
        60,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    let current = fetch_upload_by_id(state.db.sqlite_adapter(), creator_id, &id).await?;
    if current.status == "taken_down"
        && (input.visibility.is_some()
            || input.release_at.is_some()
            || input.access_policy.is_some()
            || input.access_tier_id.is_some()
            || input.price_cents.is_some()
            || input.currency.is_some()
            || input.rental_window_hours.is_some())
    {
        return Err(AppError::BadRequest(
            "taken-down uploads cannot change lifecycle or access controls through content updates"
                .to_string(),
        ));
    }
    let now = Utc::now().to_rfc3339();
    let access_terms = resolve_upload_access_terms(
        input
            .access_policy
            .or_else(|| Some(current.access_policy.clone())),
        input.access_tier_id.or(current.access_tier_id.clone()),
        input.price_cents.or(current.price_cents),
        input.currency.or(current.currency.clone()),
        input.rental_window_hours.or(current.rental_window_hours),
    )?;
    if monetized_access_policy(&access_terms.access_policy) {
        ensure_creator_can_publish_paid_content(state.db.sqlite_adapter(), creator_id).await?;
    }
    validate_creator_access_tier(
        state.db.sqlite_adapter(),
        creator_id,
        &access_terms.access_policy,
        access_terms.access_tier_id.as_deref(),
    )
    .await?;
    let slug = match input.slug {
        Some(slug) => Some(sanitize_slug(&slug)?),
        None => current.slug.clone(),
    };
    let release_at = input.release_at.or(current.release_at.clone());
    let visibility = input.visibility.unwrap_or(current.visibility.clone());
    validate_upload_visibility(&visibility)?;
    let next_status = derive_upload_lifecycle_status(
        current.status.as_str(),
        &visibility,
        release_at.as_deref(),
        &now,
    )?;
    sqlx::query(
        "UPDATE uploads SET title = ?, slug = ?, description = ?, status = ?, visibility = ?, release_at = ?, access_policy = ?, access_tier_id = ?, price_cents = ?, currency = ?, rental_window_hours = ?, published_at = CASE WHEN ? = 'published' AND published_at IS NULL THEN ? WHEN ? != 'published' THEN published_at ELSE published_at END WHERE id = ?",
    )
    .bind(input.title.unwrap_or(current.title))
    .bind(slug)
    .bind(input.description.unwrap_or(current.description))
    .bind(&next_status)
    .bind(&visibility)
    .bind(&release_at)
    .bind(access_terms.access_policy)
    .bind(access_terms.access_tier_id)
    .bind(access_terms.price_cents)
    .bind(access_terms.currency)
    .bind(access_terms.rental_window_hours)
    .bind(&next_status)
    .bind(&now)
    .bind(&next_status)
    .bind(&id)
    .execute(state.db.sqlite_adapter())
    .await?;
    sqlx::query(
        "UPDATE media_assets SET visibility = ?, status = ?, updated_at = ? WHERE upload_id = ? AND creator_id = ?",
    )
    .bind(&visibility)
    .bind(&next_status)
    .bind(&now)
    .bind(&id)
    .bind(creator_id)
    .execute(state.db.sqlite_adapter())
    .await?;
    expire_playback_sessions_for_upload(state.db.sqlite_adapter(), &id).await?;
    Ok(Json(
        fetch_upload_by_id(state.db.sqlite_adapter(), creator_id, &id).await?,
    ))
}
