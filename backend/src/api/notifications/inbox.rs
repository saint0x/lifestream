use super::*;

pub(crate) async fn list_my_notifications(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<UserNotification>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_user_notifications(&state.pool, &identity.user_id).await?,
    ))
}

pub(crate) async fn mark_my_notification_read(
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

pub(crate) async fn fetch_user_notifications(
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
