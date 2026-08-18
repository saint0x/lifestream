use super::*;

pub(super) async fn reconcile_expired_collaboration_invites(state: SharedState) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT session_id
        FROM collaboration_invites
        WHERE state = 'pending' AND expires_at <= ?
        "#,
    )
    .bind(&now)
    .fetch_all(&state.pool)
    .await?;

    for row in rows {
        let session_id: String = row.get("session_id");
        let _ = expire_pending_collaboration_invites_for_session(&state, &session_id, &now).await?;
    }

    Ok(())
}

pub(super) async fn reconcile_expired_collaboration_mirror_grants(
    state: SharedState,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT session_id
        FROM collaboration_mirror_grants
        WHERE state IN ('issued', 'active') AND expires_at <= ?
        "#,
    )
    .bind(&now)
    .fetch_all(&state.pool)
    .await?;

    for row in rows {
        let session_id: String = row.get("session_id");
        let _ = expire_collaboration_mirror_grants_for_session(&state, &session_id, &now).await?;
    }

    Ok(())
}

pub(super) async fn reconcile_expired_user_entitlements(state: SharedState) -> AppResult<()> {
    reconcile_expired_user_entitlements_for_read(&state.pool, None).await
}

pub(super) async fn reconcile_expired_user_entitlements_for_read(
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

pub(super) async fn reconcile_expired_live_moderation_actions(state: SharedState) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"
        SELECT id, stream_id, creator_id, subject_user_id, actor_user_id, action_type, reason, state,
               expires_at, created_at, revoked_at
        FROM live_moderation_actions
        WHERE state = 'active' AND expires_at IS NOT NULL AND expires_at <= ?
        "#,
    )
    .bind(&now)
    .fetch_all(&state.pool)
    .await?;

    for row in rows {
        let action = live_moderation_action_from_row(row);
        sqlx::query(
            "UPDATE live_moderation_actions SET state = 'expired', revoked_at = COALESCE(revoked_at, ?) WHERE id = ?",
        )
        .bind(&now)
        .bind(&action.id)
        .execute(&state.pool)
        .await?;
        let _ = write_moderation_audit_entry(
            &state.pool,
            &action.creator_id,
            Some(&action.stream_id),
            &action.actor_user_id,
            Some(&action.subject_user_id),
            "moderation_action_expired",
            json!({
                "actionId": action.id,
                "actionType": action.action_type,
                "expiredAt": now,
            }),
        )
        .await;
        let expired = fetch_live_moderation_action_by_id(&state.pool, &action.id).await?;
        state
            .realtime
            .publish(
                &stream_channel_id(&action.stream_id),
                WsEvent::ModerationAction { action: expired },
            )
            .await;
    }

    Ok(())
}

pub(super) async fn reconcile_expired_live_moderation_actions_for_read(
    pool: &SqlitePool,
    stream_filter: Option<&str>,
    creator_filter: Option<&str>,
    subject_filter: Option<&str>,
    action_filter: Option<&str>,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"
        SELECT id, stream_id, creator_id, subject_user_id, actor_user_id, action_type, reason, state,
               expires_at, created_at, revoked_at
        FROM live_moderation_actions
        WHERE state = 'active'
          AND expires_at IS NOT NULL
          AND expires_at <= ?
          AND (?2 IS NULL OR stream_id = ?2)
          AND (?3 IS NULL OR creator_id = ?3)
          AND (?4 IS NULL OR subject_user_id = ?4)
          AND (?5 IS NULL OR id = ?5)
        ORDER BY expires_at ASC
        "#,
    )
    .bind(&now)
    .bind(stream_filter)
    .bind(creator_filter)
    .bind(subject_filter)
    .bind(action_filter)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let action = live_moderation_action_from_row(row);
        sqlx::query(
            "UPDATE live_moderation_actions SET state = 'expired', revoked_at = COALESCE(revoked_at, ?) WHERE id = ? AND state = 'active'",
        )
        .bind(&now)
        .bind(&action.id)
        .execute(pool)
        .await?;
        write_moderation_audit_entry(
            pool,
            &action.creator_id,
            Some(&action.stream_id),
            &action.actor_user_id,
            Some(&action.subject_user_id),
            "moderation_action_expired",
            json!({
                "actionId": action.id,
                "actionType": action.action_type,
                "expiredAt": now,
            }),
        )
        .await?;
    }

    Ok(())
}

