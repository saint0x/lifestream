use super::*;

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/api/v1/creator/me/uploads", get(list_uploads))
        .route("/api/v1/creator/me/content", get(get_creator_content))
        .route("/api/v1/creator/me/uploads/:id", patch(update_upload))
        .route(
            "/api/v1/creator/me/uploads/:id/lifecycle",
            patch(update_upload_lifecycle),
        )
        .route(
            "/api/v1/creator/me/uploads/:id/unpublish",
            post(unpublish_upload),
        )
        .route(
            "/api/v1/creator/me/uploads/:id/takedown",
            post(takedown_upload),
        )
        .route("/api/v1/creator/me/uploads/bulk", post(bulk_uploads))
}

pub(super) async fn list_uploads(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<Upload>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_creator_scope()?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(fetch_uploads(&state.pool, creator_id).await?))
}

pub(super) async fn get_creator_content(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<CreatorContentQuery>,
) -> AppResult<Json<CreatorContentResponse>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let uploads = fetch_uploads(&state.pool, creator_id).await?;
    let filtered_uploads = filter_creator_uploads(uploads.clone(), &query)?;

    Ok(Json(CreatorContentResponse {
        summary: summarize_creator_content(&uploads, filtered_uploads.len() as i64),
        uploads: filtered_uploads,
    }))
}

pub(super) async fn update_upload(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<UpdateUploadRequest>,
) -> AppResult<Json<Upload>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-update-upload:{}", identity.user_id),
        60,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    let current = fetch_upload_by_id(&state.pool, creator_id, &id).await?;
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
        ensure_creator_can_publish_paid_content(&state.pool, creator_id).await?;
    }
    validate_creator_access_tier(
        &state.pool,
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
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE media_assets SET visibility = ?, status = ?, updated_at = ? WHERE upload_id = ? AND creator_id = ?",
    )
    .bind(&visibility)
    .bind(&next_status)
    .bind(&now)
    .bind(&id)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;
    expire_playback_sessions_for_upload(&state.pool, &id).await?;
    Ok(Json(
        fetch_upload_by_id(&state.pool, creator_id, &id).await?,
    ))
}

pub(super) async fn update_upload_lifecycle(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<UpdateUploadLifecycleRequest>,
) -> AppResult<Json<Upload>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let current = fetch_upload_by_id(&state.pool, creator_id, &id).await?;
    if current.status == "taken_down" {
        return Err(AppError::BadRequest(
            "taken-down uploads cannot be updated through lifecycle patch".to_string(),
        ));
    }
    let visibility = input.visibility.unwrap_or(current.visibility.clone());
    validate_upload_visibility(&visibility)?;
    let now = Utc::now().to_rfc3339();
    let release_at = input.release_at.or(current.release_at.clone());
    let next_status = derive_upload_lifecycle_status(
        current.status.as_str(),
        &visibility,
        release_at.as_deref(),
        &now,
    )?;
    sqlx::query(
        "UPDATE uploads SET visibility = ?, release_at = ?, status = ?, published_at = CASE WHEN ? = 'published' AND published_at IS NULL THEN ? ELSE published_at END WHERE id = ?",
    )
    .bind(&visibility)
    .bind(&release_at)
    .bind(&next_status)
    .bind(&next_status)
    .bind(&now)
    .bind(&id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE media_assets SET visibility = ?, status = ?, updated_at = ? WHERE upload_id = ? AND creator_id = ?",
    )
    .bind(&visibility)
    .bind(&next_status)
    .bind(&now)
    .bind(&id)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;
    expire_playback_sessions_for_upload(&state.pool, &id).await?;
    Ok(Json(
        fetch_upload_by_id(&state.pool, creator_id, &id).await?,
    ))
}

pub(super) async fn unpublish_upload(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<Upload>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let current = fetch_upload_by_id(&state.pool, creator_id, &id).await?;
    if current.status == "taken_down" {
        return Err(AppError::BadRequest(
            "taken-down uploads cannot be unpublished".to_string(),
        ));
    }
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE uploads SET visibility = 'private', status = 'draft', release_at = NULL WHERE id = ?",
    )
    .bind(&id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE media_assets SET visibility = 'private', status = 'draft', updated_at = ? WHERE upload_id = ? AND creator_id = ?",
    )
    .bind(&now)
    .bind(&id)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;
    expire_playback_sessions_for_upload(&state.pool, &id).await?;
    enqueue_notification_event(
        &state.pool,
        "content_unpublished",
        &format!("{} was unpublished.", current.title),
        Some(&identity.user_id),
        Some("creator"),
        Some(creator_id),
        None,
        None,
        json!({
            "uploadId": id,
            "previousStatus": current.status,
        }),
        &[],
        &[creator_id.to_string()],
    )
    .await?;
    Ok(Json(
        fetch_upload_by_id(&state.pool, creator_id, &id).await?,
    ))
}

