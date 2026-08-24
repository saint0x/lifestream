use super::super::ingest::get_creator_upload_job;
use super::super::lifecycle::ensure_creator_upload_ingest_enabled_for_jobs;
use super::asset::get_creator_media_asset_for_upload_job;
use super::*;

pub(crate) async fn publish_upload_job(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<PublishUploadJobRequest>,
) -> AppResult<Json<Upload>> {
    let identity = require_identity(&state.db, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-upload-publish:{}", identity.user_id),
        30,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    ensure_creator_upload_ingest_enabled_for_jobs(&state.db, creator_id).await?;
    let job = get_creator_upload_job(&state.db, creator_id, &id).await?;
    if job.status != "ready" && job.status != "published" {
        return Err(AppError::BadRequest(
            "upload job must be ready before publish".to_string(),
        ));
    }

    let asset = get_creator_media_asset_for_upload_job(&state.db, creator_id, &id).await?;
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
        ensure_creator_can_publish_paid_content_for_upload(&state.db, creator_id).await?;
    }
    validate_creator_access_tier_for_upload(
        &state.db,
        creator_id,
        &access_terms.access_policy,
        access_terms.access_tier_id.as_deref(),
    )
    .await?;
    let upload_id = job
        .upload_id
        .clone()
        .unwrap_or_else(|| format!("upl-{}", Uuid::new_v4().simple()));
    let requested_slug = sanitize_slug(input.slug.as_deref().unwrap_or(&slugify(&asset.title)))?;
    let slug = allocate_upload_slug(
        &state.db,
        creator_id,
        Some(upload_id.as_str()),
        &requested_slug,
    )
    .await?;
    let now = Utc::now().to_rfc3339();
    let release_at = input.release_at.clone().unwrap_or_else(|| now.clone());
    let is_released = release_at <= now;
    let resolution = match (asset.width, asset.height) {
        (Some(width), Some(height)) => format!("{width}x{height}"),
        _ => "audio".to_string(),
    };
    let series_title = if let Some(series_id) = job.series_id.as_deref() {
        fetch_creator_series_title_for_upload(&state.db, creator_id, series_id).await?
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
        .unwrap_or_default();
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
        &state.db,
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
        ensure_creator_series_season_for_upload(
            &state.db,
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
        &state.db,
        creator_id,
        &id,
        &upload_id,
        &visibility,
        upload_status,
        &now,
    )
    .await?;
    enqueue_upload_release_notification(
        &state.db,
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
        fetch_published_upload(&state.db, creator_id, &upload_id).await?,
    ))
}

async fn ensure_creator_can_publish_paid_content_for_upload(
    database: &crate::db::Database,
    creator_id: &str,
) -> AppResult<()> {
    if let Ok(pool) = database.try_postgres_adapter() {
        return ensure_postgres_creator_can_publish_paid_content(pool, creator_id).await;
    }
    ensure_creator_can_publish_paid_content(database.try_sqlite_adapter()?, creator_id).await
}

async fn validate_creator_access_tier_for_upload(
    database: &crate::db::Database,
    creator_id: &str,
    access_policy: &str,
    access_tier_id: Option<&str>,
) -> AppResult<()> {
    if let Ok(pool) = database.try_postgres_adapter() {
        return validate_postgres_creator_access_tier(
            pool,
            creator_id,
            access_policy,
            access_tier_id,
        )
        .await;
    }
    validate_creator_access_tier(
        database.try_sqlite_adapter()?,
        creator_id,
        access_policy,
        access_tier_id,
    )
    .await
}

async fn fetch_creator_series_title_for_upload(
    database: &crate::db::Database,
    creator_id: &str,
    series_id: &str,
) -> AppResult<Option<String>> {
    if let Ok(pool) = database.try_postgres_adapter() {
        return fetch_postgres_creator_series_title(pool, creator_id, series_id).await;
    }
    fetch_creator_series_title(database.try_sqlite_adapter()?, creator_id, series_id).await
}

async fn ensure_postgres_creator_can_publish_paid_content(
    pool: &sqlx::PgPool,
    creator_id: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let row = sqlx::query(
        r#"
        SELECT onboarding_status, identity_status, tax_status
        FROM creator_operational_state
        WHERE creator_id = $1
        "#,
    )
    .bind(creator_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let monetization_blocked = sqlx::query(
        r#"
        SELECT 1
        FROM creator_enforcement_actions
        WHERE creator_id = $1
          AND scope = 'monetization'
          AND state = 'active'
          AND (expires_at IS NULL OR expires_at > $2)
        LIMIT 1
        "#,
    )
    .bind(creator_id)
    .bind(&now)
    .fetch_optional(pool)
    .await?
    .is_some();

    let can_publish_paid_content = row.get::<String, _>("onboarding_status") == "approved"
        && row.get::<String, _>("identity_status") == "verified"
        && row.get::<String, _>("tax_status") == "verified"
        && !monetization_blocked;

    if can_publish_paid_content {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "creator is not cleared to publish paid content".to_string(),
        ))
    }
}

