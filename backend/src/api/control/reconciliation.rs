use super::queries::live_ingest_session_from_row;
use super::*;

pub(crate) async fn mark_live_ingest_session_stale(
    state: &SharedState,
    session: &LiveIngestSession,
) -> AppResult<()> {
    mark_live_ingest_session_stale_in_db(state.db.try_sqlite_adapter()?, session).await?;
    let refreshed_session = fetch_live_ingest_session_by_id(
        state.db.try_sqlite_adapter()?,
        &session.creator_id,
        &session.id,
    )
    .await?;
    persist_live_runtime_spec(state, &refreshed_session).await?;
    if let Some(collaboration_session) = fetch_active_collaboration_session_for_broadcast(
        state.db.try_sqlite_adapter()?,
        &session.broadcast_id,
    )
    .await?
    {
        let _ = sync_active_collaboration_mirror_pickups_for_session_and_publish(
            state,
            &collaboration_session.id,
        )
        .await;
    }
    publish_current_creator_live_state(state, &session.creator_id).await?;
    Ok(())
}

pub(crate) async fn mark_live_ingest_session_stale_in_db(
    pool: &SqlitePool,
    session: &LiveIngestSession,
) -> AppResult<()> {
    let creator = fetch_creator_profile(pool, &session.creator_id).await?;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE live_ingest_sessions SET status = 'stale', contribution_state = 'stale', disconnected_at = ?, last_heartbeat_at = ? WHERE id = ?",
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
    let runtime_output = set_live_runtime_output_session_state(pool, session, "stale").await?;
    let (cpu_percent, free_disk_gb) =
        fetch_current_operational_telemetry(pool, &session.creator_id).await?;
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
            "reason": "live ingest heartbeat exceeded the reconnect grace window",
        }),
    )
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

pub(crate) async fn reconcile_stale_live_ingest_sessions(state: SharedState) -> AppResult<()> {
    let cutoff = stale_live_ingest_cutoff();
    let rows = sqlx::query(
        r#"
        SELECT id, creator_id, broadcast_id, previous_session_id, protocol, contribution_class, contribution_state,
               ingest_server, ingest_latency_ms, source_container_format, source_video_codec,
               source_audio_codec, source_width, source_height, source_frame_rate,
               source_audio_sample_rate_hz, source_audio_channels, last_source_probe_at,
               source_validation_state, source_validation_issues_json, status,
               bitrate_kbps, viewers, dropped_frames, connected_at, last_heartbeat_at,
               disconnected_at
        FROM live_ingest_sessions
        WHERE status = 'connected' AND last_heartbeat_at < ?
        "#,
    )
    .bind(&cutoff)
    .fetch_all(state.db.try_sqlite_adapter()?)
    .await?;

    for row in rows {
        let session = live_ingest_session_from_row(row)?;
        mark_live_ingest_session_stale(&state, &session).await?;
    }

    Ok(())
}

pub(crate) async fn reconcile_stale_live_ingest_sessions_for_read(
    pool: &SqlitePool,
    creator_filter: Option<&str>,
    session_filter: Option<&str>,
) -> AppResult<()> {
    let cutoff = stale_live_ingest_cutoff();
    let stale_exists: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT 1
        FROM live_ingest_sessions
        WHERE status = 'connected'
          AND last_heartbeat_at < ?
          AND (? IS NULL OR creator_id = ?)
          AND (? IS NULL OR id = ?)
        LIMIT 1
        "#,
    )
    .bind(&cutoff)
    .bind(creator_filter)
    .bind(creator_filter)
    .bind(session_filter)
    .bind(session_filter)
    .fetch_optional(pool)
    .await?;
    if stale_exists.is_none() {
        return Ok(());
    }

    let rows = sqlx::query(
        r#"
        SELECT id, creator_id, broadcast_id, previous_session_id, protocol, contribution_class, contribution_state,
               ingest_server, ingest_latency_ms, source_container_format, source_video_codec,
               source_audio_codec, source_width, source_height, source_frame_rate,
               source_audio_sample_rate_hz, source_audio_channels, last_source_probe_at,
               source_validation_state, source_validation_issues_json, status,
               bitrate_kbps, viewers, dropped_frames, connected_at, last_heartbeat_at,
               disconnected_at
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

pub(crate) async fn reconcile_single_live_ingest_session(
    state: SharedState,
    session_id: &str,
) -> AppResult<LiveIngestReconciliationReport> {
    let now = Utc::now().to_rfc3339();
    let mut actions = Vec::new();
    let session = fetch_live_ingest_session_by_id_global_unreconciled(
        state.db.try_sqlite_adapter()?,
        session_id,
    )
    .await?;

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

    let record =
        fetch_admin_live_ingest_session_record(state.db.try_sqlite_adapter()?, session_id).await?;
    Ok(LiveIngestReconciliationReport {
        session_id: session_id.to_string(),
        reconciled_at: now,
        actions,
        record,
    })
}
