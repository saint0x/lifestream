use super::*;

#[tokio::test]
async fn stale_ingest_reconcile_preserves_broadcast_for_reconnect() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;

    let response = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-a".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;

    sqlx::query("UPDATE live_ingest_sessions SET last_heartbeat_at = ? WHERE id = ?")
        .bind((Utc::now() - chrono::Duration::seconds(60)).to_rfc3339())
        .bind(&response.session.id)
        .execute(&state.pool)
        .await?;

    reconcile_stale_live_ingest_sessions(state.clone()).await?;

    let session =
        fetch_live_ingest_session_by_id(&state.pool, &creator.id, &response.session.id).await?;
    let refreshed_broadcast =
        fetch_broadcast_by_id(&state.pool, &creator.id, &broadcast.id).await?;
    let refreshed_creator = fetch_creator_profile(&state.pool, &creator.id).await?;

    assert_eq!(session.status, "stale");
    assert_eq!(refreshed_broadcast.status, "ready");
    assert!(refreshed_broadcast.ended_at.is_none());
    assert_eq!(refreshed_creator.live_status, "ready");
    assert_eq!(
        refreshed_creator.current_broadcast_id.as_deref(),
        Some(broadcast.id.as_str())
    );
    assert!(
        sqlx::query("SELECT 1 FROM live_streams WHERE id = ?")
            .bind(format!("lv-{}-live", creator.handle))
            .fetch_optional(&state.pool)
            .await?
            .is_none()
    );

    Ok(())
}

#[tokio::test]
async fn stale_reconciliation_updates_runtime_spec_artifact() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;

    let response = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-stale-spec".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;

    sqlx::query("UPDATE live_ingest_sessions SET last_heartbeat_at = ? WHERE id = ?")
        .bind((Utc::now() - chrono::Duration::seconds(60)).to_rfc3339())
        .bind(&response.session.id)
        .execute(&state.pool)
        .await?;

    reconcile_stale_live_ingest_sessions(state.clone()).await?;

    let spec_full_path = media_path_for_relative(&state, &runtime_spec_path(&response.session));
    let spec: Value = serde_json::from_str(
        &tokio::fs::read_to_string(&spec_full_path)
            .await
            .map_err(AppError::Io)?,
    )?;

    assert_eq!(spec["session"]["status"], "stale");
    assert_eq!(spec["session"]["contributionState"], "stale");
    assert_eq!(spec["runtime"]["state"], "stale");
    assert_eq!(spec["runtime"]["packagingStatus"], "degraded");

    Ok(())
}

#[tokio::test]
async fn stale_live_reads_hide_stream_and_self_heal_creator_snapshot() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;

    let response = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-a".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;

    sqlx::query("UPDATE live_ingest_sessions SET last_heartbeat_at = ? WHERE id = ?")
        .bind((Utc::now() - chrono::Duration::seconds(60)).to_rfc3339())
        .bind(&response.session.id)
        .execute(&state.pool)
        .await?;

    let public_stream =
        fetch_live_stream_by_id(&state.pool, &format!("lv-{}-live", creator.handle)).await;
    assert!(matches!(public_stream, Err(AppError::NotFound)));

    let snapshot = build_creator_live_snapshot(&state.pool, &creator.id).await?;
    let refreshed_broadcast =
        fetch_broadcast_by_id(&state.pool, &creator.id, &broadcast.id).await?;
    let refreshed_creator = fetch_creator_profile(&state.pool, &creator.id).await?;
    let refreshed_session =
        fetch_live_ingest_session_by_id(&state.pool, &creator.id, &response.session.id).await?;

    assert!(snapshot.current_broadcast.is_none());
    assert_eq!(
        snapshot
            .pending_broadcast
            .as_ref()
            .map(|item| item.id.as_str()),
        Some(broadcast.id.as_str())
    );
    assert!(snapshot.ingest_session.is_none());
    assert_eq!(refreshed_session.status, "stale");
    assert_eq!(refreshed_broadcast.status, "ready");
    assert_eq!(refreshed_creator.live_status, "ready");
    assert!(
        sqlx::query("SELECT 1 FROM live_streams WHERE id = ?")
            .bind(format!("lv-{}-live", creator.handle))
            .fetch_optional(&state.pool)
            .await?
            .is_none()
    );

    Ok(())
}

#[tokio::test]
async fn direct_live_ingest_read_self_heals_stale_session() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;

    let response = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-a".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;

    sqlx::query("UPDATE live_ingest_sessions SET last_heartbeat_at = ? WHERE id = ?")
        .bind((Utc::now() - chrono::Duration::seconds(60)).to_rfc3339())
        .bind(&response.session.id)
        .execute(&state.pool)
        .await?;

    let refreshed_session =
        fetch_live_ingest_session_by_id(&state.pool, &creator.id, &response.session.id).await?;
    let refreshed_broadcast =
        fetch_broadcast_by_id(&state.pool, &creator.id, &broadcast.id).await?;
    let refreshed_creator = fetch_creator_profile(&state.pool, &creator.id).await?;

    assert_eq!(refreshed_session.status, "stale");
    assert_eq!(refreshed_broadcast.status, "ready");
    assert_eq!(refreshed_creator.live_status, "ready");
    assert!(
        sqlx::query("SELECT 1 FROM live_streams WHERE id = ?")
            .bind(format!("lv-{}-live", creator.handle))
            .fetch_optional(&state.pool)
            .await?
            .is_none()
    );

    Ok(())
}

