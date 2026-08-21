use super::*;
use crate::api::notifications::fetch_user_notifications;

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
