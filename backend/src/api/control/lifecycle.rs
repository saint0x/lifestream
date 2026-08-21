use super::*;

pub(crate) async fn transition_broadcast_to_live(
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

pub(crate) async fn enqueue_creator_broadcast_ended_notification(
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

pub(crate) async fn reset_creator_live_operational_metrics(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<()> {
    crate::api::creator::ensure_creator_live_settings_row(pool, creator_id).await?;
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

pub(crate) async fn ensure_live_stream_row(
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

pub(crate) async fn close_live_ingest_session(
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
        "UPDATE live_ingest_sessions SET status = ?, contribution_state = 'disconnected', disconnected_at = ?, last_heartbeat_at = ? WHERE id = ?",
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
    let (cpu_percent, free_disk_gb) =
        fetch_current_operational_telemetry(pool, &session.creator_id).await?;
    reset_creator_live_operational_metrics(pool, &session.creator_id).await?;
    sqlx::query("UPDATE streamers SET is_live = 0 WHERE handle = ?")
        .bind(&creator.handle)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM live_streams WHERE id = ?")
        .bind(format!("lv-{}-live", creator.handle))
        .execute(pool)
        .await?;
    let runtime_output =
        set_live_runtime_output_session_state(pool, session, "disconnected").await?;
    let refreshed_session =
        fetch_live_ingest_session_by_id(pool, &session.creator_id, &session.id).await?;
    sync_live_runtime_output_artifacts(state, &refreshed_session, &runtime_output).await?;
    persist_live_runtime_spec(state, &refreshed_session).await?;
    record_live_runtime_telemetry(
        pool,
        session,
        "session_state",
        &runtime_output.runtime_state,
        &runtime_output.packaging_status,
        &runtime_output.archive_status,
        cpu_percent,
        free_disk_gb,
        json!({
            "terminalStatus": terminal_status,
            "eventType": event_type,
        }),
    )
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
    publish_current_creator_live_state(state, &session.creator_id).await?;
    Ok(())
}
