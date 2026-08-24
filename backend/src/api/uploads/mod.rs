use super::*;

mod bulk;
mod content;
mod lifecycle;
mod listing;

pub(crate) use content::update_upload;
pub(crate) use lifecycle::{takedown_upload, unpublish_upload};

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/api/v1/creator/me/uploads", get(listing::list_uploads))
        .route(
            "/api/v1/creator/me/content",
            get(listing::get_creator_content),
        )
        .route("/api/v1/creator/me/uploads/:id", patch(update_upload))
        .route(
            "/api/v1/creator/me/uploads/:id/lifecycle",
            patch(lifecycle::update_upload_lifecycle),
        )
        .route(
            "/api/v1/creator/me/uploads/:id/unpublish",
            post(unpublish_upload),
        )
        .route(
            "/api/v1/creator/me/uploads/:id/takedown",
            post(takedown_upload),
        )
        .route("/api/v1/creator/me/uploads/bulk", post(bulk::bulk_uploads))
}

async fn ensure_creator_can_publish_paid_content_for_database(
    database: &crate::db::Database,
    creator_id: &str,
) -> AppResult<()> {
    if let Ok(pool) = database.try_postgres_adapter() {
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

        if row.get::<String, _>("onboarding_status") == "approved"
            && row.get::<String, _>("identity_status") == "verified"
            && row.get::<String, _>("tax_status") == "verified"
            && !monetization_blocked
        {
            return Ok(());
        }
        return Err(AppError::BadRequest(
            "creator is not cleared to publish paid content".to_string(),
        ));
    }

    ensure_creator_can_publish_paid_content(database.try_sqlite_adapter()?, creator_id).await
}

async fn validate_creator_access_tier_for_database(
    database: &crate::db::Database,
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

    if let Ok(pool) = database.try_postgres_adapter() {
        let row = sqlx::query(
            "SELECT status FROM creator_subscriber_tiers WHERE creator_id = $1 AND id = $2",
        )
        .bind(creator_id)
        .bind(tier_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
        if row.get::<String, _>("status") == "active" {
            return Ok(());
        }
        return Err(AppError::BadRequest(
            "subscription-based access requires an active subscriber tier".to_string(),
        ));
    }

    validate_creator_access_tier(
        database.try_sqlite_adapter()?,
        creator_id,
        access_policy,
        Some(tier_id),
    )
    .await
}

async fn sync_upload_media_asset_lifecycle(
    database: &crate::db::Database,
    creator_id: &str,
    upload_id: &str,
    visibility: &str,
    status: &str,
    now: &str,
) -> AppResult<()> {
    if let Ok(pool) = database.try_postgres_adapter() {
        sqlx::query(
            "UPDATE media_assets SET visibility = $1, status = $2, updated_at = $3 WHERE upload_id = $4 AND creator_id = $5",
        )
        .bind(visibility)
        .bind(status)
        .bind(now)
        .bind(upload_id)
        .bind(creator_id)
        .execute(pool)
        .await?;
        return Ok(());
    }

    sqlx::query(
        "UPDATE media_assets SET visibility = ?, status = ?, updated_at = ? WHERE upload_id = ? AND creator_id = ?",
    )
    .bind(visibility)
    .bind(status)
    .bind(now)
    .bind(upload_id)
    .bind(creator_id)
    .execute(database.try_sqlite_adapter()?)
    .await?;
    Ok(())
}

async fn expire_playback_sessions_for_upload_in_database(
    database: &crate::db::Database,
    upload_id: &str,
) -> AppResult<()> {
    if let Ok(pool) = database.try_postgres_adapter() {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE playback_sessions SET expires_at = $1, last_used_at = $2 WHERE content_id = $3 AND expires_at > $4",
        )
        .bind(&now)
        .bind(&now)
        .bind(upload_id)
        .bind(&now)
        .execute(pool)
        .await?;
        return Ok(());
    }

    expire_playback_sessions_for_upload(database.try_sqlite_adapter()?, upload_id).await
}

async fn enqueue_upload_lifecycle_notification(
    database: &crate::db::Database,
    kind: &str,
    body: &str,
    actor_user_id: &str,
    creator_id: &str,
    payload: Value,
) -> AppResult<()> {
    if let Ok(pool) = database.try_postgres_adapter() {
        let event_id = format!("notev-{}", Uuid::new_v4().simple());
        let delivery_id = format!("notd-{}", Uuid::new_v4().simple());
        let sent_at = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO notification_events (
                id, kind, body, actor_user_id, actor_label, creator_id, stream_id, amount, payload_json, created_at
            ) VALUES ($1, $2, $3, $4, 'creator', $5, NULL, NULL, $6, $7)
            "#,
        )
        .bind(&event_id)
        .bind(kind)
        .bind(body)
        .bind(actor_user_id)
        .bind(creator_id)
        .bind(to_json(&payload)?)
        .bind(&sent_at)
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO notification_deliveries (
                id, event_id, recipient_user_id, recipient_creator_id, channel, state, sent_at,
                delivered_at, read_at, failed_at, last_error, retry_count, last_attempted_at, next_attempt_at
            ) VALUES ($1, $2, NULL, $3, 'inbox', 'pending', $4, NULL, NULL, NULL, NULL, 0, NULL, $5)
            "#,
        )
        .bind(delivery_id)
        .bind(event_id)
        .bind(creator_id)
        .bind(&sent_at)
        .bind(&sent_at)
        .execute(pool)
        .await?;
        return Ok(());
    }

    enqueue_notification_event(
        database.try_sqlite_adapter()?,
        kind,
        body,
        Some(actor_user_id),
        Some("creator"),
        Some(creator_id),
        None,
        None,
        payload,
        &[],
        &[creator_id.to_string()],
    )
    .await
}
