use super::*;

pub(crate) async fn publish_upload_job(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<PublishUploadJobRequest>,
) -> AppResult<Json<Upload>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-upload-publish:{}", identity.user_id),
        30,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    ensure_creator_upload_ingest_enabled(&state.pool, creator_id).await?;
    let job = fetch_upload_job_by_id(&state.pool, creator_id, &id).await?;
    if job.status != "ready" && job.status != "published" {
        return Err(AppError::BadRequest(
            "upload job must be ready before publish".to_string(),
        ));
    }

    let asset = fetch_media_asset_by_upload_job(&state.pool, creator_id, &id).await?;
    if asset.playback_path.is_none() {
        return Err(AppError::BadRequest(
            "media asset does not yet have a playback manifest".to_string(),
        ));
    }

    let visibility = input
        .visibility
        .clone()
        .unwrap_or_else(|| job.intended_visibility.clone());
    let access_terms = resolve_upload_access_terms(
        input.access_policy.clone(),
        input.access_tier_id.clone(),
        input.price_cents,
        input.currency.clone(),
        input.rental_window_hours,
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
    let slug = sanitize_slug(input.slug.as_deref().unwrap_or(&slugify(&asset.title)))?;
    let upload_id = job
        .upload_id
        .clone()
        .unwrap_or_else(|| format!("upl-{}", Uuid::new_v4().simple()));
    let now = Utc::now().to_rfc3339();
    let release_at = input.release_at.clone().unwrap_or_else(|| now.clone());
    let is_released = release_at <= now;
    let resolution = match (asset.width, asset.height) {
        (Some(width), Some(height)) => format!("{width}x{height}"),
        _ => "audio".to_string(),
    };
    let series_title = if let Some(series_id) = job.series_id.as_deref() {
        fetch_creator_series_title(&state.pool, creator_id, series_id).await?
    } else {
        None
    };
    let thumbnail = asset
        .variants
        .iter()
        .find(|variant| {
            variant.variant_type == "thumbnail"
                && (variant.label == "card_thumbnail" || variant.is_default)
        })
        .map(|variant| variant.url.clone())
        .or_else(|| asset.poster_url.clone())
        .unwrap_or_else(|| "https://cdn.lifestream.local/thumb/upload-default.jpg".to_string());
    let upload_status = if is_released && (visibility == "public" || visibility == "unlisted") {
        "published"
    } else if !is_released {
        "scheduled"
    } else {
        "draft"
    };
    let published_at = if upload_status == "published" {
        Some(now.clone())
    } else {
        None
    };

    upsert_upload_record(
        &state.pool,
        &job,
        &asset,
        &input,
        creator_id,
        &upload_id,
        &slug,
        &visibility,
        &access_terms.access_policy,
        access_terms.access_tier_id.as_deref(),
        access_terms.price_cents,
        access_terms.currency.as_deref(),
        access_terms.rental_window_hours,
        &release_at,
        &resolution,
        series_title,
        &thumbnail,
        upload_status,
        published_at.clone(),
        &now,
    )
    .await?;

    if let (Some(series_id), Some(season_number)) = (job.series_id.as_deref(), input.season_number)
    {
        ensure_creator_series_season(
            &state.pool,
            creator_id,
            series_id,
            season_number,
            input
                .season_title
                .clone()
                .unwrap_or_else(|| format!("Season {season_number}")),
            input.season_synopsis.clone().unwrap_or_default(),
        )
        .await?;
    }

    update_upload_publish_state(
        &state.pool,
        creator_id,
        &id,
        &upload_id,
        &visibility,
        upload_status,
        &now,
    )
    .await?;
    enqueue_upload_release_notification(
        &state,
        &identity.user_id,
        creator_id,
        &asset.title,
        upload_status,
        &upload_id,
        &id,
        &release_at,
        &visibility,
        &slug,
    )
    .await?;

    Ok(Json(
        fetch_upload_by_id(&state.pool, creator_id, &upload_id).await?,
    ))
}

