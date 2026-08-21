use super::*;

pub(crate) async fn fetch_notifications_rows(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<CreatorNotification>> {
    fetch_notifications_rows_limited(pool, creator_id, None).await
}

pub(crate) async fn fetch_notifications_rows_limited(
    pool: &SqlitePool,
    creator_id: &str,
    limit: Option<usize>,
) -> AppResult<Vec<CreatorNotification>> {
    reconcile_notification_deliveries_for_read(pool, None, None, Some(creator_id), None).await?;
    let effective_limit = limit.unwrap_or(i64::MAX as usize).max(1) as i64;
    let rows = sqlx::query(
        r#"
        SELECT id, kind, body, sent_at, amount, actor, delivery_state, read_at
        FROM (
            SELECT
                id,
                kind,
                body,
                sent_at,
                amount,
                actor,
                NULL AS delivery_state,
                NULL AS read_at
            FROM creator_notifications
            WHERE creator_id = ?

            UNION ALL

            SELECT
                d.id AS id,
                e.kind AS kind,
                e.body AS body,
                d.sent_at AS sent_at,
                e.amount AS amount,
                e.actor_label AS actor,
                d.state AS delivery_state,
                d.read_at AS read_at
            FROM notification_deliveries d
            JOIN notification_events e ON e.id = d.event_id
            WHERE d.recipient_creator_id = ? AND d.channel = 'inbox'
        )
        ORDER BY sent_at DESC
        LIMIT ?
        "#,
    )
    .bind(creator_id)
    .bind(creator_id)
    .bind(effective_limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| CreatorNotification {
            id: row.get("id"),
            kind: row.get("kind"),
            body: row.get("body"),
            sent_at: row.get("sent_at"),
            amount: row.get("amount"),
            actor: row.get("actor"),
            delivery_state: row.get("delivery_state"),
            read_at: row.get("read_at"),
        })
        .collect())
}

