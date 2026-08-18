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
async fn creator_can_inspect_and_reconcile_live_ingest_session_by_id() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(&state.pool, &creator).await?;
    let headers = auth_headers(&token);
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;

    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-inspect".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;
    let mut ingest_headers = HeaderMap::new();
    ingest_headers.insert(
        "x-ingest-token",
        HeaderValue::from_str(&connected.ingest_token).unwrap(),
    );
    let heartbeat = heartbeat_live_ingest(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers,
        Json(IngestHeartbeatRequest {
            bitrate_kbps: 5100,
            viewers: 44,
            dropped_frames: 2,
            cpu_percent: Some(18),
            free_disk_gb: Some(240.0),
        }),
    )
    .await?
    .0;
    assert_eq!(heartbeat.bitrate_kbps, 5100);

    let record = get_creator_live_ingest_session_by_id(
        State(state.clone()),
        headers.clone(),
        Path(connected.session.id.clone()),
    )
    .await?
    .0;
    assert_eq!(record.session.id, connected.session.id);
    assert!(
        record
            .recent_events
            .iter()
            .any(|event| event.event_type == "heartbeat_recorded")
    );

    sqlx::query("UPDATE live_ingest_sessions SET last_heartbeat_at = ? WHERE id = ?")
        .bind((Utc::now() - chrono::Duration::seconds(60)).to_rfc3339())
        .bind(&connected.session.id)
        .execute(&state.pool)
        .await?;

    let report = reconcile_creator_live_ingest_session(
        State(state.clone()),
        headers.clone(),
        Path(connected.session.id.clone()),
    )
    .await?
    .0;
    assert_eq!(report.session_id, connected.session.id);
    assert!(report.actions.iter().any(|action| {
        action.action_type == "session_marked_stale"
            && action.previous_status.as_deref() == Some("connected")
            && action.next_status.as_deref() == Some("stale")
    }));
    assert_eq!(report.record.session.status, "stale");
    assert!(
        report
            .record
            .recent_events
            .iter()
            .any(|event| event.event_type == "stale_reconciled")
    );

    let events = list_creator_live_ingest_events(
        State(state.clone()),
        headers,
        Path(connected.session.id.clone()),
    )
    .await?
    .0;
    assert!(
        events
            .iter()
            .any(|event| event.event_type == "stale_reconciled")
    );
    Ok(())
}

#[tokio::test]
async fn admin_can_inspect_and_reconcile_live_ingest_session_by_id() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(&state.pool, &creator).await?;
    let headers = auth_headers(&token);
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;

    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-admin-inspect".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;

    let record = get_admin_live_ingest_session(
        State(state.clone()),
        headers.clone(),
        Path(connected.session.id.clone()),
    )
    .await?
    .0;
    assert_eq!(record.session.id, connected.session.id);
    assert!(
        record
            .recent_events
            .iter()
            .any(|event| event.event_type == "connected")
    );

    sqlx::query("UPDATE live_ingest_sessions SET last_heartbeat_at = ? WHERE id = ?")
        .bind((Utc::now() - chrono::Duration::seconds(60)).to_rfc3339())
        .bind(&connected.session.id)
        .execute(&state.pool)
        .await?;

    let report = reconcile_admin_live_ingest_session(
        State(state.clone()),
        headers,
        Path(connected.session.id.clone()),
    )
    .await?
    .0;
    assert!(
        report
            .actions
            .iter()
            .any(|action| action.action_type == "session_marked_stale")
    );
    assert_eq!(report.record.session.status, "stale");
    Ok(())
}

#[tokio::test]
async fn creator_contract_surfaces_do_not_leak_ready_statuses() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(&state.pool, &creator).await?;
    let headers = auth_headers(&token);
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;

    let creator_state = get_creator_state(State(state.clone()), headers.clone())
        .await?
        .0;
    assert_eq!(creator_state.dashboard.profile.live_status, "starting");
    assert!(creator_state.dashboard.current_broadcast.is_none());
    assert!(
        creator_state
            .dashboard
            .scheduled_broadcasts
            .iter()
            .any(|item| item.id == broadcast.id && item.status == "scheduled")
    );
    assert!(
        creator_state
            .dashboard
            .scheduled_broadcasts
            .iter()
            .all(|item| item.status != "ready")
    );
    assert_eq!(
        creator_state.live_control.snapshot.profile.live_status,
        "starting"
    );
    assert_eq!(
        creator_state
            .live_control
            .snapshot
            .pending_broadcast
            .as_ref()
            .map(|item| item.status.as_str()),
        Some("scheduled")
    );
    assert_eq!(
        creator_state.live_runtime.snapshot.profile.live_status,
        "starting"
    );
    assert_eq!(
        creator_state
            .live_runtime
            .snapshot
            .pending_broadcast
            .as_ref()
            .map(|item| item.status.as_str()),
        Some("scheduled")
    );

    let bootstrap_payload = bootstrap(State(state.clone()), headers).await?.0;
    assert_eq!(
        bootstrap_payload["creator"]["profile"]["liveStatus"],
        Value::String("starting".to_string())
    );
    assert_eq!(
        bootstrap_payload["creator"]["currentBroadcast"],
        Value::Null
    );
    assert_eq!(
        bootstrap_payload["creator"]["scheduledBroadcasts"][0]["status"],
        Value::String("scheduled".to_string())
    );
    assert_eq!(
        bootstrap_payload["creatorState"]["liveControl"]["snapshot"]["profile"]["liveStatus"],
        Value::String("starting".to_string())
    );
    assert_eq!(
        bootstrap_payload["creatorState"]["liveControl"]["snapshot"]["pendingBroadcast"]["status"],
        Value::String("scheduled".to_string())
    );
    assert_eq!(
        bootstrap_payload["creatorState"]["liveRuntime"]["snapshot"]["pendingBroadcast"]["id"],
        Value::String(broadcast.id)
    );

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

    assert_ne!(first.session.id, second.session.id);
    assert_eq!(after_reconnect, after_first_connect);
    assert_eq!(refreshed_broadcast.status, "live");

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