#[tokio::test]
async fn active_live_ingest_read_omits_stale_session_without_waiting_for_background_loop()
-> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;

    let response = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-b".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;

    sqlx::query("UPDATE live_ingest_sessions SET last_heartbeat_at = ? WHERE id = ?")
        .bind((Utc::now() - chrono::Duration::seconds(60)).to_rfc3339())
        .bind(&response.session.id)
        .execute(&state.pool)
        .await?;

    let active = fetch_active_live_ingest_session(&state.pool, &creator.id).await?;
    let refreshed_session =
        fetch_live_ingest_session_by_id(&state.pool, &creator.id, &response.session.id).await?;

    assert!(active.is_none());
    assert_eq!(refreshed_session.status, "stale");
    Ok(())
}

#[tokio::test]
async fn reconnect_after_stale_does_not_duplicate_live_notification() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    let before_count = creator_live_event_count(&state.pool, &broadcast.id).await?;

    let first = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-a".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;
    let after_first_connect = creator_live_event_count(&state.pool, &broadcast.id).await?;
    assert_eq!(after_first_connect, before_count + 1);

    sqlx::query("UPDATE live_ingest_sessions SET last_heartbeat_at = ? WHERE id = ?")
        .bind((Utc::now() - chrono::Duration::seconds(60)).to_rfc3339())
        .bind(&first.session.id)
        .execute(&state.pool)
        .await?;
    reconcile_stale_live_ingest_sessions(state.clone()).await?;

    let second = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-b".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;
    let after_reconnect = creator_live_event_count(&state.pool, &broadcast.id).await?;
    let refreshed_broadcast =
        fetch_broadcast_by_id(&state.pool, &creator.id, &broadcast.id).await?;
    let reconnect_spec_path = media_path_for_relative(&state, &runtime_spec_path(&second.session));
    let reconnect_spec: Value = serde_json::from_str(
        &tokio::fs::read_to_string(&reconnect_spec_path)
            .await
            .map_err(AppError::Io)?,
    )?;
    let runtime = fetch_creator_live_runtime_response(&state.pool, &creator.id).await?;

    assert_ne!(first.session.id, second.session.id);
    assert_eq!(
        second.session.previous_session_id.as_deref(),
        Some(first.session.id.as_str())
    );
    assert_eq!(after_reconnect, after_first_connect);
    assert_eq!(refreshed_broadcast.status, "live");
    assert_eq!(
        reconnect_spec["session"]["previousSessionId"],
        first.session.id.clone()
    );
    assert_eq!(reconnect_spec["session"]["reconnectSession"], true);
    assert_eq!(
        reconnect_spec["reconnectPolicy"]["requiresDiscontinuityOnReconnect"],
        true
    );
    assert_eq!(reconnect_spec["runtime"]["discontinuitySequence"], 1);
    assert_eq!(reconnect_spec["packaging"]["discontinuitySequence"], 1);
    assert_eq!(runtime.telemetry_summary.reconnect_events, 1);
    assert_eq!(runtime.telemetry_summary.peak_discontinuity_sequence, 1);
    assert!(runtime.recent_telemetry.iter().any(|sample| {
        sample.sample_kind == "session_connected"
            && sample.detail["session"]["reconnectSession"] == true
            && sample.detail["session"]["previousSessionId"] == first.session.id
            && sample.detail["delivery"]["discontinuitySequence"] == 1
    }));

    Ok(())
}

#[tokio::test]
async fn connect_live_ingest_self_reconciles_stale_active_session() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;

    let first = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-a".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;

    sqlx::query("UPDATE live_ingest_sessions SET last_heartbeat_at = ? WHERE id = ?")
        .bind((Utc::now() - chrono::Duration::seconds(60)).to_rfc3339())
        .bind(&first.session.id)
        .execute(&state.pool)
        .await?;

    let second = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-b".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;

    let first_session =
        fetch_live_ingest_session_by_id(&state.pool, &creator.id, &first.session.id).await?;
    let refreshed_broadcast =
        fetch_broadcast_by_id(&state.pool, &creator.id, &broadcast.id).await?;
    let refreshed_creator = fetch_creator_profile(&state.pool, &creator.id).await?;

    assert_eq!(first_session.status, "stale");
    assert_ne!(first.session.id, second.session.id);
    assert_eq!(refreshed_broadcast.status, "live");
    assert_eq!(refreshed_creator.live_status, "live");
    assert_eq!(
        refreshed_creator.current_broadcast_id.as_deref(),
        Some(broadcast.id.as_str())
    );

    Ok(())
}