async fn validate_postgres_creator_access_tier(
    pool: &sqlx::PgPool,
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
    let row = sqlx::query(
        r#"
        SELECT status
        FROM creator_subscriber_tiers
        WHERE creator_id = $1 AND id = $2
        "#,
    )
    .bind(creator_id)
    .bind(tier_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    if row.get::<String, _>("status") != "active" {
        return Err(AppError::BadRequest(
            "subscription-based access requires an active subscriber tier".to_string(),
        ));
    }
    Ok(())
}

async fn fetch_postgres_creator_series_title(
    pool: &sqlx::PgPool,
    creator_id: &str,
    series_id: &str,
) -> AppResult<Option<String>> {
    let row =
        sqlx::query("SELECT title FROM creator_series_projects WHERE creator_id = $1 AND id = $2")
            .bind(creator_id)
            .bind(series_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|row| row.get("title")))
}

async fn upsert_upload_record(
    database: &crate::db::Database,
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
    if let Ok(pool) = database.try_postgres_adapter() {
        return upsert_postgres_upload_record(
            pool,
            job,
            asset,
            input,
            creator_id,
            upload_id,
            slug,
            visibility,
            access_policy,
            access_tier_id,
            price_cents,
            currency,
            rental_window_hours,
            release_at,
            resolution,
            series_title,
            thumbnail,
            upload_status,
            published_at,
            now,
        )
        .await;
    }
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
    .execute(database.try_sqlite_adapter()?)
    .await
    .map_err(AppError::Database)?;
    Ok(())
}

async fn upsert_postgres_upload_record(
    pool: &sqlx::PgPool,
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
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29)
        ON CONFLICT(id) DO UPDATE SET
            creator_id = EXCLUDED.creator_id,
            slug = EXCLUDED.slug,
            title = EXCLUDED.title,
            description = EXCLUDED.description,
            kind = EXCLUDED.kind,
            published_at = EXCLUDED.published_at,
            release_at = EXCLUDED.release_at,
            status = EXCLUDED.status,
            visibility = EXCLUDED.visibility,
            access_policy = EXCLUDED.access_policy,
            access_tier_id = EXCLUDED.access_tier_id,
            price_cents = EXCLUDED.price_cents,
            currency = EXCLUDED.currency,
            rental_window_hours = EXCLUDED.rental_window_hours,
            thumbnail = EXCLUDED.thumbnail,
            series_title = EXCLUDED.series_title,
            season_number = EXCLUDED.season_number,
            episode_number = EXCLUDED.episode_number,
            size_bytes = EXCLUDED.size_bytes,
            resolution = EXCLUDED.resolution,
            transcode_progress = EXCLUDED.transcode_progress,
            series_id = EXCLUDED.series_id
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
    .await
    .map_err(AppError::Database)?;
    Ok(())
}

async fn allocate_upload_slug(
    database: &crate::db::Database,
    creator_id: &str,
    current_upload_id: Option<&str>,
    requested_slug: &str,
) -> AppResult<String> {
    if let Ok(pool) = database.try_postgres_adapter() {
        return allocate_postgres_upload_slug(pool, creator_id, current_upload_id, requested_slug)
            .await;
    }
    let existing_rows =
        sqlx::query("SELECT id, slug FROM uploads WHERE creator_id = ? AND slug IS NOT NULL")
            .bind(creator_id)
            .fetch_all(database.try_sqlite_adapter()?)
            .await?;
    let existing_slugs = existing_rows
        .into_iter()
        .filter_map(|row| {
            let upload_id: String = row.get("id");
            let slug: String = row.get("slug");
            if current_upload_id.is_some_and(|current| current == upload_id) {
                None
            } else {
                Some(slug)
            }
        })
        .collect::<std::collections::HashSet<_>>();

    if !existing_slugs.contains(requested_slug) {
        return Ok(requested_slug.to_string());
    }

    for suffix_number in 2..=10_000 {
        let candidate = build_upload_slug_candidate(requested_slug, suffix_number);
        if !existing_slugs.contains(&candidate) {
            return Ok(candidate);
        }
    }

    Err(AppError::Internal(
        "failed to allocate a unique upload slug".to_string(),
    ))
}

