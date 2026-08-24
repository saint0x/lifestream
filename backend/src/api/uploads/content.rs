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
    let current = fetch_upload_by_id_for_database(&state.db, creator_id, &id).await?;
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
        ensure_creator_can_publish_paid_content_for_database(&state.db, creator_id).await?;
    }
    validate_creator_access_tier_for_database(
        &state.db,
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
    update_upload_content_record(
        &state.db,
        creator_id,
        &id,
        input.title.unwrap_or(current.title),
        slug,
        input.description.unwrap_or(current.description),
        &next_status,
        &visibility,
        release_at,
        access_terms.access_policy,
        access_terms.access_tier_id,
        access_terms.price_cents,
        access_terms.currency,
        access_terms.rental_window_hours,
        &now,
    )
    .await?;
    sync_upload_media_asset_lifecycle(&state.db, creator_id, &id, &visibility, &next_status, &now)
        .await?;
    expire_playback_sessions_for_upload_in_database(&state.db, &id).await?;
    Ok(Json(
        fetch_upload_by_id_for_database(&state.db, creator_id, &id).await?,
    ))
}

async fn update_upload_content_record(
    database: &crate::db::Database,
    creator_id: &str,
    upload_id: &str,
    title: String,
    slug: Option<String>,
    description: String,
    status: &str,
    visibility: &str,
    release_at: Option<String>,
    access_policy: String,
    access_tier_id: Option<String>,
    price_cents: Option<i64>,
    currency: Option<String>,
    rental_window_hours: Option<i64>,
    now: &str,
) -> AppResult<()> {
    if let Ok(pool) = database.try_postgres_adapter() {
        sqlx::query(
            r#"
            UPDATE uploads
            SET title = $1,
                slug = $2,
                description = $3,
                status = $4,
                visibility = $5,
                release_at = $6,
                access_policy = $7,
                access_tier_id = $8,
                price_cents = $9,
                currency = $10,
                rental_window_hours = $11,
                published_at = CASE
                    WHEN $12 = 'published' AND published_at IS NULL THEN $13
                    WHEN $14 != 'published' THEN published_at
                    ELSE published_at
                END
            WHERE id = $15 AND creator_id = $16
            "#,
        )
        .bind(title)
        .bind(slug)
        .bind(description)
        .bind(status)
        .bind(visibility)
        .bind(release_at)
        .bind(access_policy)
        .bind(access_tier_id)
        .bind(price_cents)
        .bind(currency)
        .bind(rental_window_hours)
        .bind(status)
        .bind(now)
        .bind(status)
        .bind(upload_id)
        .bind(creator_id)
        .execute(pool)
        .await?;
        return Ok(());
    }

    sqlx::query(
        r#"
        UPDATE uploads
        SET title = ?,
            slug = ?,
            description = ?,
            status = ?,
            visibility = ?,
            release_at = ?,
            access_policy = ?,
            access_tier_id = ?,
            price_cents = ?,
            currency = ?,
            rental_window_hours = ?,
            published_at = CASE
                WHEN ? = 'published' AND published_at IS NULL THEN ?
                WHEN ? != 'published' THEN published_at
                ELSE published_at
            END
        WHERE id = ? AND creator_id = ?
        "#,
    )
    .bind(title)
    .bind(slug)
    .bind(description)
    .bind(status)
    .bind(visibility)
    .bind(release_at)
    .bind(access_policy)
    .bind(access_tier_id)
    .bind(price_cents)
    .bind(currency)
    .bind(rental_window_hours)
    .bind(status)
    .bind(now)
    .bind(status)
    .bind(upload_id)
    .bind(creator_id)
    .execute(database.try_sqlite_adapter()?)
    .await?;
    Ok(())
}
