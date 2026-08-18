use super::*;

pub(super) async fn fetch_active_live_ingest_session(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Option<LiveIngestSession>> {
    reconcile_stale_live_ingest_sessions_for_read(pool, Some(creator_id), None).await?;
    fetch_active_live_ingest_session_unreconciled(pool, creator_id).await
}

pub(super) async fn fetch_active_live_ingest_session_unreconciled(
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

pub(super) async fn count_live_ingest_sessions_for_broadcast(
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

pub(super) async fn fetch_live_ingest_session_by_id(
    pool: &SqlitePool,
    creator_id: &str,
    session_id: &str,
) -> AppResult<LiveIngestSession> {
    reconcile_stale_live_ingest_sessions_for_read(pool, Some(creator_id), Some(session_id)).await?;
    fetch_live_ingest_session_by_id_unreconciled(pool, creator_id, session_id).await
}

pub(super) async fn fetch_live_ingest_session_by_id_unreconciled(
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

pub(super) async fn fetch_recent_live_ingest_sessions(
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

pub(super) async fn fetch_live_ingest_session_by_id_global(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<LiveIngestSession> {
    reconcile_stale_live_ingest_sessions_for_read(pool, None, Some(session_id)).await?;
    fetch_live_ingest_session_by_id_global_unreconciled(pool, session_id).await
}

pub(super) async fn fetch_live_ingest_session_by_id_global_unreconciled(
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

pub(super) async fn fetch_live_ingest_events_for_session(
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

pub(super) async fn fetch_admin_live_ingest_sessions(
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

pub(super) async fn fetch_admin_live_ingest_session_record(
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

pub(super) async fn fetch_creator_live_ingest_session_record(
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

pub(super) async fn write_live_ingest_event(
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

pub(super) async fn fetch_live_ingest_events_for_creator(
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

pub(super) async fn fetch_terminalizable_live_ingest_sessions_for_broadcast(
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

fn live_ingest_session_from_row(row: sqlx::sqlite::SqliteRow) -> AppResult<LiveIngestSession> {
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

pub(super) async fn validate_live_ingest_session(
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

pub(super) async fn transition_broadcast_to_live(
    pool: &SqlitePool,
    creator: &CreatorProfile,
    broadcast: &Broadcast,
    notify_followers: bool,
    emit_creator_notification: bool,
) -> AppResult<()> {
    sqlx::query("UPDATE broadcasts SET status = 'live' WHERE id = ?")
        .bind(&broadcast.id)
        .execute(pool)
        .await?;
    sqlx::query(
        "UPDATE creator_profiles SET live_status = 'live', current_broadcast_id = ? WHERE id = ?",
    )
    .bind(&broadcast.id)
    .bind(&creator.id)
    .execute(pool)
    .await?;
    sqlx::query("UPDATE streamers SET is_live = 1 WHERE handle = ?")
        .bind(&creator.handle)
        .execute(pool)
        .await?;
    ensure_live_stream_row(pool, creator, broadcast, 0).await?;
    let follower_recipient_ids = if notify_followers {
        let streamer = fetch_streamer_by_handle(pool, &creator.handle).await?;
        fetch_live_notification_recipient_user_ids(pool, &streamer.id).await?
    } else {
        Vec::new()
    };
    if emit_creator_notification || !follower_recipient_ids.is_empty() {
        let creator_recipient_ids = if emit_creator_notification {
            vec![creator.id.clone()]
        } else {
            Vec::new()
        };
        enqueue_notification_event(
            pool,
            "creator_live",
            &format!(
                "{} just went live: {}.",
                creator.display_name, broadcast.title
            ),
            Some(&creator.user_id),
            Some(&creator.display_name),
            Some(&creator.id),
            Some(&format!("lv-{}-live", creator.handle)),
            None,
            json!({
                "broadcastId": broadcast.id,
                "title": broadcast.title,
                "category": broadcast.category,
                "followersNotified": notify_followers,
                "creatorNotified": emit_creator_notification,
            }),
            &follower_recipient_ids,
            &creator_recipient_ids,
        )
        .await?;
    }
    if let Some(collaboration_session) =
        fetch_active_collaboration_session_for_broadcast(pool, &broadcast.id).await?
    {
        let _ =
            sync_active_collaboration_mirror_pickups_for_session(pool, &collaboration_session.id)
                .await;
    }
    Ok(())
}

pub(super) async fn enqueue_creator_broadcast_ended_notification(
    pool: &SqlitePool,
    creator: &CreatorProfile,
    broadcast: &Broadcast,
    terminal_status: &str,
    event_type: &str,
) -> AppResult<()> {
    enqueue_notification_event(
        pool,
        "creator_live_ended",
        &format!("{} ended: {}.", creator.display_name, broadcast.title),
        Some(&creator.user_id),
        Some(&creator.display_name),
        Some(&creator.id),
        None,
        None,
        json!({
            "broadcastId": broadcast.id,
            "title": broadcast.title,
            "category": broadcast.category,
            "terminalStatus": terminal_status,
            "eventType": event_type,
        }),
        &[],
        &[creator.id.clone()],
    )
    .await
}

pub(super) async fn reset_creator_live_operational_metrics(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE creator_live_settings
        SET bitrate_kbps = 0,
            cpu_percent = 0,
            dropped_frames = 0,
            free_disk_gb = 0
        WHERE creator_id = ?
        "#,
    )
    .bind(creator_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn ensure_live_stream_row(
    pool: &SqlitePool,
    creator: &CreatorProfile,
    broadcast: &Broadcast,
    viewers: i64,
) -> AppResult<()> {
    let streamer = fetch_streamer_by_handle(pool, &creator.handle).await?;
    let live_stream_id = format!("lv-{}-live", creator.handle);
    let live_slug = format!("{}-live", creator.handle);
    sqlx::query(
        r#"
        INSERT INTO live_streams (
            id, slug, title, category, tags_json, streamer_id, viewers, started_at, thumbnail, language, is_mature
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            slug = excluded.slug,
            title = excluded.title,
            category = excluded.category,
            tags_json = excluded.tags_json,
            viewers = excluded.viewers,
            started_at = excluded.started_at,
            thumbnail = excluded.thumbnail,
            is_mature = excluded.is_mature
        "#,
    )
    .bind(&live_stream_id)
    .bind(&live_slug)
    .bind(&broadcast.title)
    .bind(&broadcast.category)
    .bind(to_json(&broadcast.tags)?)
    .bind(&streamer.id)
    .bind(viewers)
    .bind(&broadcast.started_at)
    .bind(&broadcast.thumbnail)
    .bind("EN")
    .bind(broadcast.is_mature as i64)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn close_live_ingest_session(
    state: &SharedState,
    session: &LiveIngestSession,
    terminal_status: &str,
    event_type: &str,
    payload: Value,
) -> AppResult<()> {
    let pool = &state.pool;
    let creator = fetch_creator_profile(pool, &session.creator_id).await?;
    let broadcast = fetch_broadcast_by_id(pool, &session.creator_id, &session.broadcast_id).await?;
    let started_at = chrono::DateTime::parse_from_rfc3339(&broadcast.started_at)
        .map_err(|_| AppError::BadRequest("invalid broadcast timestamp".to_string()))?
        .with_timezone(&Utc);
    let ended_at = Utc::now();
    let duration_sec = (ended_at - started_at).num_seconds().max(0);
    let now = ended_at.to_rfc3339();

    sqlx::query(
        "UPDATE live_ingest_sessions SET status = ?, disconnected_at = ?, last_heartbeat_at = ? WHERE id = ?",
    )
    .bind(terminal_status)
    .bind(&now)
    .bind(&now)
    .bind(&session.id)
    .execute(pool)
    .await?;
    sqlx::query(
        "UPDATE broadcasts SET status = 'ended', ended_at = ?, duration_sec = ?, average_viewers = ?, peak_viewers = MAX(peak_viewers, ?) WHERE id = ?",
    )
    .bind(&now)
    .bind(duration_sec)
    .bind(session.viewers)
    .bind(session.viewers)
    .bind(&session.broadcast_id)
    .execute(pool)
    .await?;
    sqlx::query(
        "UPDATE creator_profiles SET live_status = 'offline', current_broadcast_id = NULL WHERE id = ?",
    )
    .bind(&session.creator_id)
    .execute(pool)
    .await?;
    reset_creator_live_operational_metrics(pool, &session.creator_id).await?;
    sqlx::query("UPDATE streamers SET is_live = 0 WHERE handle = ?")
        .bind(&creator.handle)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM live_streams WHERE id = ?")
        .bind(format!("lv-{}-live", creator.handle))
        .execute(pool)
        .await?;
    write_live_ingest_event(
        pool,
        &session.id,
        &session.creator_id,
        &session.broadcast_id,
        event_type,
        json!({
            "status": terminal_status,
            "finalViewers": session.viewers,
            "finalBitrateKbps": session.bitrate_kbps,
            "finalDroppedFrames": session.dropped_frames,
            "details": payload,
        }),
    )
    .await?;
    enqueue_creator_broadcast_ended_notification(
        pool,
        &creator,
        &broadcast,
        terminal_status,
        event_type,
    )
    .await?;
    if let Some(collaboration_session) =
        fetch_active_collaboration_session_for_broadcast(pool, &session.broadcast_id).await?
    {
        let _ = end_collaboration_session_internal(
            state,
            &collaboration_session,
            None,
            json!({
                "reason": "source broadcast ingest session closed",
                "broadcastId": session.broadcast_id,
                "terminalStatus": terminal_status,
                "eventType": event_type,
            }),
        )
        .await?;
    }
    publish_creator_live_state(state, &session.creator_id).await?;
    Ok(())
}

pub(super) async fn mark_live_ingest_session_stale(
    state: &SharedState,
    session: &LiveIngestSession,
) -> AppResult<()> {
    mark_live_ingest_session_stale_in_db(&state.pool, session).await?;
    if let Some(collaboration_session) =
        fetch_active_collaboration_session_for_broadcast(&state.pool, &session.broadcast_id).await?
    {
        let _ = sync_active_collaboration_mirror_pickups_for_session_and_publish(
            state,
            &collaboration_session.id,
        )
        .await;
    }
    publish_creator_live_state(state, &session.creator_id).await?;
    Ok(())
}

pub(super) async fn mark_live_ingest_session_stale_in_db(
    pool: &SqlitePool,
    session: &LiveIngestSession,
) -> AppResult<()> {
    let creator = fetch_creator_profile(pool, &session.creator_id).await?;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE live_ingest_sessions SET status = 'stale', disconnected_at = ?, last_heartbeat_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(&session.id)
    .execute(pool)
    .await?;
    sqlx::query(
        "UPDATE broadcasts SET status = 'ready', ended_at = NULL, duration_sec = NULL WHERE id = ? AND status = 'live'",
    )
    .bind(&session.broadcast_id)
    .execute(pool)
    .await?;
    sqlx::query(
        "UPDATE creator_profiles SET live_status = 'ready', current_broadcast_id = ? WHERE id = ?",
    )
    .bind(&session.broadcast_id)
    .bind(&session.creator_id)
    .execute(pool)
    .await?;
    reset_creator_live_operational_metrics(pool, &session.creator_id).await?;
    sqlx::query("UPDATE streamers SET is_live = 0 WHERE handle = ?")
        .bind(&creator.handle)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM live_streams WHERE id = ?")
        .bind(format!("lv-{}-live", creator.handle))
        .execute(pool)
        .await?;
    write_live_ingest_event(
        pool,
        &session.id,
        &session.creator_id,
        &session.broadcast_id,
        "stale_reconciled",
        json!({
            "status": "stale",
            "reason": "live ingest heartbeat exceeded the reconnect grace window",
            "lastViewers": session.viewers,
            "lastBitrateKbps": session.bitrate_kbps,
            "lastDroppedFrames": session.dropped_frames,
        }),
    )
    .await?;
    if let Some(collaboration_session) =
        fetch_active_collaboration_session_for_broadcast(pool, &session.broadcast_id).await?
    {
        let _ =
            sync_active_collaboration_mirror_pickups_for_session(pool, &collaboration_session.id)
                .await;
    }
    Ok(())
}

pub(super) async fn reconcile_stale_live_ingest_sessions(state: SharedState) -> AppResult<()> {
    let cutoff = (Utc::now() - chrono::Duration::seconds(20)).to_rfc3339();
    let rows = sqlx::query(
        r#"
        SELECT id, creator_id, broadcast_id, protocol, ingest_server, status, bitrate_kbps, viewers,
               dropped_frames, connected_at, last_heartbeat_at, disconnected_at
        FROM live_ingest_sessions
        WHERE status = 'connected' AND last_heartbeat_at < ?
        "#,
    )
    .bind(&cutoff)
    .fetch_all(&state.pool)
    .await?;

    for row in rows {
        let session = live_ingest_session_from_row(row)?;
        mark_live_ingest_session_stale(&state, &session).await?;
    }

    Ok(())
}

pub(super) async fn reconcile_stale_live_ingest_sessions_for_read(
    pool: &SqlitePool,
    creator_filter: Option<&str>,
    session_filter: Option<&str>,
) -> AppResult<()> {
    let cutoff = stale_live_ingest_cutoff();
    let rows = sqlx::query(
        r#"
        SELECT id, creator_id, broadcast_id, protocol, ingest_server, status, bitrate_kbps, viewers,
               dropped_frames, connected_at, last_heartbeat_at, disconnected_at
        FROM live_ingest_sessions
        WHERE status = 'connected'
          AND last_heartbeat_at < ?
          AND (? IS NULL OR creator_id = ?)
          AND (? IS NULL OR id = ?)
        ORDER BY last_heartbeat_at ASC
        LIMIT 50
        "#,
    )
    .bind(&cutoff)
    .bind(creator_filter)
    .bind(creator_filter)
    .bind(session_filter)
    .bind(session_filter)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let session = live_ingest_session_from_row(row)?;
        mark_live_ingest_session_stale_in_db(pool, &session).await?;
    }

    Ok(())
}

pub(super) async fn reconcile_single_live_ingest_session(
    state: SharedState,
    session_id: &str,
) -> AppResult<LiveIngestReconciliationReport> {
    let now = Utc::now().to_rfc3339();
    let mut actions = Vec::new();
    let session =
        fetch_live_ingest_session_by_id_global_unreconciled(&state.pool, session_id).await?;

    if session.status == "connected" && is_live_ingest_session_stale(&session) {
        mark_live_ingest_session_stale(&state, &session).await?;
        actions.push(LiveIngestReconciliationAction {
            action_type: "session_marked_stale".to_string(),
            target_id: session.id.clone(),
            previous_status: Some("connected".to_string()),
            next_status: Some("stale".to_string()),
            reason: "live ingest heartbeat exceeded the reconnect grace window".to_string(),
            occurred_at: now.clone(),
        });
    }

    let record = fetch_admin_live_ingest_session_record(&state.pool, session_id).await?;
    Ok(LiveIngestReconciliationReport {
        session_id: session_id.to_string(),
        reconciled_at: now,
        actions,
        record,
    })
}