async fn allocate_postgres_upload_slug(
    pool: &sqlx::PgPool,
    creator_id: &str,
    current_upload_id: Option<&str>,
    requested_slug: &str,
) -> AppResult<String> {
    let existing_rows =
        sqlx::query("SELECT id, slug FROM uploads WHERE creator_id = $1 AND slug IS NOT NULL")
            .bind(creator_id)
            .fetch_all(pool)
            .await?;
    let existing_slugs = existing_rows
        .into_iter()
        .filter_map(|row| {
            let upload_id: String = row.get("id");
            let slug: String = row.get("slug");
            if current_upload_id.is_some_and(|current| current == upload_id) {
                None
            } else {
                Some(slug)
            }
        })
        .collect::<std::collections::HashSet<_>>();

    if !existing_slugs.contains(requested_slug) {
        return Ok(requested_slug.to_string());
    }

    for suffix_number in 2..=10_000 {
        let candidate = build_upload_slug_candidate(requested_slug, suffix_number);
        if !existing_slugs.contains(&candidate) {
            return Ok(candidate);
        }
    }

    Err(AppError::Internal(
        "failed to allocate a unique upload slug".to_string(),
    ))
}

async fn ensure_creator_series_season_for_upload(
    database: &crate::db::Database,
    creator_id: &str,
    series_id: &str,
    season_number: i64,
    title: String,
    synopsis: String,
) -> AppResult<()> {
    if let Ok(pool) = database.try_postgres_adapter() {
        return ensure_postgres_creator_series_season(
            pool,
            creator_id,
            series_id,
            season_number,
            title,
            synopsis,
        )
        .await;
    }
    ensure_creator_series_season(
        database.try_sqlite_adapter()?,
        creator_id,
        series_id,
        season_number,
        title,
        synopsis,
    )
    .await
}