#[tokio::test]
async fn creator_receives_live_notification_even_when_followers_not_notified() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    let before_count = creator_notification_delivery_count(
        &state.pool,
        &creator.id,
        "creator_live",
        &broadcast.id,
    )
    .await?;

    transition_broadcast_to_live(&state.pool, &creator, &broadcast, false, true).await?;

    let after_count = creator_notification_delivery_count(
        &state.pool,
        &creator.id,
        "creator_live",
        &broadcast.id,
    )
    .await?;
    let notifications = fetch_notifications_rows(&state.pool, &creator.id).await?;

    assert_eq!(after_count, before_count + 1);
    assert!(notifications.iter().any(|item| {
        item.kind == "creator_live"
            && item.body.contains("just went live")
            && item.body.contains(&broadcast.title)
    }));

    Ok(())
}

#[tokio::test]
async fn notification_delivery_attempt_claim_is_exclusive() -> AppResult<()> {
    let (state, _) = setup_test_state().await?;
    let delivery_id = insert_test_notification_delivery(&state.pool, "usr-2", "inbox").await?;
    let attempted_at = Utc::now().to_rfc3339();

    let first_claim =
        claim_notification_delivery_attempt(&state.pool, &delivery_id, &attempted_at).await?;
    let second_claim =
        claim_notification_delivery_attempt(&state.pool, &delivery_id, &attempted_at).await?;
    let delivery = fetch_notification_delivery_by_id(&state.pool, &delivery_id).await?;

    assert!(first_claim);
    assert!(!second_claim);
    assert_eq!(delivery.state, "delivering");
    assert_eq!(
        delivery.last_attempted_at.as_deref(),
        Some(attempted_at.as_str())
    );
    assert!(delivery.next_attempt_at.is_none());

    Ok(())
}

#[tokio::test]
async fn request_origin_validation_accepts_configured_frontend_origin() -> AppResult<()> {
    let (state, _) = setup_test_state().await?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::ORIGIN,
        HeaderValue::from_static("http://localhost:3000"),
    );

    validate_request_origin(&state, &headers)?;
    Ok(())
}

#[tokio::test]
async fn request_origin_validation_rejects_unconfigured_origin() -> AppResult<()> {
    let (state, _) = setup_test_state().await?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::ORIGIN,
        HeaderValue::from_static("https://evil.example"),
    );

    let error =
        validate_request_origin(&state, &headers).expect_err("unconfigured origin must fail");
    assert!(matches!(error, AppError::Forbidden));
    Ok(())
}

#[tokio::test]
async fn concurrent_notification_dispatch_only_records_one_attempt() -> AppResult<()> {
    let (state, _) = setup_test_state().await?;
    let delivery_id =
        insert_test_notification_delivery(&state.pool, "usr-2", "unsupported").await?;

    let (first, second) = tokio::join!(
        dispatch_notification_delivery(&state.pool, &delivery_id),
        dispatch_notification_delivery(&state.pool, &delivery_id)
    );
    let first = first?;
    let second = second?;
    let delivery = fetch_notification_delivery_by_id(&state.pool, &delivery_id).await?;

    assert_eq!(delivery.retry_count, 1);
    assert_eq!(delivery.state, "retrying");
    assert!(delivery.last_error.is_some());
    assert!(delivery.next_attempt_at.is_some());
    assert!(matches!(first.state.as_str(), "retrying" | "delivering"));
    assert!(matches!(second.state.as_str(), "retrying" | "delivering"));

    Ok(())
}

