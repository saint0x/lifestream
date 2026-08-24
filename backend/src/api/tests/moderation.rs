use super::*;

#[tokio::test]
async fn shadowbanned_chat_history_is_visible_only_to_sender() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let stream_id = insert_live_stream_for_creator(state.db.sqlite_adapter(), &creator).await?;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO chat_messages (
            id, stream_id, user_id, creator_id, user_handle, display_name, color, badges_json,
            body, sent_at, hidden_by_moderation, sequence
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(format!("msg-test-{}", Uuid::new_v4().simple()))
    .bind(&stream_id)
    .bind("usr-1")
    .bind(Some("crt-deepsaint"))
    .bind("deepsaint")
    .bind("deepsaint")
    .bind("#ffffff")
    .bind(json!(["partner"]).to_string())
    .bind("public message")
    .bind(&now)
    .bind(0_i64)
    .bind(1_i64)
    .execute(state.db.sqlite_adapter())
    .await?;

    sqlx::query(
        r#"
        INSERT INTO chat_messages (
            id, stream_id, user_id, creator_id, user_handle, display_name, color, badges_json,
            body, sent_at, hidden_by_moderation, sequence
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(format!("msg-test-{}", Uuid::new_v4().simple()))
    .bind(&stream_id)
    .bind("usr-2")
    .bind(Option::<&str>::None)
    .bind("atlas_codes")
    .bind("atlas_codes")
    .bind("#fafafa")
    .bind(json!(["subscriber"]).to_string())
    .bind("shadow hidden message")
    .bind(&now)
    .bind(1_i64)
    .bind(2_i64)
    .execute(state.db.sqlite_adapter())
    .await?;

    let public_history =
        fetch_chat_messages_for_viewer(state.db.sqlite_adapter(), &stream_id, None, 50, None)
            .await?;
    let sender_history = fetch_chat_messages_for_viewer(
        state.db.sqlite_adapter(),
        &stream_id,
        Some("usr-2"),
        50,
        None,
    )
    .await?;
    let other_history = fetch_chat_messages_for_viewer(
        state.db.sqlite_adapter(),
        &stream_id,
        Some("usr-1"),
        50,
        None,
    )
    .await?;
    let sender_replay = fetch_chat_messages_for_viewer(
        state.db.sqlite_adapter(),
        &stream_id,
        Some("usr-2"),
        50,
        Some(1),
    )
    .await?;
    let sender_token = format!("test-viewer-token-{}", Uuid::new_v4().simple());
    let sender_session_now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO auth_sessions (
            id, user_id, label, token_hash, scopes_json, created_at, expires_at, revoked_at, last_used_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(format!("sess-test-{}", Uuid::new_v4().simple()))
    .bind("usr-2")
    .bind("atlas-chat-rest")
    .bind(hash_token(&sender_token))
    .bind(json!(["user"]).to_string())
    .bind(&sender_session_now)
    .bind((Utc::now() + chrono::Duration::hours(2)).to_rfc3339())
    .bind(Option::<String>::None)
    .bind(&sender_session_now)
    .execute(state.db.sqlite_adapter())
    .await?;
    let sender_rest_history = list_chat_messages(
        State(state.clone()),
        auth_headers(&sender_token),
        Path(stream_id.clone()),
        Query(LimitQuery {
            limit: Some(50),
            after_seq: None,
        }),
    )
    .await?
    .0;
    let public_rest_history = list_chat_messages(
        State(state.clone()),
        HeaderMap::new(),
        Path(stream_id.clone()),
        Query(LimitQuery {
            limit: Some(50),
            after_seq: None,
        }),
    )
    .await?
    .0;

    assert_eq!(public_history.len(), 1);
    assert!(
        public_history
            .iter()
            .all(|item| item.body != "shadow hidden message")
    );
    assert!(
        sender_history
            .iter()
            .any(|item| item.body == "shadow hidden message")
    );
    assert!(
        other_history
            .iter()
            .all(|item| item.body != "shadow hidden message")
    );
    assert_eq!(sender_replay.len(), 1);
    assert_eq!(sender_replay[0].body, "shadow hidden message");
    assert!(
        sender_rest_history
            .iter()
            .any(|item| item.body == "shadow hidden message")
    );
    assert!(
        public_rest_history
            .iter()
            .all(|item| item.body != "shadow hidden message")
    );
    Ok(())
}

#[tokio::test]
async fn collaboration_resume_bootstrap_does_not_duplicate_snapshot_events() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let (session, participant) = insert_active_collaboration_session(
        state.db.sqlite_adapter(),
        &creator,
        "crt-atlas",
        "usr-2",
    )
    .await?;
    let first = publish_test_collaboration_event(
        &state,
        &session.id,
        &participant.id,
        "usr-2",
        "participant_state_requested",
    )
    .await?;
    let second = publish_test_collaboration_event(
        &state,
        &session.id,
        &participant.id,
        "usr-2",
        "mirror_grant_issued",
    )
    .await?;

    let (fresh_snapshot, fresh_replay) =
        load_collaboration_socket_event_bootstrap(state.db.sqlite_adapter(), &session.id, 0)
            .await?;
    let (resumed_snapshot, resumed_replay) = load_collaboration_socket_event_bootstrap(
        state.db.sqlite_adapter(),
        &session.id,
        first.sequence,
    )
    .await?;

    assert_eq!(fresh_replay.len(), 0);
    assert!(fresh_snapshot.iter().any(|event| event.id == first.id));
    assert!(fresh_snapshot.iter().any(|event| event.id == second.id));

    assert!(resumed_snapshot.is_empty());
    assert_eq!(resumed_replay.len(), 1);
    assert_eq!(resumed_replay[0].id, second.id);
    assert!(
        resumed_replay
            .iter()
            .all(|event| event.sequence > first.sequence)
    );
    Ok(())
}

#[tokio::test]
async fn create_collaboration_session_rejects_invalid_chat_mode() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);
    let broadcast =
        insert_ready_collaboration_broadcast(state.db.sqlite_adapter(), &creator).await?;

    let error = create_collaboration_session(
        State(state.clone()),
        headers,
        Json(CreateCollaborationSessionRequest {
            broadcast_id: Some(broadcast.id),
            title: Some("Invalid Chat Mode".to_string()),
            chat_mode: Some("everyone_everywhere".to_string()),
            recording_policy: Some("host_archive".to_string()),
        }),
    )
    .await
    .expect_err("invalid chat mode must be rejected");

    match error {
        AppError::BadRequest(message) => {
            assert!(message.contains("unsupported collaboration chat mode"));
        }
        other => panic!("unexpected error: {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn create_collaboration_session_rejects_invalid_recording_policy() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);
    let broadcast =
        insert_ready_collaboration_broadcast(state.db.sqlite_adapter(), &creator).await?;

    let error = create_collaboration_session(
        State(state.clone()),
        headers,
        Json(CreateCollaborationSessionRequest {
            broadcast_id: Some(broadcast.id),
            title: Some("Invalid Recording Policy".to_string()),
            chat_mode: Some("shared".to_string()),
            recording_policy: Some("mirror_everything".to_string()),
        }),
    )
    .await
    .expect_err("invalid recording policy must be rejected");

    match error {
        AppError::BadRequest(message) => {
            assert!(message.contains("unsupported collaboration recording policy"));
        }
        other => panic!("unexpected error: {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn collaboration_participant_without_chat_speaking_rights_cannot_post() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    sqlx::query(
        "UPDATE creator_live_settings SET subscriber_only = 0, slow_mode_seconds = 0 WHERE creator_id = ?",
    )
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;
    let stream_id = insert_live_stream_for_creator(state.db.sqlite_adapter(), &creator).await?;
    let (_session, participant) = insert_shared_chat_collaboration_for_current_broadcast(
        state.db.sqlite_adapter(),
        &creator,
        "crt-atlas",
        "usr-2",
        false,
    )
    .await?;
    let identity = RequestIdentity {
        session_id: format!("sess-chat-{}", Uuid::new_v4().simple()),
        user_id: participant.user_id.clone(),
        creator_id: participant.creator_id.clone(),
        scopes: vec!["user".to_string(), "creator".to_string()],
    };

    let error = persist_chat_message(
        &state,
        &stream_id,
        &identity,
        ChatInput {
            body: "hello shared chat".to_string(),
            color: None,
        },
    )
    .await
    .expect_err("participant without speaking rights must be blocked");

    assert!(matches!(error, AppError::Forbidden));
    Ok(())
}

#[tokio::test]
async fn revoking_live_moderation_action_publishes_realtime_event() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);
    let stream_id = insert_live_stream_for_creator(state.db.sqlite_adapter(), &creator).await?;

    let created = create_live_moderation_action(
        State(state.clone()),
        headers.clone(),
        Path(stream_id.clone()),
        Json(CreateLiveModerationActionRequest {
            subject_user_id: "usr-2".to_string(),
            action_type: "mute".to_string(),
            reason: "cooldown".to_string(),
            duration_minutes: Some(15),
        }),
    )
    .await?
    .0;

    let (mut subscription, _) = state.realtime.join(&stream_channel_id(&stream_id)).await;

    let revoked = revoke_live_moderation_action(
        State(state.clone()),
        headers,
        Path((stream_id.clone(), created.id.clone())),
    )
    .await?
    .0;

    let event = tokio::time::timeout(Duration::from_secs(1), subscription.recv())
        .await
        .map_err(|_| {
            AppError::Internal("timed out waiting for moderation revoke realtime event".to_string())
        })?
        .map_err(|error| {
            AppError::Internal(format!(
                "failed receiving moderation revoke realtime event: {error}"
            ))
        })?;

    match event {
        WsEvent::ModerationAction { action } => {
            assert_eq!(action.id, revoked.id);
            assert_eq!(action.state, "revoked");
            assert_eq!(action.stream_id, stream_id);
        }
        other => {
            return Err(AppError::Internal(format!(
                "unexpected realtime event for moderation revoke: {other:?}"
            )));
        }
    }

    state.realtime.leave(&stream_channel_id(&stream_id)).await;
    Ok(())
}

#[tokio::test]
async fn moderator_cannot_apply_action_to_stream_owner() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let stream_id = insert_live_stream_for_creator(state.db.sqlite_adapter(), &creator).await?;
    sqlx::query(
        "INSERT INTO creator_moderators (creator_id, user_id, role, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(&creator.id)
    .bind("usr-2")
    .bind("mod")
    .bind(Utc::now().to_rfc3339())
    .execute(state.db.sqlite_adapter())
    .await?;
    let token = insert_user_auth_session(state.db.sqlite_adapter(), "usr-2", &["user"]).await?;

    let error = create_live_moderation_action(
        State(state),
        auth_headers(&token),
        Path(stream_id),
        Json(CreateLiveModerationActionRequest {
            subject_user_id: creator.user_id.clone(),
            action_type: "mute".to_string(),
            reason: "abuse".to_string(),
            duration_minutes: Some(10),
        }),
    )
    .await
    .expect_err("moderator should not target stream owner");

    match error {
        AppError::BadRequest(message) => {
            assert!(message.contains("stream owner"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
    Ok(())
}

#[tokio::test]
async fn resolving_report_rejects_cross_stream_report_ids_without_audit_write() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);
    let stream_id = insert_live_stream_for_creator(state.db.sqlite_adapter(), &creator).await?;

    let other_broadcast = Broadcast {
        id: format!("test-other-bc-{}", Uuid::new_v4().simple()),
        title: "Other Stream".to_string(),
        category: creator.default_category.clone(),
        tags: creator.default_tags.clone(),
        status: "live".to_string(),
        started_at: Utc::now().to_rfc3339(),
        ended_at: None,
        duration_sec: None,
        peak_viewers: 0,
        average_viewers: 0,
        chat_messages: 0,
        new_followers: 0,
        new_subscribers: 0,
        revenue: 0.0,
        thumbnail: "https://cdn.vanta.local/thumb/other-stream.jpg".to_string(),
        is_mature: false,
    };
    sqlx::query(
        r#"
        INSERT INTO broadcasts (
            id, creator_id, title, category, tags_json, status, started_at, ended_at, duration_sec,
            peak_viewers, average_viewers, chat_messages, new_followers, new_subscribers, revenue,
            thumbnail, is_mature
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&other_broadcast.id)
    .bind(&creator.id)
    .bind(&other_broadcast.title)
    .bind(&other_broadcast.category)
    .bind(to_json(&other_broadcast.tags)?)
    .bind(&other_broadcast.status)
    .bind(&other_broadcast.started_at)
    .bind(&other_broadcast.ended_at)
    .bind(&other_broadcast.duration_sec)
    .bind(other_broadcast.peak_viewers)
    .bind(other_broadcast.average_viewers)
    .bind(other_broadcast.chat_messages)
    .bind(other_broadcast.new_followers)
    .bind(other_broadcast.new_subscribers)
    .bind(other_broadcast.revenue)
    .bind(&other_broadcast.thumbnail)
    .bind(other_broadcast.is_mature as i64)
    .execute(state.db.sqlite_adapter())
    .await?;
    let other_stream_id = format!("lv-{}-other", Uuid::new_v4().simple());
    let streamer = fetch_streamer_by_handle(state.db.sqlite_adapter(), &creator.handle).await?;
    sqlx::query(
        r#"
        INSERT INTO live_streams (
            id, slug, title, category, tags_json, streamer_id, viewers, started_at, thumbnail, language, is_mature
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&other_stream_id)
    .bind(format!("{}-other", other_stream_id))
    .bind(&other_broadcast.title)
    .bind(&other_broadcast.category)
    .bind(to_json(&other_broadcast.tags)?)
    .bind(&streamer.id)
    .bind(0_i64)
    .bind(&other_broadcast.started_at)
    .bind(&other_broadcast.thumbnail)
    .bind("EN")
    .bind(other_broadcast.is_mature as i64)
    .execute(state.db.sqlite_adapter())
    .await?;

    let report_id = format!("test-report-{}", Uuid::new_v4().simple());
    let created_at = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO live_stream_reports (id, stream_id, user_id, reason, details, status, resolved_by_user_id, resolution_note, created_at, resolved_at) VALUES (?, ?, ?, ?, ?, 'open', NULL, NULL, ?, NULL)",
    )
    .bind(&report_id)
    .bind(&other_stream_id)
    .bind("usr-2")
    .bind("spam")
    .bind("cross-stream mismatch")
    .bind(&created_at)
    .execute(state.db.sqlite_adapter())
    .await?;

    let before_audit_count: i64 = sqlx::query(
        "SELECT COUNT(*) AS count FROM moderation_audit_log WHERE creator_id = ? AND event_type = 'report_resolved'",
    )
    .bind(&creator.id)
    .fetch_one(state.db.sqlite_adapter())
    .await?
    .get("count");

    let error = resolve_live_stream_report(
        State(state.clone()),
        headers,
        Path((stream_id.clone(), report_id.clone())),
        Json(ResolveLiveStreamReportRequest {
            status: "resolved".to_string(),
            resolution_note: Some("should fail".to_string()),
        }),
    )
    .await
    .expect_err("cross-stream report resolution must fail");

    assert!(matches!(error, AppError::NotFound));

    let after_audit_count: i64 = sqlx::query(
        "SELECT COUNT(*) AS count FROM moderation_audit_log WHERE creator_id = ? AND event_type = 'report_resolved'",
    )
    .bind(&creator.id)
    .fetch_one(state.db.sqlite_adapter())
    .await?
    .get("count");
    let report = fetch_live_stream_report_by_id(state.db.sqlite_adapter(), &report_id).await?;

    assert_eq!(before_audit_count, after_audit_count);
    assert_eq!(report.status, "open");
    assert_eq!(report.stream_id, other_stream_id);
    Ok(())
}

#[tokio::test]
async fn removing_nonexistent_moderator_returns_not_found_without_audit_write() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let stream_id = insert_live_stream_for_creator(state.db.sqlite_adapter(), &creator).await?;

    let before_audit_count: i64 = sqlx::query(
        "SELECT COUNT(*) AS count FROM moderation_audit_log WHERE creator_id = ? AND event_type = 'moderator_removed'",
    )
    .bind(&creator.id)
    .fetch_one(state.db.sqlite_adapter())
    .await?
    .get("count");

    let error = remove_live_stream_moderator(
        State(state.clone()),
        auth_headers(&token),
        Path((stream_id, "usr-missing".to_string())),
    )
    .await
    .expect_err("removing nonexistent moderator must fail");

    assert!(matches!(error, AppError::NotFound));

    let after_audit_count: i64 = sqlx::query(
        "SELECT COUNT(*) AS count FROM moderation_audit_log WHERE creator_id = ? AND event_type = 'moderator_removed'",
    )
    .bind(&creator.id)
    .fetch_one(state.db.sqlite_adapter())
    .await?
    .get("count");
    assert_eq!(before_audit_count, after_audit_count);
    Ok(())
}

#[tokio::test]
async fn expired_creator_enforcement_is_not_reported_active_before_reconciliation() -> AppResult<()>
{
    let (state, creator) = setup_test_state().await?;
    let before_state = fetch_creator_enforcement_state(state.db.sqlite_adapter(), &creator).await?;
    let expired_at = (Utc::now() - chrono::Duration::minutes(5)).to_rfc3339();
    let action_id = format!("test-cea-{}", Uuid::new_v4().simple());
    let created_at = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO creator_enforcement_actions (
            id, creator_id, scope, state, reason, resolution_note, created_by_user_id,
            released_by_user_id, created_at, released_at, expires_at
        ) VALUES (?, ?, 'collaboration', 'active', ?, NULL, ?, NULL, ?, NULL, ?)
        "#,
    )
    .bind(&action_id)
    .bind(&creator.id)
    .bind("expired enforcement")
    .bind(&creator.user_id)
    .bind(&created_at)
    .bind(&expired_at)
    .execute(state.db.sqlite_adapter())
    .await?;

    let enforcement_state =
        fetch_creator_enforcement_state(state.db.sqlite_adapter(), &creator).await?;

    assert_eq!(
        enforcement_state.active_actions.len(),
        before_state.active_actions.len()
    );
    assert_eq!(
        enforcement_state.collaboration_enabled,
        before_state.collaboration_enabled
    );
    assert_eq!(
        enforcement_state.history.len(),
        before_state.history.len() + 1
    );
    assert!(
        enforcement_state
            .history
            .iter()
            .any(|action| action.id == action_id && action.state == "expired")
    );
    assert!(
        enforcement_state
            .active_actions
            .iter()
            .all(|action| action.id != action_id)
    );
    let by_id =
        fetch_creator_enforcement_action_by_id(state.db.sqlite_adapter(), &action_id).await?;
    let stored_state: String =
        sqlx::query("SELECT state FROM creator_enforcement_actions WHERE id = ?")
            .bind(&action_id)
            .fetch_one(state.db.sqlite_adapter())
            .await?
            .get("state");
    let audit = fetch_moderation_audit_log(state.db.sqlite_adapter(), &creator.id, None).await?;
    let notifications = fetch_notifications_rows(state.db.sqlite_adapter(), &creator.id).await?;
    assert_eq!(by_id.state, "expired");
    assert_eq!(stored_state, "expired");
    assert!(
        enforcement_state
            .history
            .iter()
            .any(|action| action.id == action_id && action.state == "expired")
    );
    assert!(audit.iter().any(|entry| {
        entry.event_type == "creator_enforcement_expired"
            && entry.payload["actionId"] == action_id.as_str()
    }));
    assert!(notifications.iter().any(|entry| {
        entry.kind == "creator_enforcement_expired" && entry.id.starts_with("notd-")
    }));
    Ok(())
}

#[tokio::test]
async fn expired_creator_enforcement_read_self_heals_operational_state_and_by_id() -> AppResult<()>
{
    let (state, creator) = setup_test_state().await?;
    let action_id = format!("test-cea-op-{}", Uuid::new_v4().simple());
    let created_at = (Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
    let expired_at = (Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO creator_enforcement_actions (
            id, creator_id, scope, state, reason, resolution_note, created_by_user_id,
            released_by_user_id, created_at, released_at, expires_at
        ) VALUES (?, ?, 'uploads', 'active', ?, NULL, ?, NULL, ?, NULL, ?)
        "#,
    )
    .bind(&action_id)
    .bind(&creator.id)
    .bind("expired upload enforcement")
    .bind(&creator.user_id)
    .bind(&created_at)
    .bind(&expired_at)
    .execute(state.db.sqlite_adapter())
    .await?;

    let operational = fetch_creator_operational_state(state.db.sqlite_adapter(), &creator).await?;
    let by_id =
        fetch_creator_enforcement_action_by_id(state.db.sqlite_adapter(), &action_id).await?;
    let stored_state: String =
        sqlx::query("SELECT state FROM creator_enforcement_actions WHERE id = ?")
            .bind(&action_id)
            .fetch_one(state.db.sqlite_adapter())
            .await?
            .get("state");
    let audit = fetch_moderation_audit_log(state.db.sqlite_adapter(), &creator.id, None).await?;
    let notifications = fetch_notifications_rows(state.db.sqlite_adapter(), &creator.id).await?;

    assert!(operational.upload_ingest_enabled);
    assert!(
        operational
            .active_enforcement_actions
            .iter()
            .all(|action| action.id != action_id)
    );
    assert_eq!(by_id.state, "expired");
    assert_eq!(stored_state, "expired");
    assert_eq!(
        audit
            .iter()
            .filter(|entry| {
                entry.event_type == "creator_enforcement_expired"
                    && entry.payload["actionId"] == action_id.as_str()
            })
            .count(),
        1
    );
    assert_eq!(
        notifications
            .iter()
            .filter(|entry| entry.kind == "creator_enforcement_expired")
            .count(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn admin_can_inspect_and_reconcile_creator_enforcement_action_by_id() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);
    let action_id = format!("test-cea-admin-{}", Uuid::new_v4().simple());
    let created_at = (Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
    let expired_at = (Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO creator_enforcement_actions (
            id, creator_id, scope, state, reason, resolution_note, created_by_user_id,
            released_by_user_id, created_at, released_at, expires_at
        ) VALUES (?, ?, 'uploads', 'active', ?, NULL, ?, NULL, ?, NULL, ?)
        "#,
    )
    .bind(&action_id)
    .bind(&creator.id)
    .bind("admin reconciliation check")
    .bind(&creator.user_id)
    .bind(&created_at)
    .bind(&expired_at)
    .execute(state.db.sqlite_adapter())
    .await?;

    let inspected = get_admin_creator_enforcement_action(
        State(state.clone()),
        headers.clone(),
        Path((creator.id.clone(), action_id.clone())),
    )
    .await?;
    let stored_before: String =
        sqlx::query("SELECT state FROM creator_enforcement_actions WHERE id = ?")
            .bind(&action_id)
            .fetch_one(state.db.sqlite_adapter())
            .await?
            .get("state");

    assert_eq!(inspected.id, action_id);
    assert_eq!(inspected.state, "active");
    assert_eq!(stored_before, "active");

    let report = reconcile_admin_creator_enforcement_action(
        State(state.clone()),
        headers,
        Path((creator.id.clone(), action_id.clone())),
    )
    .await?
    .0;
    let stored_after: String =
        sqlx::query("SELECT state FROM creator_enforcement_actions WHERE id = ?")
            .bind(&action_id)
            .fetch_one(state.db.sqlite_adapter())
            .await?
            .get("state");

    assert_eq!(report.action_id, action_id);
    assert_eq!(report.action.id, action_id);
    assert_eq!(report.action.creator_id, creator.id);
    assert_eq!(report.action.state, "expired");
    assert_eq!(stored_after, "expired");
    assert_eq!(report.actions.len(), 1);
    assert_eq!(report.actions[0].action_type, "action_expired");
    assert_eq!(report.actions[0].previous_state.as_deref(), Some("active"));
    assert_eq!(report.actions[0].next_state.as_deref(), Some("expired"));
    Ok(())
}