async fn ensure_postgres_creator_series_season(
    pool: &sqlx::PgPool,
    creator_id: &str,
    series_id: &str,
    season_number: i64,
    title: String,
    synopsis: String,
) -> AppResult<()> {
    let season_id = format!("season-{}", Uuid::new_v4().simple());
    sqlx::query(
        r#"
        INSERT INTO creator_series_seasons (
            id, creator_id, series_id, season_number, title, synopsis, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $7)
        ON CONFLICT(series_id, season_number) DO UPDATE SET
            title = EXCLUDED.title,
            synopsis = EXCLUDED.synopsis,
            updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(season_id)
    .bind(creator_id)
    .bind(series_id)
    .bind(season_number)
    .bind(title)
    .bind(synopsis)
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

fn build_upload_slug_candidate(requested_slug: &str, suffix_number: usize) -> String {
    let suffix = format!("-{suffix_number}");
    let base_limit = 120usize.saturating_sub(suffix.len());
    let mut candidate = requested_slug.chars().take(base_limit).collect::<String>();
    candidate = candidate.trim_matches('-').to_string();
    if candidate.is_empty() {
        candidate = requested_slug.to_string();
    }
    candidate.push_str(&suffix);
    candidate
}

async fn update_upload_publish_state(
    database: &crate::db::Database,
    creator_id: &str,
    job_id: &str,
    upload_id: &str,
    visibility: &str,
    upload_status: &str,
    now: &str,
) -> AppResult<()> {
    if let Ok(pool) = database.try_postgres_adapter() {
        return update_postgres_upload_publish_state(
            pool,
            creator_id,
            job_id,
            upload_id,
            visibility,
            upload_status,
            now,
        )
        .await;
    }
    sqlx::query(
        "UPDATE upload_jobs SET status = 'published', upload_id = ?, published_content_id = ?, updated_at = ? WHERE id = ? AND creator_id = ?",
    )
    .bind(upload_id)
    .bind(upload_id)
    .bind(now)
    .bind(job_id)
    .bind(creator_id)
    .execute(database.try_sqlite_adapter()?)
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
    .execute(database.try_sqlite_adapter()?)
    .await?;
    Ok(())
}

async fn update_postgres_upload_publish_state(
    pool: &sqlx::PgPool,
    creator_id: &str,
    job_id: &str,
    upload_id: &str,
    visibility: &str,
    upload_status: &str,
    now: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE upload_jobs SET status = 'published', upload_id = $1, published_content_id = $2, updated_at = $3 WHERE id = $4 AND creator_id = $5",
    )
    .bind(upload_id)
    .bind(upload_id)
    .bind(now)
    .bind(job_id)
    .bind(creator_id)
    .execute(pool)
    .await?;

    sqlx::query(
        "UPDATE media_assets SET status = $1, visibility = $2, upload_id = $3, published_content_id = $4, updated_at = $5 WHERE upload_job_id = $6 AND creator_id = $7",
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

async fn fetch_published_upload(
    database: &crate::db::Database,
    creator_id: &str,
    upload_id: &str,
) -> AppResult<Upload> {
    if let Ok(pool) = database.try_postgres_adapter() {
        return fetch_postgres_published_upload(pool, creator_id, upload_id).await;
    }
    fetch_upload_by_id(database.try_sqlite_adapter()?, creator_id, upload_id).await
}

async fn fetch_postgres_published_upload(
    pool: &sqlx::PgPool,
    creator_id: &str,
    upload_id: &str,
) -> AppResult<Upload> {
    let row = sqlx::query(
        r#"
        SELECT id, slug, title, description, kind, duration_sec::BIGINT AS duration_sec,
               uploaded_at, published_at, release_at, status, visibility, access_policy,
               access_tier_id, price_cents::BIGINT AS price_cents, currency,
               rental_window_hours::BIGINT AS rental_window_hours,
               views::BIGINT AS views, likes::BIGINT AS likes, comments::BIGINT AS comments,
               watch_hours::BIGINT AS watch_hours, thumbnail, series_title,
               season_number::BIGINT AS season_number, episode_number::BIGINT AS episode_number,
               size_bytes::BIGINT AS size_bytes, resolution,
               transcode_progress::DOUBLE PRECISION AS transcode_progress
        FROM uploads
        WHERE creator_id = $1 AND id = $2
        "#,
    )
    .bind(creator_id)
    .bind(upload_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(postgres_upload_from_row(row))
}

fn postgres_upload_from_row(row: sqlx::postgres::PgRow) -> Upload {
    Upload {
        id: row.get("id"),
        slug: row.get("slug"),
        title: row.get("title"),
        description: row.get("description"),
        kind: row.get("kind"),
        duration_sec: row.get("duration_sec"),
        uploaded_at: row.get("uploaded_at"),
        published_at: row.get("published_at"),
        release_at: row.get("release_at"),
        status: row.get("status"),
        visibility: row.get("visibility"),
        access_policy: row.get("access_policy"),
        access_tier_id: row.get("access_tier_id"),
        price_cents: row.get("price_cents"),
        currency: row.get("currency"),
        rental_window_hours: row.get("rental_window_hours"),
        views: row.get("views"),
        likes: row.get("likes"),
        comments: row.get("comments"),
        watch_hours: row.get("watch_hours"),
        thumbnail: row.get("thumbnail"),
        series_title: row.get("series_title"),
        season_number: row.get("season_number"),
        episode_number: row.get("episode_number"),
        size_bytes: row.get("size_bytes"),
        resolution: row.get("resolution"),
        transcode_progress: row.get("transcode_progress"),
    }
}

async fn enqueue_upload_release_notification(
    database: &crate::db::Database,
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
    if let Ok(pool) = database.try_postgres_adapter() {
        return enqueue_postgres_upload_release_notification(
            pool,
            user_id,
            creator_id,
            asset_title,
            upload_status,
            upload_id,
            job_id,
            release_at,
            visibility,
            slug,
        )
        .await;
    }
    let creator_profile = fetch_creator_profile(database.try_sqlite_adapter()?, creator_id).await?;
    enqueue_notification_event(
        database.try_sqlite_adapter()?,
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

async fn enqueue_postgres_upload_release_notification(
    pool: &sqlx::PgPool,
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
    let creator_row = sqlx::query("SELECT display_name FROM creator_profiles WHERE id = $1")
        .bind(creator_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    let creator_display_name: String = creator_row.get("display_name");
    let event_id = format!("notev-{}", Uuid::new_v4().simple());
    let delivery_id = format!("notd-{}", Uuid::new_v4().simple());
    let sent_at = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO notification_events (
            id, kind, body, actor_user_id, actor_label, creator_id, stream_id, amount, payload_json, created_at
        ) VALUES ($1, 'content_release', $2, $3, $4, $5, NULL, NULL, $6, $7)
        "#,
    )
    .bind(&event_id)
    .bind(format!("{asset_title} is now {upload_status}."))
    .bind(user_id)
    .bind(&creator_display_name)
    .bind(creator_id)
    .bind(to_json(&json!({
        "uploadId": upload_id,
        "jobId": job_id,
        "status": upload_status,
        "releaseAt": release_at,
        "visibility": visibility,
        "slug": slug,
    }))?)
    .bind(&sent_at)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO notification_deliveries (
            id, event_id, recipient_user_id, recipient_creator_id, channel, state, sent_at, read_at,
            failed_at, last_error, retry_count
        ) VALUES ($1, $2, NULL, $3, 'inbox', 'pending', $4, NULL, NULL, NULL, 0)
        "#,
    )
    .bind(delivery_id)
    .bind(event_id)
    .bind(creator_id)
    .bind(sent_at)
    .execute(pool)
    .await?;
    Ok(())
}
