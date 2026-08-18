use super::*;

pub(crate) async fn reconcile_expired_user_entitlements(state: SharedState) -> AppResult<()> {
    reconcile_expired_user_entitlements_for_read(&state.pool, None).await
}

pub(crate) async fn reconcile_expired_user_entitlements_for_read(
    pool: &SqlitePool,
    user_filter: Option<&str>,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let expired_memberships = sqlx::query(
        r#"
        SELECT DISTINCT user_id, creator_id
        FROM creator_memberships
        WHERE status IN ('active', 'canceling')
          AND COALESCE(ends_at, renews_at) IS NOT NULL
          AND COALESCE(ends_at, renews_at) <= ?
          AND (? IS NULL OR user_id = ?)
        "#,
    )
    .bind(&now)
    .bind(user_filter)
    .bind(user_filter)
    .fetch_all(pool)
    .await?;
    let expired_purchases = sqlx::query(
        r#"
        SELECT DISTINCT user_id, creator_id, upload_id
        FROM content_purchases
        WHERE status = 'active'
          AND expires_at IS NOT NULL
          AND expires_at <= ?
          AND (? IS NULL OR user_id = ?)
        "#,
    )
    .bind(&now)
    .bind(user_filter)
    .bind(user_filter)
    .fetch_all(pool)
    .await?;

    sqlx::query(
        r#"
        UPDATE creator_memberships
        SET status = 'expired',
            ends_at = COALESCE(ends_at, renews_at, ?)
        WHERE status IN ('active', 'canceling')
          AND COALESCE(ends_at, renews_at) IS NOT NULL
          AND COALESCE(ends_at, renews_at) <= ?
          AND (? IS NULL OR user_id = ?)
        "#,
    )
    .bind(&now)
    .bind(&now)
    .bind(user_filter)
    .bind(user_filter)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        UPDATE content_purchases
        SET status = 'expired'
        WHERE status = 'active'
          AND expires_at IS NOT NULL
          AND expires_at <= ?
          AND (? IS NULL OR user_id = ?)
        "#,
    )
    .bind(&now)
    .bind(user_filter)
    .bind(user_filter)
    .execute(pool)
    .await?;

    for row in expired_memberships {
        let user_id: String = row.get("user_id");
        let creator_id: String = row.get("creator_id");
        reconcile_playback_sessions_for_user(pool, &user_id, Some(&creator_id), None).await?;
    }
    for row in expired_purchases {
        let user_id: String = row.get("user_id");
        let creator_id: String = row.get("creator_id");
        let upload_id: String = row.get("upload_id");
        reconcile_playback_sessions_for_user(pool, &user_id, Some(&creator_id), Some(&upload_id))
            .await?;
    }

    Ok(())
}
