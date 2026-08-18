use super::*;

pub(crate) async fn fetch_active_live_ingest_session(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Option<LiveIngestSession>> {
    reconcile_stale_live_ingest_sessions_for_read(pool, Some(creator_id), None).await?;
    fetch_active_live_ingest_session_unreconciled(pool, creator_id).await
}

pub(crate) async fn fetch_active_live_ingest_session_unreconciled(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Option<LiveIngestSession>> {
    let row = sqlx::query(
        r#"
        SELECT id, creator_id, broadcast_id, protocol, ingest_server, status, bitrate_kbps, viewers,
               dropped_frames, connected_at, last_heartbeat_at, disconnected_at
        FROM live_ingest_sessions
        WHERE creator_id = ? AND status = 'connected'
        ORDER BY connected_at DESC
        LIMIT 1
        "#,
    )
    .bind(creator_id)
    .fetch_optional(pool)
    .await?;

    row.map(live_ingest_session_from_row).transpose()
}

pub(crate) async fn count_live_ingest_sessions_for_broadcast(
    pool: &SqlitePool,
    creator_id: &str,
    broadcast_id: &str,
) -> AppResult<i64> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS count
        FROM live_ingest_sessions
        WHERE creator_id = ? AND broadcast_id = ?
        "#,
    )
    .bind(creator_id)
    .bind(broadcast_id)
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}

pub(crate) async fn fetch_live_ingest_session_by_id(
    pool: &SqlitePool,
    creator_id: &str,
    session_id: &str,
) -> AppResult<LiveIngestSession> {
    reconcile_stale_live_ingest_sessions_for_read(pool, Some(creator_id), Some(session_id)).await?;
    fetch_live_ingest_session_by_id_unreconciled(pool, creator_id, session_id).await
}

