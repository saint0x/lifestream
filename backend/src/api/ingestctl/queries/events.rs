use super::*;

pub(crate) async fn fetch_live_ingest_events_for_session(
    pool: &SqlitePool,
    session_id: &str,
    limit: i64,
) -> AppResult<Vec<LiveIngestEvent>> {
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, creator_id, broadcast_id, event_type, payload_json, created_at
        FROM live_ingest_events
        WHERE session_id = ?
        ORDER BY created_at DESC
        LIMIT ?
        "#,
    )
    .bind(session_id)
    .bind(limit.max(1))
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(live_ingest_event_from_row).collect())
}

pub(crate) async fn write_live_ingest_event(
    pool: &SqlitePool,
    session_id: &str,
    creator_id: &str,
    broadcast_id: &str,
    event_type: &str,
    payload: Value,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO live_ingest_events (
            id, session_id, creator_id, broadcast_id, event_type, payload_json, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(format!("lie-{}", Uuid::new_v4().simple()))
    .bind(session_id)
    .bind(creator_id)
    .bind(broadcast_id)
    .bind(event_type)
    .bind(payload.to_string())
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn fetch_live_ingest_events_for_creator(
    pool: &SqlitePool,
    creator_id: &str,
    limit: i64,
) -> AppResult<Vec<LiveIngestEvent>> {
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, creator_id, broadcast_id, event_type, payload_json, created_at
        FROM live_ingest_events
        WHERE creator_id = ?
        ORDER BY created_at DESC
        LIMIT ?
        "#,
    )
    .bind(creator_id)
    .bind(limit.max(1))
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(live_ingest_event_from_row).collect())
}

fn live_ingest_event_from_row(row: sqlx::sqlite::SqliteRow) -> LiveIngestEvent {
    LiveIngestEvent {
        id: row.get("id"),
        session_id: row.get("session_id"),
        creator_id: row.get("creator_id"),
        broadcast_id: row.get("broadcast_id"),
        event_type: row.get("event_type"),
        payload: serde_json::from_str(&row.get::<String, _>("payload_json")).unwrap_or(json!({})),
        created_at: row.get("created_at"),
    }
}
