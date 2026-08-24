use super::*;

pub(crate) async fn reconcile_notification_deliveries(state: SharedState) -> AppResult<()> {
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
    .fetch_all(state.db.try_sqlite_adapter()?)
    .await?;

    for row in rows {
        let delivery_id: String = row.get("id");
        let _ =
            dispatch_notification_delivery(state.db.try_sqlite_adapter()?, &delivery_id).await?;
    }

    Ok(())
}

pub(crate) async fn reconcile_scheduled_upload_releases(state: SharedState) -> AppResult<()> {
    publish_due_scheduled_upload_releases(state.db.try_sqlite_adapter()?, None, None).await
}