pub(crate) async fn fetch_live_ingest_session_by_id_unreconciled(
    pool: &SqlitePool,
    creator_id: &str,
    session_id: &str,
) -> AppResult<LiveIngestSession> {
    let row = sqlx::query(
        r#"
        SELECT id, creator_id, broadcast_id, protocol, ingest_server, status, bitrate_kbps, viewers,
               dropped_frames, connected_at, last_heartbeat_at, disconnected_at
        FROM live_ingest_sessions
        WHERE creator_id = ? AND id = ?
        "#,
    )
    .bind(creator_id)
    .bind(session_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    live_ingest_session_from_row(row)
}

pub(crate) async fn fetch_recent_live_ingest_sessions(
    pool: &SqlitePool,
    creator_id: &str,
    limit: i64,
) -> AppResult<Vec<LiveIngestSession>> {
    reconcile_stale_live_ingest_sessions_for_read(pool, Some(creator_id), None).await?;
    let rows = sqlx::query(
        r#"
        SELECT id, creator_id, broadcast_id, protocol, ingest_server, status, bitrate_kbps, viewers,
               dropped_frames, connected_at, last_heartbeat_at, disconnected_at
        FROM live_ingest_sessions
        WHERE creator_id = ?
        ORDER BY connected_at DESC
        LIMIT ?
        "#,
    )
    .bind(creator_id)
    .bind(limit.max(1))
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(live_ingest_session_from_row).collect()
}

pub(crate) async fn fetch_live_ingest_session_by_id_global(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<LiveIngestSession> {
    reconcile_stale_live_ingest_sessions_for_read(pool, None, Some(session_id)).await?;
    fetch_live_ingest_session_by_id_global_unreconciled(pool, session_id).await
}

pub(crate) async fn fetch_live_ingest_session_by_id_global_unreconciled(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<LiveIngestSession> {
    let row = sqlx::query(
        r#"
        SELECT id, creator_id, broadcast_id, protocol, ingest_server, status, bitrate_kbps, viewers,
               dropped_frames, connected_at, last_heartbeat_at, disconnected_at
        FROM live_ingest_sessions
        WHERE id = ?
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    live_ingest_session_from_row(row)
}

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

    Ok(rows
        .into_iter()
        .map(|row| LiveIngestEvent {
            id: row.get("id"),
            session_id: row.get("session_id"),
            creator_id: row.get("creator_id"),
            broadcast_id: row.get("broadcast_id"),
            event_type: row.get("event_type"),
            payload: serde_json::from_str(&row.get::<String, _>("payload_json"))
                .unwrap_or(json!({})),
            created_at: row.get("created_at"),
        })
        .collect())
}

pub(crate) async fn fetch_admin_live_ingest_sessions(
    pool: &SqlitePool,
    creator_filter: Option<&str>,
    status_filter: Option<&str>,
    limit: i64,
) -> AppResult<Vec<AdminLiveIngestSessionRecord>> {
    reconcile_stale_live_ingest_sessions_for_read(pool, creator_filter, None).await?;
    let limit = limit.clamp(1, 250);
    let rows = match (creator_filter, status_filter) {
        (Some(creator_id), Some(status)) => {
            sqlx::query(
                "SELECT id FROM live_ingest_sessions WHERE creator_id = ? AND status = ? ORDER BY connected_at DESC LIMIT ?",
            )
            .bind(creator_id)
            .bind(status)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        (Some(creator_id), None) => {
            sqlx::query(
                "SELECT id FROM live_ingest_sessions WHERE creator_id = ? ORDER BY connected_at DESC LIMIT ?",
            )
            .bind(creator_id)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        (None, Some(status)) => {
            sqlx::query(
                "SELECT id FROM live_ingest_sessions WHERE status = ? ORDER BY connected_at DESC LIMIT ?",
            )
            .bind(status)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        (None, None) => {
            sqlx::query("SELECT id FROM live_ingest_sessions ORDER BY connected_at DESC LIMIT ?")
                .bind(limit)
                .fetch_all(pool)
                .await?
        }
    };

    let mut records = Vec::with_capacity(rows.len());
    for row in rows {
        let session_id: String = row.get("id");
        records.push(fetch_admin_live_ingest_session_record(pool, &session_id).await?);
    }
    Ok(records)
}

pub(crate) async fn fetch_admin_live_ingest_session_record(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<AdminLiveIngestSessionRecord> {
    let session = fetch_live_ingest_session_by_id_global(pool, session_id).await?;
    let recent_events = fetch_live_ingest_events_for_session(pool, session_id, 20).await?;
    Ok(AdminLiveIngestSessionRecord {
        stale_connection: is_live_ingest_session_stale(&session),
        session,
        recent_events,
    })
}

pub(crate) async fn fetch_creator_live_ingest_session_record(
    pool: &SqlitePool,
    creator_id: &str,
    session_id: &str,
) -> AppResult<AdminLiveIngestSessionRecord> {
    let session = fetch_live_ingest_session_by_id(pool, creator_id, session_id).await?;
    let recent_events = fetch_live_ingest_events_for_session(pool, session_id, 20).await?;
    Ok(AdminLiveIngestSessionRecord {
        stale_connection: is_live_ingest_session_stale(&session),
        session,
        recent_events,
    })
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

    Ok(rows
        .into_iter()
        .map(|row| LiveIngestEvent {
            id: row.get("id"),
            session_id: row.get("session_id"),
            creator_id: row.get("creator_id"),
            broadcast_id: row.get("broadcast_id"),
            event_type: row.get("event_type"),
            payload: serde_json::from_str(&row.get::<String, _>("payload_json"))
                .unwrap_or(json!({})),
            created_at: row.get("created_at"),
        })
        .collect())
}

pub(crate) async fn fetch_terminalizable_live_ingest_sessions_for_broadcast(
    pool: &SqlitePool,
    creator_id: &str,
    broadcast_id: &str,
) -> AppResult<Vec<LiveIngestSession>> {
    let rows = sqlx::query(
        r#"
        SELECT id, creator_id, broadcast_id, protocol, ingest_server, status, bitrate_kbps, viewers,
               dropped_frames, connected_at, last_heartbeat_at, disconnected_at
        FROM live_ingest_sessions
        WHERE creator_id = ?
          AND broadcast_id = ?
          AND status IN ('connected', 'stale')
        ORDER BY connected_at DESC
        "#,
    )
    .bind(creator_id)
    .bind(broadcast_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(live_ingest_session_from_row).collect()
}

pub(super) fn live_ingest_session_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> AppResult<LiveIngestSession> {
    Ok(LiveIngestSession {
        id: row.get("id"),
        creator_id: row.get("creator_id"),
        broadcast_id: row.get("broadcast_id"),
        protocol: row.get("protocol"),
        ingest_server: row.get("ingest_server"),
        status: row.get("status"),
        bitrate_kbps: row.get("bitrate_kbps"),
        viewers: row.get("viewers"),
        dropped_frames: row.get("dropped_frames"),
        connected_at: row.get("connected_at"),
        last_heartbeat_at: row.get("last_heartbeat_at"),
        disconnected_at: row.get("disconnected_at"),
    })
}

pub(crate) async fn validate_live_ingest_session(
    pool: &SqlitePool,
    session_id: &str,
    ingest_token: &str,
) -> AppResult<LiveIngestSession> {
    let token_hash = crate::auth::hash_token(ingest_token);
    let row = sqlx::query(
        r#"
        SELECT id, creator_id, broadcast_id, protocol, ingest_server, status, bitrate_kbps, viewers,
               dropped_frames, connected_at, last_heartbeat_at, disconnected_at
        FROM live_ingest_sessions
        WHERE id = ? AND ingest_token_hash = ? AND status = 'connected'
        "#,
    )
    .bind(session_id)
    .bind(token_hash)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    live_ingest_session_from_row(row)
}