pub(super) async fn reconcile_single_live_moderation_action(
    state: SharedState,
    action_id: &str,
) -> AppResult<LiveModerationReconciliationReport> {
    let before = fetch_live_moderation_action_by_id_raw(&state.pool, action_id).await?;
    let now = Utc::now().to_rfc3339();
    let mut actions = Vec::new();

    if before.state == "active"
        && before
            .expires_at
            .as_deref()
            .is_some_and(|expires_at| expires_at <= now.as_str())
    {
        let updated = sqlx::query(
            "UPDATE live_moderation_actions SET state = 'expired', revoked_at = COALESCE(revoked_at, ?) WHERE id = ? AND state = 'active'",
        )
        .bind(&now)
        .bind(action_id)
        .execute(&state.pool)
        .await?;
        if updated.rows_affected() > 0 {
            write_moderation_audit_entry(
                &state.pool,
                &before.creator_id,
                Some(&before.stream_id),
                &before.actor_user_id,
                Some(&before.subject_user_id),
                "moderation_action_expired",
                json!({
                    "actionId": before.id,
                    "actionType": before.action_type,
                    "expiredAt": now,
                }),
            )
            .await?;
            actions.push(LiveModerationReconciliationAction {
                action_type: "action_expired".to_string(),
                target_id: before.id.clone(),
                previous_state: Some("active".to_string()),
                next_state: Some("expired".to_string()),
                reason: "live moderation action exceeded its expiry window".to_string(),
                occurred_at: now.clone(),
            });
        }
    }

    let action = fetch_live_moderation_action_by_id_raw(&state.pool, action_id).await?;
    if !actions.is_empty() {
        state
            .realtime
            .publish(
                &stream_channel_id(&action.stream_id),
                WsEvent::ModerationAction {
                    action: action.clone(),
                },
            )
            .await;
    }
    Ok(LiveModerationReconciliationReport {
        action_id: action_id.to_string(),
        reconciled_at: now,
        actions,
        action,
    })
}

pub(super) async fn reconcile_expired_creator_enforcement_actions(
    state: SharedState,
) -> AppResult<()> {
    reconcile_expired_creator_enforcement_actions_for_read(&state.pool, None, None).await
}

pub(super) async fn reconcile_expired_creator_enforcement_actions_for_read(
    pool: &SqlitePool,
    creator_filter: Option<&str>,
    action_filter: Option<&str>,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"
        SELECT id, creator_id, scope, state, reason, resolution_note, created_by_user_id,
               released_by_user_id, created_at, released_at, expires_at
        FROM creator_enforcement_actions
        WHERE state = 'active'
          AND expires_at IS NOT NULL
          AND expires_at <= ?
          AND (?2 IS NULL OR creator_id = ?2)
          AND (?3 IS NULL OR id = ?3)
        "#,
    )
    .bind(&now)
    .bind(creator_filter)
    .bind(action_filter)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let action = creator_enforcement_action_from_row(row);
        let updated = sqlx::query(
            "UPDATE creator_enforcement_actions SET state = 'expired', released_at = COALESCE(released_at, ?) WHERE id = ? AND state = 'active'",
        )
        .bind(&now)
        .bind(&action.id)
        .execute(pool)
        .await?;
        if updated.rows_affected() == 0 {
            continue;
        }
        let profile = fetch_creator_profile(pool, &action.creator_id).await?;
        let _ = write_moderation_audit_entry(
            pool,
            &action.creator_id,
            None,
            &action.created_by_user_id,
            Some(&profile.user_id),
            "creator_enforcement_expired",
            json!({
                "actionId": action.id,
                "scope": action.scope,
                "expiredAt": now,
            }),
        )
        .await;
        let _ = enqueue_notification_event(
            pool,
            "creator_enforcement_expired",
            &format!(
                "An enforcement restriction for scope '{}' has expired.",
                action.scope
            ),
            None,
            Some("system"),
            Some(&action.creator_id),
            None,
            None,
            json!({
                "actionId": action.id,
                "scope": action.scope,
                "expiredAt": now,
            }),
            &[],
            std::slice::from_ref(&action.creator_id),
        )
        .await;
    }

    Ok(())
}

pub(super) async fn reconcile_notification_deliveries(state: SharedState) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"
        SELECT id
        FROM notification_deliveries
        WHERE state IN ('pending', 'retrying')
          AND COALESCE(next_attempt_at, sent_at) <= ?
        ORDER BY COALESCE(next_attempt_at, sent_at) ASC
        LIMIT 100
        "#,
    )
    .bind(&now)
    .fetch_all(&state.pool)
    .await?;

    for row in rows {
        let delivery_id: String = row.get("id");
        let _ = dispatch_notification_delivery(&state.pool, &delivery_id).await?;
    }

    Ok(())
}

