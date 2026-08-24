use super::*;

pub(crate) async fn fetch_creator_membership(
    pool: &SqlitePool,
    user_id: &str,
    creator_id: &str,
) -> AppResult<CreatorMembership> {
    let row = sqlx::query(
        r#"
        SELECT cms.creator_id, cp.handle, cp.display_name, cms.tier_id, cst.tier_name, cst.rank,
               cms.status, cms.started_at, cms.renews_at, cms.ends_at
        FROM creator_memberships cms
        JOIN creator_profiles cp ON cp.id = cms.creator_id
        JOIN creator_subscriber_tiers cst ON cst.id = cms.tier_id
        WHERE cms.user_id = ? AND cms.creator_id = ?
        "#,
    )
    .bind(user_id)
    .bind(creator_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(CreatorMembership {
        creator_id: row.get("creator_id"),
        creator_handle: row.get("handle"),
        creator_display_name: row.get("display_name"),
        tier_id: row.get("tier_id"),
        tier_name: row.get("tier_name"),
        tier_rank: row.get("rank"),
        status: row.get("status"),
        started_at: row.get("started_at"),
        renews_at: row.get("renews_at"),
        ends_at: row.get("ends_at"),
    })
}

pub(crate) async fn fetch_content_purchase_by_id(
    pool: &SqlitePool,
    purchase_id: &str,
) -> AppResult<ContentPurchase> {
    let row = sqlx::query(
        r#"
        SELECT p.id, p.creator_id, cp.handle, cp.display_name, p.upload_id, u.title,
               p.access_policy, p.amount_cents, p.currency, p.status, p.purchased_at, p.expires_at
        FROM content_purchases p
        JOIN creator_profiles cp ON cp.id = p.creator_id
        JOIN uploads u ON u.id = p.upload_id
        WHERE p.id = ?
        "#,
    )
    .bind(purchase_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(ContentPurchase {
        id: row.get("id"),
        creator_id: row.get("creator_id"),
        creator_handle: row.get("handle"),
        creator_display_name: row.get("display_name"),
        upload_id: row.get("upload_id"),
        title: row.get("title"),
        access_policy: row.get("access_policy"),
        amount_cents: row.get("amount_cents"),
        currency: row.get("currency"),
        status: row.get("status"),
        purchased_at: row.get("purchased_at"),
        expires_at: row.get("expires_at"),
    })
}

pub(crate) async fn purchase_belongs_to_user(
    pool: &SqlitePool,
    user_id: &str,
    purchase_id: &str,
) -> AppResult<bool> {
    let row = sqlx::query("SELECT 1 FROM content_purchases WHERE id = ? AND user_id = ?")
        .bind(purchase_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.is_some())
}

pub(crate) async fn fetch_current_content_purchase(
    pool: &SqlitePool,
    user_id: &str,
    upload_id: &str,
) -> AppResult<Option<ContentPurchase>> {
    let now = Utc::now().to_rfc3339();
    let row = sqlx::query(
        r#"
        SELECT id
        FROM content_purchases
        WHERE user_id = ?
          AND upload_id = ?
          AND status = 'active'
          AND (expires_at IS NULL OR expires_at > ?)
        ORDER BY purchased_at DESC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(upload_id)
    .bind(&now)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(row) => {
            let purchase_id: String = row.get("id");
            fetch_content_purchase_by_id(pool, &purchase_id)
                .await
                .map(Some)
        }
        None => Ok(None),
    }
}

pub(crate) async fn fetch_user_entitlements(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<UserEntitlements> {
    reconcile_expired_user_entitlements_for_read(pool, Some(user_id)).await?;
    let membership_rows = sqlx::query(
        r#"
        SELECT cms.creator_id, cp.handle, cp.display_name, cms.tier_id, cst.tier_name, cst.rank,
               cms.status, cms.started_at, cms.renews_at, cms.ends_at
        FROM creator_memberships cms
        JOIN creator_profiles cp ON cp.id = cms.creator_id
        JOIN creator_subscriber_tiers cst ON cst.id = cms.tier_id
        WHERE cms.user_id = ?
        ORDER BY cms.started_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let purchase_rows = sqlx::query(
        r#"
        SELECT p.id, p.creator_id, cp.handle, cp.display_name, p.upload_id, u.title,
               p.access_policy, p.amount_cents, p.currency, p.status, p.purchased_at, p.expires_at
        FROM content_purchases p
        JOIN creator_profiles cp ON cp.id = p.creator_id
        JOIN uploads u ON u.id = p.upload_id
        WHERE p.user_id = ?
        ORDER BY p.purchased_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(UserEntitlements {
        memberships: membership_rows
            .into_iter()
            .map(|row| CreatorMembership {
                creator_id: row.get("creator_id"),
                creator_handle: row.get("handle"),
                creator_display_name: row.get("display_name"),
                tier_id: row.get("tier_id"),
                tier_name: row.get("tier_name"),
                tier_rank: row.get("rank"),
                status: row.get("status"),
                started_at: row.get("started_at"),
                renews_at: row.get("renews_at"),
                ends_at: row.get("ends_at"),
            })
            .collect(),
        purchases: purchase_rows
            .into_iter()
            .map(|row| ContentPurchase {
                id: row.get("id"),
                creator_id: row.get("creator_id"),
                creator_handle: row.get("handle"),
                creator_display_name: row.get("display_name"),
                upload_id: row.get("upload_id"),
                title: row.get("title"),
                access_policy: row.get("access_policy"),
                amount_cents: row.get("amount_cents"),
                currency: row.get("currency"),
                status: row.get("status"),
                purchased_at: row.get("purchased_at"),
                expires_at: row.get("expires_at"),
            })
            .collect(),
    })
}

pub(crate) async fn reconcile_single_membership_entitlement(
    state: SharedState,
    user_id: &str,
    creator_id: &str,
) -> AppResult<CreatorMembershipReconciliationReport> {
    let before = fetch_creator_membership(state.db.sqlite_adapter(), user_id, creator_id).await?;
    let now = Utc::now().to_rfc3339();
    let mut actions = Vec::new();

    if matches!(before.status.as_str(), "active" | "canceling")
        && before
            .ends_at
            .as_deref()
            .or(before.renews_at.as_deref())
            .is_some_and(|expires_at| expires_at <= now.as_str())
    {
        let updated = sqlx::query(
            r#"
            UPDATE creator_memberships
            SET status = 'expired',
                ends_at = COALESCE(ends_at, renews_at, ?)
            WHERE user_id = ? AND creator_id = ?
              AND status IN ('active', 'canceling')
              AND COALESCE(ends_at, renews_at) IS NOT NULL
              AND COALESCE(ends_at, renews_at) <= ?
            "#,
        )
        .bind(&now)
        .bind(user_id)
        .bind(creator_id)
        .bind(&now)
        .execute(state.db.sqlite_adapter())
        .await?;
        if updated.rows_affected() > 0 {
            actions.push(UserEntitlementReconciliationAction {
                action_type: "membership_expired".to_string(),
                target_id: creator_id.to_string(),
                previous_state: Some(before.status.clone()),
                next_state: Some("expired".to_string()),
                reason: "creator membership exceeded its renewal or end boundary".to_string(),
                occurred_at: now.clone(),
            });
            reconcile_playback_sessions_for_user(
                state.db.sqlite_adapter(),
                user_id,
                Some(creator_id),
                None,
            )
            .await?;
        }
    }

    let membership =
        fetch_creator_membership(state.db.sqlite_adapter(), user_id, creator_id).await?;
    Ok(CreatorMembershipReconciliationReport {
        creator_id: creator_id.to_string(),
        user_id: user_id.to_string(),
        reconciled_at: now,
        actions,
        membership,
    })
}

pub(crate) async fn reconcile_single_purchase_entitlement(
    state: SharedState,
    user_id: &str,
    purchase_id: &str,
) -> AppResult<ContentPurchaseReconciliationReport> {
    let before = fetch_content_purchase_by_id(state.db.sqlite_adapter(), purchase_id).await?;
    let now = Utc::now().to_rfc3339();
    let mut actions = Vec::new();

    if before.status == "active"
        && before
            .expires_at
            .as_deref()
            .is_some_and(|expires_at| expires_at <= now.as_str())
    {
        let updated = sqlx::query(
            r#"
            UPDATE content_purchases
            SET status = 'expired'
            WHERE id = ? AND user_id = ? AND status = 'active'
              AND expires_at IS NOT NULL
              AND expires_at <= ?
            "#,
        )
        .bind(purchase_id)
        .bind(user_id)
        .bind(&now)
        .execute(state.db.sqlite_adapter())
        .await?;
        if updated.rows_affected() > 0 {
            actions.push(UserEntitlementReconciliationAction {
                action_type: "purchase_expired".to_string(),
                target_id: purchase_id.to_string(),
                previous_state: Some("active".to_string()),
                next_state: Some("expired".to_string()),
                reason: "content purchase exceeded its expiry boundary".to_string(),
                occurred_at: now.clone(),
            });
            reconcile_playback_sessions_for_user(
                state.db.sqlite_adapter(),
                user_id,
                Some(&before.creator_id),
                Some(&before.upload_id),
            )
            .await?;
        }
    }

    let purchase = fetch_content_purchase_by_id(state.db.sqlite_adapter(), purchase_id).await?;
    Ok(ContentPurchaseReconciliationReport {
        purchase_id: purchase_id.to_string(),
        user_id: user_id.to_string(),
        reconciled_at: now,
        actions,
        purchase,
    })
}