async fn upsert_upload_record(
    pool: &SqlitePool,
    job: &UploadJob,
    asset: &MediaAsset,
    input: &PublishUploadJobRequest,
    creator_id: &str,
    upload_id: &str,
    slug: &str,
    visibility: &str,
    access_policy: &str,
    access_tier_id: Option<&str>,
    price_cents: Option<i64>,
    currency: Option<&str>,
    rental_window_hours: Option<i64>,
    release_at: &str,
    resolution: &str,
    series_title: Option<String>,
    thumbnail: &str,
    upload_status: &str,
    published_at: Option<String>,
    now: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO uploads (
            id, creator_id, slug, title, description, kind, duration_sec, uploaded_at, published_at, release_at, status,
            visibility, access_policy, access_tier_id, price_cents, currency, rental_window_hours,
            views, likes, comments, watch_hours, thumbnail, series_title,
            season_number, episode_number, size_bytes, resolution, transcode_progress, series_id
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            creator_id = excluded.creator_id,
            slug = excluded.slug,
            title = excluded.title,
            description = excluded.description,
            kind = excluded.kind,
            published_at = excluded.published_at,
            release_at = excluded.release_at,
            status = excluded.status,
            visibility = excluded.visibility,
            access_policy = excluded.access_policy,
            access_tier_id = excluded.access_tier_id,
            price_cents = excluded.price_cents,
            currency = excluded.currency,
            rental_window_hours = excluded.rental_window_hours,
            thumbnail = excluded.thumbnail,
            series_title = excluded.series_title,
            season_number = excluded.season_number,
            episode_number = excluded.episode_number,
            size_bytes = excluded.size_bytes,
            resolution = excluded.resolution,
            transcode_progress = excluded.transcode_progress,
            series_id = excluded.series_id
        "#,
    )
    .bind(upload_id)
    .bind(creator_id)
    .bind(slug)
    .bind(&asset.title)
    .bind(input.description.clone().unwrap_or_default())
    .bind(&asset.kind)
    .bind(asset.duration_sec.round() as i64)
    .bind(job.completed_at.clone().unwrap_or_else(|| now.to_string()))
    .bind(published_at)
    .bind(release_at)
    .bind(upload_status)
    .bind(visibility)
    .bind(access_policy)
    .bind(access_tier_id)
    .bind(price_cents)
    .bind(currency)
    .bind(rental_window_hours)
    .bind(0_i64)
    .bind(0_i64)
    .bind(0_i64)
    .bind(0_i64)
    .bind(thumbnail)
    .bind(series_title)
    .bind(input.season_number)
    .bind(input.episode_number)
    .bind(asset.file_size_bytes)
    .bind(resolution)
    .bind(1.0_f64)
    .bind(job.series_id.clone())
    .execute(pool)
    .await?;
    Ok(())
}

async fn update_upload_publish_state(
    pool: &SqlitePool,
    creator_id: &str,
    job_id: &str,
    upload_id: &str,
    visibility: &str,
    upload_status: &str,
    now: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE upload_jobs SET status = 'published', upload_id = ?, published_content_id = ?, updated_at = ? WHERE id = ? AND creator_id = ?",
    )
    .bind(upload_id)
    .bind(upload_id)
    .bind(now)
    .bind(job_id)
    .bind(creator_id)
    .execute(pool)
    .await?;

    sqlx::query(
        "UPDATE media_assets SET status = ?, visibility = ?, upload_id = ?, published_content_id = ?, updated_at = ? WHERE upload_job_id = ? AND creator_id = ?",
    )
    .bind(upload_status)
    .bind(visibility)
    .bind(upload_id)
    .bind(upload_id)
    .bind(now)
    .bind(job_id)
    .bind(creator_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn enqueue_upload_release_notification(
    state: &SharedState,
    user_id: &str,
    creator_id: &str,
    asset_title: &str,
    upload_status: &str,
    upload_id: &str,
    job_id: &str,
    release_at: &str,
    visibility: &str,
    slug: &str,
) -> AppResult<()> {
    let creator_profile = fetch_creator_profile(&state.pool, creator_id).await?;
    enqueue_notification_event(
        &state.pool,
        "content_release",
        &format!("{asset_title} is now {upload_status}."),
        Some(user_id),
        Some(&creator_profile.display_name),
        Some(creator_id),
        None,
        None,
        json!({
            "uploadId": upload_id,
            "jobId": job_id,
            "status": upload_status,
            "releaseAt": release_at,
            "visibility": visibility,
            "slug": slug,
        }),
        &[],
        &[creator_id.to_string()],
    )
    .await
}