pub(super) async fn takedown_upload(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<Upload>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let current = fetch_upload_by_id(&state.pool, creator_id, &id).await?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE uploads SET visibility = 'private', status = 'taken_down', release_at = NULL WHERE id = ?",
    )
    .bind(&id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE media_assets SET visibility = 'private', status = 'taken_down', updated_at = ? WHERE upload_id = ? AND creator_id = ?",
    )
    .bind(&now)
    .bind(&id)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;
    expire_playback_sessions_for_upload(&state.pool, &id).await?;
    enqueue_notification_event(
        &state.pool,
        "content_takedown",
        &format!("{} was taken down.", current.title),
        Some(&identity.user_id),
        Some("creator"),
        Some(creator_id),
        None,
        None,
        json!({
            "uploadId": id,
            "previousStatus": current.status,
        }),
        &[],
        &[creator_id.to_string()],
    )
    .await?;
    Ok(Json(
        fetch_upload_by_id(&state.pool, creator_id, &id).await?,
    ))
}

pub(super) async fn bulk_uploads(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<BulkUploadRequest>,
) -> AppResult<Json<Vec<Upload>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-bulk-uploads:{}", identity.user_id),
        20,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    if input.upload_ids.is_empty() {
        return Err(AppError::BadRequest(
            "uploadIds cannot be empty".to_string(),
        ));
    }
    let now = Utc::now().to_rfc3339();

    for upload_id in &input.upload_ids {
        let current = fetch_upload_by_id(&state.pool, creator_id, upload_id).await?;
        match input.action.as_str() {
            "archive" => {
                validate_bulk_upload_action(&current, "archive")?;
                sqlx::query(
                    "UPDATE uploads SET status = 'archived' WHERE id = ? AND creator_id = ?",
                )
                .bind(upload_id)
                .bind(creator_id)
                .execute(&state.pool)
                .await?;
                sqlx::query(
                    "UPDATE media_assets SET status = 'archived', updated_at = ? WHERE upload_id = ? AND creator_id = ?",
                )
                .bind(&now)
                .bind(upload_id)
                .bind(creator_id)
                .execute(&state.pool)
                .await?;
                expire_playback_sessions_for_upload(&state.pool, upload_id).await?;
            }
            "make_public" => {
                validate_bulk_upload_action(&current, "make_public")?;
                let next_status = derive_upload_lifecycle_status(
                    current.status.as_str(),
                    "public",
                    current.release_at.as_deref(),
                    &now,
                )?;
                sqlx::query(
                    "UPDATE uploads SET visibility = 'public', status = ?, published_at = CASE WHEN ? = 'published' AND published_at IS NULL THEN ? ELSE published_at END WHERE id = ? AND creator_id = ?",
                )
                .bind(&next_status)
                .bind(&next_status)
                .bind(&now)
                .bind(upload_id)
                .bind(creator_id)
                .execute(&state.pool)
                .await?;
                sqlx::query(
                    "UPDATE media_assets SET visibility = 'public', status = ?, updated_at = ? WHERE upload_id = ? AND creator_id = ?",
                )
                .bind(&next_status)
                .bind(&now)
                .bind(upload_id)
                .bind(creator_id)
                .execute(&state.pool)
                .await?;
                expire_playback_sessions_for_upload(&state.pool, upload_id).await?;
            }
            "make_unlisted" => {
                validate_bulk_upload_action(&current, "make_unlisted")?;
                let next_status = derive_upload_lifecycle_status(
                    current.status.as_str(),
                    "unlisted",
                    current.release_at.as_deref(),
                    &now,
                )?;
                sqlx::query(
                    "UPDATE uploads SET visibility = 'unlisted', status = ?, published_at = CASE WHEN ? = 'published' AND published_at IS NULL THEN ? ELSE published_at END WHERE id = ? AND creator_id = ?",
                )
                .bind(&next_status)
                .bind(&next_status)
                .bind(&now)
                .bind(upload_id)
                .bind(creator_id)
                .execute(&state.pool)
                .await?;
                sqlx::query(
                    "UPDATE media_assets SET visibility = 'unlisted', status = ?, updated_at = ? WHERE upload_id = ? AND creator_id = ?",
                )
                .bind(&next_status)
                .bind(&now)
                .bind(upload_id)
                .bind(creator_id)
                .execute(&state.pool)
                .await?;
                expire_playback_sessions_for_upload(&state.pool, upload_id).await?;
            }
            "delete" => {
                validate_bulk_upload_action(&current, "delete")?;
                let active_purchase_exists = sqlx::query(
                    "SELECT 1 FROM content_purchases WHERE upload_id = ? AND status = 'active' LIMIT 1",
                )
                .bind(upload_id)
                .fetch_optional(&state.pool)
                .await?
                .is_some();
                if active_purchase_exists {
                    return Err(AppError::BadRequest(
                        "cannot delete uploads with active purchases".to_string(),
                    ));
                }
                expire_playback_sessions_for_upload(&state.pool, upload_id).await?;
                sqlx::query("DELETE FROM media_assets WHERE upload_id = ? AND creator_id = ?")
                    .bind(upload_id)
                    .bind(creator_id)
                    .execute(&state.pool)
                    .await?;
                sqlx::query("DELETE FROM upload_jobs WHERE upload_id = ? AND creator_id = ?")
                    .bind(upload_id)
                    .bind(creator_id)
                    .execute(&state.pool)
                    .await?;
                sqlx::query("DELETE FROM uploads WHERE id = ? AND creator_id = ?")
                    .bind(upload_id)
                    .bind(creator_id)
                    .execute(&state.pool)
                    .await?;
            }
            other => {
                return Err(AppError::BadRequest(format!(
                    "unsupported bulk action: {other}"
                )));
            }
        }
    }

    Ok(Json(fetch_uploads(&state.pool, creator_id).await?))
}
