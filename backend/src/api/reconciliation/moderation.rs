use super::*;

pub(crate) async fn reconcile_expired_live_moderation_actions(state: SharedState) -> AppResult<()> {
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

pub(crate) async fn reconcile_expired_live_moderation_actions_for_read(
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

pub(crate) async fn reconcile_single_live_moderation_action(
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
