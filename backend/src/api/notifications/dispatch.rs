use super::*;

pub(crate) async fn enqueue_notification_event(
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

pub(crate) async fn dispatch_notification_delivery(
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

pub(crate) async fn claim_notification_delivery_attempt(
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

pub(crate) async fn deliver_inbox_notification(
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

pub(crate) async fn mark_notification_delivery_failed(
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
