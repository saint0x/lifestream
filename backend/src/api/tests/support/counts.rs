use super::*;

pub(super) async fn creator_live_event_count(
    pool: &SqlitePool,
    broadcast_id: &str,
) -> AppResult<i64> {
    let row = sqlx::query(
        "SELECT COUNT(*) AS count FROM notification_events WHERE kind = 'creator_live' AND payload_json LIKE ?",
    )
    .bind(format!("%{broadcast_id}%"))
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}

pub(super) async fn creator_notification_delivery_count(
    pool: &SqlitePool,
    creator_id: &str,
    kind: &str,
    broadcast_id: &str,
) -> AppResult<i64> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS count
        FROM notification_deliveries d
        JOIN notification_events e ON e.id = d.event_id
        WHERE d.recipient_creator_id = ?
          AND e.kind = ?
          AND e.payload_json LIKE ?
        "#,
    )
    .bind(creator_id)
    .bind(kind)
    .bind(format!("%{broadcast_id}%"))
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}

pub(super) async fn insert_test_notification_delivery(
    pool: &SqlitePool,
    recipient_user_id: &str,
    channel: &str,
) -> AppResult<String> {
    let event_id = format!("notev-{}", Uuid::new_v4().simple());
    let delivery_id = format!("notd-{}", Uuid::new_v4().simple());
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO notification_events (
            id, kind, body, actor_user_id, actor_label, creator_id, stream_id, amount, payload_json, created_at
        ) VALUES (?, 'test_notification', 'test body', NULL, NULL, NULL, NULL, NULL, '{}', ?)
        "#,
    )
    .bind(&event_id)
    .bind(&now)
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO notification_deliveries (
            id, event_id, recipient_user_id, recipient_creator_id, channel, state, sent_at, delivered_at, read_at,
            failed_at, last_error, retry_count, last_attempted_at, next_attempt_at
        ) VALUES (?, ?, ?, NULL, ?, 'pending', ?, NULL, NULL, NULL, NULL, 0, NULL, ?)
        "#,
    )
    .bind(&delivery_id)
    .bind(&event_id)
    .bind(recipient_user_id)
    .bind(channel)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(delivery_id)
}

pub(super) async fn live_ingest_event_count_for_session(
    pool: &SqlitePool,
    session_id: &str,
    event_type: &str,
) -> AppResult<i64> {
    let row = sqlx::query(
        "SELECT COUNT(*) AS count FROM live_ingest_events WHERE session_id = ? AND event_type = ?",
    )
    .bind(session_id)
    .bind(event_type)
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}