pub(crate) async fn fetch_notification_deliveries(
    pool: &SqlitePool,
    state_filter: Option<&str>,
    creator_id: Option<&str>,
    limit: i64,
) -> AppResult<Vec<NotificationDeliveryRecord>> {
    reconcile_notification_deliveries_for_read(pool, creator_id, None, None, None).await?;
    let limit = limit.clamp(1, 500);
    let rows = match (state_filter, creator_id) {
        (Some(state), Some(creator_id)) => {
            sqlx::query(
                r#"
                SELECT d.id, d.event_id, e.kind, e.body, d.channel, d.state, e.actor_label,
                       d.recipient_user_id, d.recipient_creator_id, d.sent_at, d.delivered_at,
                       d.read_at, d.failed_at, d.last_error, d.retry_count, d.last_attempted_at, d.next_attempt_at
                FROM notification_deliveries d
                JOIN notification_events e ON e.id = d.event_id
                WHERE d.state = ? AND e.creator_id = ?
                ORDER BY d.sent_at DESC
                LIMIT ?
                "#,
            )
            .bind(state)
            .bind(creator_id)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        (Some(state), None) => {
            sqlx::query(
                r#"
                SELECT d.id, d.event_id, e.kind, e.body, d.channel, d.state, e.actor_label,
                       d.recipient_user_id, d.recipient_creator_id, d.sent_at, d.delivered_at,
                       d.read_at, d.failed_at, d.last_error, d.retry_count, d.last_attempted_at, d.next_attempt_at
                FROM notification_deliveries d
                JOIN notification_events e ON e.id = d.event_id
                WHERE d.state = ?
                ORDER BY d.sent_at DESC
                LIMIT ?
                "#,
            )
            .bind(state)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        (None, Some(creator_id)) => {
            sqlx::query(
                r#"
                SELECT d.id, d.event_id, e.kind, e.body, d.channel, d.state, e.actor_label,
                       d.recipient_user_id, d.recipient_creator_id, d.sent_at, d.delivered_at,
                       d.read_at, d.failed_at, d.last_error, d.retry_count, d.last_attempted_at, d.next_attempt_at
                FROM notification_deliveries d
                JOIN notification_events e ON e.id = d.event_id
                WHERE e.creator_id = ?
                ORDER BY d.sent_at DESC
                LIMIT ?
                "#,
            )
            .bind(creator_id)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        (None, None) => {
            sqlx::query(
                r#"
                SELECT d.id, d.event_id, e.kind, e.body, d.channel, d.state, e.actor_label,
                       d.recipient_user_id, d.recipient_creator_id, d.sent_at, d.delivered_at,
                       d.read_at, d.failed_at, d.last_error, d.retry_count, d.last_attempted_at, d.next_attempt_at
                FROM notification_deliveries d
                JOIN notification_events e ON e.id = d.event_id
                ORDER BY d.sent_at DESC
                LIMIT ?
                "#,
            )
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
    };

    Ok(rows
        .into_iter()
        .map(notification_delivery_record_from_row)
        .collect())
}

pub(crate) async fn fetch_notification_delivery_by_id(
    pool: &SqlitePool,
    delivery_id: &str,
) -> AppResult<NotificationDeliveryRecord> {
    reconcile_notification_deliveries_for_read(pool, None, None, None, Some(delivery_id)).await?;
    fetch_notification_delivery_by_id_raw(pool, delivery_id).await
}

pub(crate) async fn fetch_notification_delivery_by_id_raw(
    pool: &SqlitePool,
    delivery_id: &str,
) -> AppResult<NotificationDeliveryRecord> {
    let row = sqlx::query(
        r#"
        SELECT d.id, d.event_id, e.kind, e.body, d.channel, d.state, e.actor_label,
               d.recipient_user_id, d.recipient_creator_id, d.sent_at, d.delivered_at,
               d.read_at, d.failed_at, d.last_error, d.retry_count, d.last_attempted_at, d.next_attempt_at
        FROM notification_deliveries d
        JOIN notification_events e ON e.id = d.event_id
        WHERE d.id = ?
        "#,
    )
    .bind(delivery_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(notification_delivery_record_from_row(row))
}

pub(crate) async fn reconcile_notification_deliveries_for_read(
    pool: &SqlitePool,
    event_creator_filter: Option<&str>,
    recipient_user_filter: Option<&str>,
    recipient_creator_filter: Option<&str>,
    delivery_filter: Option<&str>,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let pending_exists: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT 1
        FROM notification_deliveries d
        JOIN notification_events e ON e.id = d.event_id
        WHERE d.state IN ('pending', 'retrying')
          AND COALESCE(d.next_attempt_at, d.sent_at) <= ?
          AND (? IS NULL OR e.creator_id = ?)
          AND (? IS NULL OR d.recipient_user_id = ?)
          AND (? IS NULL OR d.recipient_creator_id = ?)
          AND (? IS NULL OR d.id = ?)
        LIMIT 1
        "#,
    )
    .bind(&now)
    .bind(event_creator_filter)
    .bind(event_creator_filter)
    .bind(recipient_user_filter)
    .bind(recipient_user_filter)
    .bind(recipient_creator_filter)
    .bind(recipient_creator_filter)
    .bind(delivery_filter)
    .bind(delivery_filter)
    .fetch_optional(pool)
    .await?;
    if pending_exists.is_none() {
        return Ok(());
    }
    let rows = sqlx::query(
        r#"
        SELECT d.id
        FROM notification_deliveries d
        JOIN notification_events e ON e.id = d.event_id
        WHERE d.state IN ('pending', 'retrying')
          AND COALESCE(d.next_attempt_at, d.sent_at) <= ?
          AND (? IS NULL OR e.creator_id = ?)
          AND (? IS NULL OR d.recipient_user_id = ?)
          AND (? IS NULL OR d.recipient_creator_id = ?)
          AND (? IS NULL OR d.id = ?)
        ORDER BY COALESCE(d.next_attempt_at, d.sent_at) ASC
        LIMIT 100
        "#,
    )
    .bind(&now)
    .bind(event_creator_filter)
    .bind(event_creator_filter)
    .bind(recipient_user_filter)
    .bind(recipient_user_filter)
    .bind(recipient_creator_filter)
    .bind(recipient_creator_filter)
    .bind(delivery_filter)
    .bind(delivery_filter)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let delivery_id: String = row.get("id");
        let _ = dispatch_notification_delivery(pool, &delivery_id).await?;
    }

    Ok(())
}

pub(crate) async fn reconcile_single_notification_delivery(
    state: SharedState,
    delivery_id: &str,
) -> AppResult<NotificationDeliveryReconciliationReport> {
    let before = fetch_notification_delivery_by_id_raw(&state.pool, delivery_id).await?;
    let now = Utc::now().to_rfc3339();
    let mut actions = Vec::new();

    if matches!(before.state.as_str(), "pending" | "retrying")
        && before
            .next_attempt_at
            .as_deref()
            .unwrap_or(before.sent_at.as_str())
            <= now.as_str()
    {
        let after = dispatch_notification_delivery(&state.pool, delivery_id).await?;
        if after.state != before.state {
            let reason = match after.state.as_str() {
                "delivered" => "notification delivery was dispatched successfully",
                "retrying" => "notification delivery failed and was rescheduled",
                "dead_lettered" => "notification delivery exceeded retry policy",
                _ => "notification delivery state changed during reconciliation",
            };
            actions.push(NotificationDeliveryReconciliationAction {
                action_type: "delivery_reconciled".to_string(),
                target_id: delivery_id.to_string(),
                previous_state: Some(before.state.clone()),
                next_state: Some(after.state.clone()),
                reason: reason.to_string(),
                occurred_at: now.clone(),
            });
        }
    }

    let delivery = fetch_notification_delivery_by_id_raw(&state.pool, delivery_id).await?;
    Ok(NotificationDeliveryReconciliationReport {
        delivery_id: delivery_id.to_string(),
        reconciled_at: now,
        actions,
        delivery,
    })
}

pub(crate) async fn fetch_live_notification_recipient_user_ids(
    pool: &SqlitePool,
    streamer_id: &str,
) -> AppResult<Vec<String>> {
    let rows = sqlx::query(
        "SELECT user_id FROM live_stream_notification_preferences WHERE streamer_id = ? AND enabled = 1",
    )
    .bind(streamer_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.get("user_id")).collect())
}
