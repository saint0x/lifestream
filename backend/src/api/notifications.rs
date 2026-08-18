use super::*;

pub(super) async fn list_my_notifications(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<UserNotification>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_user_notifications(&state.pool, &identity.user_id).await?,
    ))
}

pub(super) async fn mark_my_notification_read(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(notification_id): Path<String>,
) -> AppResult<StatusCode> {
    let identity = require_identity(&state.pool, &headers).await?;
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        "UPDATE notification_deliveries SET read_at = COALESCE(read_at, ?) WHERE id = ? AND recipient_user_id = ?",
    )
    .bind(&now)
    .bind(&notification_id)
    .bind(&identity.user_id)
    .execute(&state.pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn fetch_notifications_rows(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<CreatorNotification>> {
    reconcile_notification_deliveries_for_read(pool, None, None, Some(creator_id), None).await?;
    let legacy_rows = sqlx::query(
        "SELECT id, kind, body, sent_at, amount, actor FROM creator_notifications WHERE creator_id = ?",
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    let delivery_rows = sqlx::query(
        r#"
        SELECT d.id, e.kind, e.body, d.sent_at, e.amount, e.actor_label, d.state, d.read_at
        FROM notification_deliveries d
        JOIN notification_events e ON e.id = d.event_id
        WHERE d.recipient_creator_id = ? AND d.channel = 'inbox'
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    let mut notifications: Vec<CreatorNotification> = legacy_rows
        .into_iter()
        .map(|row| CreatorNotification {
            id: row.get("id"),
            kind: row.get("kind"),
            body: row.get("body"),
            sent_at: row.get("sent_at"),
            amount: row.get("amount"),
            actor: row.get("actor"),
            delivery_state: None,
            read_at: None,
        })
        .collect();

    notifications.extend(delivery_rows.into_iter().map(|row| CreatorNotification {
        id: row.get("id"),
        kind: row.get("kind"),
        body: row.get("body"),
        sent_at: row.get("sent_at"),
        amount: row.get("amount"),
        actor: row.get("actor_label"),
        delivery_state: Some(row.get("state")),
        read_at: row.get("read_at"),
    }));

    notifications.sort_by(|a, b| b.sent_at.cmp(&a.sent_at));
    Ok(notifications)
}

pub(super) async fn fetch_user_notifications(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<Vec<UserNotification>> {
    reconcile_notification_deliveries_for_read(pool, None, Some(user_id), None, None).await?;
    let rows = sqlx::query(
        r#"
        SELECT d.id, e.kind, e.body, d.sent_at, e.amount, e.actor_label, d.state, d.read_at
        FROM notification_deliveries d
        JOIN notification_events e ON e.id = d.event_id
        WHERE d.recipient_user_id = ? AND d.channel = 'inbox'
        ORDER BY d.sent_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| UserNotification {
            id: row.get("id"),
            kind: row.get("kind"),
            body: row.get("body"),
            sent_at: row.get("sent_at"),
            amount: row.get("amount"),
            actor: row.get("actor_label"),
            delivery_state: row.get("state"),
            read_at: row.get("read_at"),
        })
        .collect())
}

pub(super) async fn fetch_notification_deliveries(
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

pub(super) async fn fetch_notification_delivery_by_id(
    pool: &SqlitePool,
    delivery_id: &str,
) -> AppResult<NotificationDeliveryRecord> {
    reconcile_notification_deliveries_for_read(pool, None, None, None, Some(delivery_id)).await?;
    fetch_notification_delivery_by_id_raw(pool, delivery_id).await
}

pub(super) async fn fetch_notification_delivery_by_id_raw(
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

pub(super) async fn enqueue_notification_event(
    pool: &SqlitePool,
    kind: &str,
    body: &str,
    actor_user_id: Option<&str>,
    actor_label: Option<&str>,
    creator_id: Option<&str>,
    stream_id: Option<&str>,
    amount: Option<f64>,
    payload: Value,
    recipient_user_ids: &[String],
    recipient_creator_ids: &[String],
) -> AppResult<()> {
    let event_id = format!("notev-{}", Uuid::new_v4().simple());
    let sent_at = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO notification_events (
            id, kind, body, actor_user_id, actor_label, creator_id, stream_id, amount, payload_json, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&event_id)
    .bind(kind)
    .bind(body)
    .bind(actor_user_id)
    .bind(actor_label)
    .bind(creator_id)
    .bind(stream_id)
    .bind(amount)
    .bind(to_json(&payload)?)
    .bind(&sent_at)
    .execute(pool)
    .await?;

    let mut delivery_ids = Vec::new();
    for recipient_user_id in recipient_user_ids {
        let delivery_id = format!("notd-{}", Uuid::new_v4().simple());
        sqlx::query(
            r#"
            INSERT INTO notification_deliveries (
                id, event_id, recipient_user_id, recipient_creator_id, channel, state, sent_at, delivered_at, read_at,
                failed_at, last_error, retry_count, last_attempted_at, next_attempt_at
            ) VALUES (?, ?, ?, NULL, 'inbox', 'pending', ?, NULL, NULL, NULL, NULL, 0, NULL, ?)
            "#,
        )
        .bind(&delivery_id)
        .bind(&event_id)
        .bind(recipient_user_id)
        .bind(&sent_at)
        .bind(&sent_at)
        .execute(pool)
        .await?;
        delivery_ids.push(delivery_id);
    }

    for recipient_creator_id in recipient_creator_ids {
        let delivery_id = format!("notd-{}", Uuid::new_v4().simple());
        sqlx::query(
            r#"
            INSERT INTO notification_deliveries (
                id, event_id, recipient_user_id, recipient_creator_id, channel, state, sent_at, delivered_at, read_at,
                failed_at, last_error, retry_count, last_attempted_at, next_attempt_at
            ) VALUES (?, ?, NULL, ?, 'inbox', 'pending', ?, NULL, NULL, NULL, NULL, 0, NULL, ?)
            "#,
        )
        .bind(&delivery_id)
        .bind(&event_id)
        .bind(recipient_creator_id)
        .bind(&sent_at)
        .bind(&sent_at)
        .execute(pool)
        .await?;
        delivery_ids.push(delivery_id);
    }

    for delivery_id in delivery_ids {
        let _ = dispatch_notification_delivery(pool, &delivery_id).await;
    }

    Ok(())
}

pub(super) async fn dispatch_notification_delivery(
    pool: &SqlitePool,
    delivery_id: &str,
) -> AppResult<NotificationDeliveryRecord> {
    let delivery = fetch_notification_delivery_by_id_raw(pool, delivery_id).await?;
    if delivery.state == "delivered" {
        return Ok(delivery);
    }

    let attempted_at = Utc::now().to_rfc3339();
    if !claim_notification_delivery_attempt(pool, delivery_id, &attempted_at).await? {
        return fetch_notification_delivery_by_id_raw(pool, delivery_id).await;
    }

    let dispatch_result = match delivery.channel.as_str() {
        "inbox" => deliver_inbox_notification(pool, &delivery, &attempted_at).await,
        other => Err(AppError::BadRequest(format!(
            "unsupported notification delivery channel: {other}"
        ))),
    };

    match dispatch_result {
        Ok(()) => Ok(fetch_notification_delivery_by_id_raw(pool, delivery_id).await?),
        Err(error) => {
            mark_notification_delivery_failed(pool, &delivery, &attempted_at, &error).await?;
            Ok(fetch_notification_delivery_by_id_raw(pool, delivery_id).await?)
        }
    }
}

pub(super) async fn reconcile_notification_deliveries_for_read(
    pool: &SqlitePool,
    event_creator_filter: Option<&str>,
    recipient_user_filter: Option<&str>,
    recipient_creator_filter: Option<&str>,
    delivery_filter: Option<&str>,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
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

pub(super) async fn reconcile_single_notification_delivery(
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

pub(super) async fn claim_notification_delivery_attempt(
    pool: &SqlitePool,
    delivery_id: &str,
    attempted_at: &str,
) -> AppResult<bool> {
    let result = sqlx::query(
        r#"
        UPDATE notification_deliveries
        SET state = 'delivering', last_attempted_at = ?, next_attempt_at = NULL
        WHERE id = ?
          AND state IN ('pending', 'retrying')
          AND COALESCE(next_attempt_at, sent_at) <= ?
        "#,
    )
    .bind(attempted_at)
    .bind(delivery_id)
    .bind(attempted_at)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub(super) async fn deliver_inbox_notification(
    pool: &SqlitePool,
    delivery: &NotificationDeliveryRecord,
    attempted_at: &str,
) -> AppResult<()> {
    if let Some(user_id) = delivery.recipient_user_id.as_deref() {
        sqlx::query("SELECT 1 FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .ok_or(AppError::NotFound)?;
    } else if let Some(creator_id) = delivery.recipient_creator_id.as_deref() {
        sqlx::query("SELECT 1 FROM creator_profiles WHERE id = ?")
            .bind(creator_id)
            .fetch_optional(pool)
            .await?
            .ok_or(AppError::NotFound)?;
    } else {
        return Err(AppError::BadRequest(
            "notification delivery is missing a recipient".to_string(),
        ));
    }

    sqlx::query(
        r#"
        UPDATE notification_deliveries
        SET state = 'delivered', delivered_at = ?, failed_at = NULL, last_error = NULL, next_attempt_at = NULL
        WHERE id = ?
        "#,
    )
    .bind(attempted_at)
    .bind(&delivery.id)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn mark_notification_delivery_failed(
    pool: &SqlitePool,
    delivery: &NotificationDeliveryRecord,
    attempted_at: &str,
    error: &AppError,
) -> AppResult<()> {
    let next_retry_count = delivery.retry_count + 1;
    let message = match error {
        AppError::BadRequest(message)
        | AppError::Internal(message)
        | AppError::MediaPipeline(message)
        | AppError::PaymentRequired(message) => message.clone(),
        AppError::NotFound => "notification recipient no longer exists".to_string(),
        AppError::Unauthorized => "notification delivery unauthorized".to_string(),
        AppError::Forbidden => "notification delivery forbidden".to_string(),
        AppError::RateLimited => "notification delivery rate limited".to_string(),
        AppError::Database(err) => format!("notification database failure: {err}"),
        AppError::Io(err) => format!("notification io failure: {err}"),
        AppError::Serialization(err) => format!("notification serialization failure: {err}"),
    };

    if next_retry_count >= MAX_NOTIFICATION_DELIVERY_ATTEMPTS {
        sqlx::query(
            r#"
            UPDATE notification_deliveries
            SET state = 'dead_lettered', failed_at = ?, last_error = ?, retry_count = ?, next_attempt_at = NULL
            WHERE id = ?
            "#,
        )
        .bind(attempted_at)
        .bind(&message)
        .bind(next_retry_count)
        .bind(&delivery.id)
        .execute(pool)
        .await?;
    } else {
        let retry_at =
            (Utc::now() + ChronoDuration::seconds(15 * next_retry_count.max(1))).to_rfc3339();
        sqlx::query(
            r#"
            UPDATE notification_deliveries
            SET state = 'retrying', failed_at = ?, last_error = ?, retry_count = ?, next_attempt_at = ?
            WHERE id = ?
            "#,
        )
        .bind(attempted_at)
        .bind(&message)
        .bind(next_retry_count)
        .bind(&retry_at)
        .bind(&delivery.id)
        .execute(pool)
        .await?;
    }

    Ok(())
}

pub(super) async fn fetch_live_notification_recipient_user_ids(
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