#[tokio::test]
async fn user_notification_read_dispatches_due_pending_delivery() -> AppResult<()> {
    let (state, _) = setup_test_state().await?;
    let delivery_id = insert_test_notification_delivery(&state.pool, "usr-2", "inbox").await?;

    let notifications = fetch_user_notifications(&state.pool, "usr-2").await?;
    let delivery = fetch_notification_delivery_by_id_raw(&state.pool, &delivery_id).await?;

    assert!(
        notifications
            .iter()
            .any(|item| item.id == delivery_id && item.delivery_state == "delivered")
    );
    assert_eq!(delivery.state, "delivered");
    assert!(delivery.delivered_at.is_some());
    Ok(())
}

#[tokio::test]
async fn admin_notification_delivery_read_dispatches_due_retrying_delivery() -> AppResult<()> {
    let (state, _) = setup_test_state().await?;
    let delivery_id = insert_test_notification_delivery(&state.pool, "usr-2", "email").await?;
    sqlx::query(
        "UPDATE notification_deliveries SET state = 'retrying', retry_count = 2, failed_at = ?, last_error = ?, last_attempted_at = ?, next_attempt_at = ? WHERE id = ?",
    )
    .bind("2026-08-18T00:00:00Z")
    .bind("previous email failure")
    .bind("2026-08-18T00:00:00Z")
    .bind("2026-08-18T00:00:01Z")
    .bind(&delivery_id)
    .execute(&state.pool)
    .await?;

    let deliveries = fetch_notification_deliveries(&state.pool, None, None, 100).await?;
    let record = deliveries
        .into_iter()
        .find(|item| item.id == delivery_id)
        .expect("delivery should remain queryable");

    assert_eq!(record.state, "dead_lettered");
    assert!(
        record
            .last_error
            .as_deref()
            .is_some_and(|message| message.contains("unsupported notification delivery channel"))
    );
    assert_eq!(record.retry_count, 3);
    Ok(())
}

#[tokio::test]
async fn admin_can_inspect_and_reconcile_notification_delivery_by_id() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(&state.pool, &creator).await?;
    let headers = auth_headers(&token);
    let delivery_id = insert_test_notification_delivery(&state.pool, "usr-2", "email").await?;

    sqlx::query(
        "UPDATE notification_deliveries SET state = 'retrying', retry_count = 2, failed_at = ?, last_error = ?, last_attempted_at = ?, next_attempt_at = ? WHERE id = ?",
    )
    .bind("2026-08-18T00:00:00Z")
    .bind("previous email failure")
    .bind("2026-08-18T00:00:00Z")
    .bind("2026-08-18T00:00:01Z")
    .bind(&delivery_id)
    .execute(&state.pool)
    .await?;

    let inspected = get_admin_notification_delivery(
        State(state.clone()),
        headers.clone(),
        Path(delivery_id.clone()),
    )
    .await?
    .0;
    assert_eq!(inspected.id, delivery_id);
    assert_eq!(inspected.state, "retrying");

    let report = reconcile_admin_notification_delivery(
        State(state.clone()),
        headers,
        Path(delivery_id.clone()),
    )
    .await?
    .0;
    assert_eq!(report.delivery_id, delivery_id);
    assert!(report.actions.iter().any(|action| {
        action.action_type == "delivery_reconciled"
            && action.previous_state.as_deref() == Some("retrying")
            && action.next_state.as_deref() == Some("dead_lettered")
    }));
    assert_eq!(report.delivery.state, "dead_lettered");
    assert_eq!(report.delivery.retry_count, 3);
    assert!(
        report
            .delivery
            .last_error
            .as_deref()
            .is_some_and(|message| message.contains("unsupported notification delivery channel"))
    );
    Ok(())
}

#[tokio::test]
async fn terminating_ingest_creates_creator_broadcast_end_notification() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-ended-note".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;
    let before_count = creator_notification_delivery_count(
        &state.pool,
        &creator.id,
        "creator_live_ended",
        &connected.session.broadcast_id,
    )
    .await?;

    let _ = terminate_creator_live_ingest(
        State(state.clone()),
        auth_headers(&insert_creator_auth_session(&state.pool, &creator).await?),
        Path(connected.session.id.clone()),
        Json(TerminateLiveIngestRequest {
            reason: Some("creator validation shutdown".to_string()),
        }),
    )
    .await?;

    let after_count = creator_notification_delivery_count(
        &state.pool,
        &creator.id,
        "creator_live_ended",
        &connected.session.broadcast_id,
    )
    .await?;
    let notifications = fetch_notifications_rows(&state.pool, &creator.id).await?;
    let ended_broadcast =
        fetch_broadcast_by_id(&state.pool, &creator.id, &connected.session.broadcast_id).await?;

    assert_eq!(after_count, before_count + 1);
    assert!(notifications.iter().any(|item| {
        item.kind == "creator_live_ended"
            && item.body.contains("ended")
            && item.body.contains(&ended_broadcast.title)
            && item.read_at.is_none()
    }));

    Ok(())
}
