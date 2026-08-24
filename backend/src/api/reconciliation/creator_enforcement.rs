use super::*;

pub(crate) async fn reconcile_expired_creator_enforcement_actions(
    state: SharedState,
) -> AppResult<()> {
    reconcile_expired_creator_enforcement_actions_for_read(state.db.sqlite_adapter(), None, None)
        .await
}

pub(crate) async fn reconcile_expired_creator_enforcement_actions_for_read(
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

pub(crate) async fn reconcile_single_creator_enforcement_action(
    state: SharedState,
    action_id: &str,
) -> AppResult<CreatorEnforcementReconciliationReport> {
    let before =
        fetch_creator_enforcement_action_by_id_raw(state.db.sqlite_adapter(), action_id).await?;
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
        .execute(state.db.sqlite_adapter())
        .await?;
        let _ = write_moderation_audit_entry(
            state.db.sqlite_adapter(),
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
            state.db.sqlite_adapter(),
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

    let action =
        fetch_creator_enforcement_action_by_id_raw(state.db.sqlite_adapter(), action_id).await?;
    Ok(CreatorEnforcementReconciliationReport {
        action_id: action_id.to_string(),
        reconciled_at: now,
        actions,
        action,
    })
}