pub(super) async fn reconcile_single_creator_enforcement_action(
    state: SharedState,
    action_id: &str,
) -> AppResult<CreatorEnforcementReconciliationReport> {
    let before = fetch_creator_enforcement_action_by_id_raw(&state.pool, action_id).await?;
    let now = Utc::now().to_rfc3339();
    let mut actions = Vec::new();

    if before.state == "active"
        && before
            .expires_at
            .as_deref()
            .is_some_and(|expires_at| expires_at <= now.as_str())
    {
        sqlx::query(
            "UPDATE creator_enforcement_actions SET state = 'expired', released_at = COALESCE(released_at, ?) WHERE id = ? AND state = 'active'",
        )
        .bind(&now)
        .bind(action_id)
        .execute(&state.pool)
        .await?;
        let _ = write_moderation_audit_entry(
            &state.pool,
            &before.creator_id,
            None,
            &before.created_by_user_id,
            None,
            "creator_enforcement_expired",
            json!({
                "actionId": before.id,
                "scope": before.scope,
                "expiredAt": now,
            }),
        )
        .await?;
        let _ = enqueue_notification_event(
            &state.pool,
            "creator_enforcement_expired",
            &format!(
                "An enforcement restriction for scope '{}' has expired.",
                before.scope
            ),
            Some(&before.created_by_user_id),
            Some("operator"),
            Some(&before.creator_id),
            None,
            None,
            json!({
                "actionId": before.id,
                "scope": before.scope,
                "expiredAt": now,
            }),
            &[],
            std::slice::from_ref(&before.creator_id),
        )
        .await;
        actions.push(CreatorEnforcementReconciliationAction {
            action_type: "action_expired".to_string(),
            target_id: before.id.clone(),
            previous_state: Some("active".to_string()),
            next_state: Some("expired".to_string()),
            reason: "creator enforcement action exceeded its expiry window".to_string(),
            occurred_at: now.clone(),
        });
    }

    let action = fetch_creator_enforcement_action_by_id_raw(&state.pool, action_id).await?;
    Ok(CreatorEnforcementReconciliationReport {
        action_id: action_id.to_string(),
        reconciled_at: now,
        actions,
        action,
    })
}

pub(super) async fn reconcile_scheduled_upload_releases(state: SharedState) -> AppResult<()> {
    publish_due_scheduled_upload_releases(&state.pool, None, None).await
}

pub(super) fn stale_media_processing_cutoff() -> String {
    (Utc::now() - ChronoDuration::minutes(5)).to_rfc3339()
}

pub(super) fn is_upload_job_stale(job: &UploadJob) -> bool {
    job.status == "processing" && job.updated_at < stale_media_processing_cutoff()
}

pub(super) fn stale_live_ingest_cutoff() -> String {
    (Utc::now() - ChronoDuration::seconds(20)).to_rfc3339()
}

pub(super) fn is_live_ingest_session_stale(session: &LiveIngestSession) -> bool {
    session.status == "connected" && session.last_heartbeat_at < stale_live_ingest_cutoff()
}

pub(super) async fn ensure_creator_live_streaming_enabled(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<()> {
    let profile = fetch_creator_profile(pool, creator_id).await?;
    let state = fetch_creator_operational_state(pool, &profile).await?;
    if state.live_streaming_enabled {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "creator is not currently allowed to start or connect live streams".to_string(),
        ))
    }
}

pub(super) async fn ensure_creator_upload_ingest_enabled(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<()> {
    let profile = fetch_creator_profile(pool, creator_id).await?;
    let state = fetch_creator_operational_state(pool, &profile).await?;
    if state.upload_ingest_enabled {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "creator is not currently allowed to ingest or publish uploads".to_string(),
        ))
    }
}

pub(super) async fn ensure_creator_collaboration_enabled(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<()> {
    let profile = fetch_creator_profile(pool, creator_id).await?;
    let state = fetch_creator_operational_state(pool, &profile).await?;
    if state.collaboration_enabled {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "creator is not currently allowed to manage collaboration sessions".to_string(),
        ))
    }
}

pub(super) async fn ensure_creator_can_manage_subscription_tiers(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<()> {
    let profile = fetch_creator_profile(pool, creator_id).await?;
    let state = fetch_creator_operational_state(pool, &profile).await?;
    if state.can_monetize {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "creator is not cleared to manage subscription tiers".to_string(),
        ))
    }
}

pub(super) async fn validate_creator_access_tier(
    pool: &SqlitePool,
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
    let tier = fetch_creator_subscriber_tier_by_id(pool, creator_id, tier_id).await?;
    if tier.status != "active" {
        return Err(AppError::BadRequest(
            "subscription-based access requires an active subscriber tier".to_string(),
        ));
    }
    Ok(())
}

pub(super) async fn ensure_creator_can_publish_paid_content(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<()> {
    let profile = fetch_creator_profile(pool, creator_id).await?;
    let state = fetch_creator_operational_state(pool, &profile).await?;
    if state.can_publish_paid_content {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "creator is not cleared to publish paid content".to_string(),
        ))
    }
}

pub(super) async fn ensure_creator_can_accept_paid_transactions(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<()> {
    let profile = fetch_creator_profile(pool, creator_id).await?;
    let state = fetch_creator_operational_state(pool, &profile).await?;
    if state.can_receive_payouts {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "creator is not cleared to accept paid transactions".to_string(),
        ))
    }
}
