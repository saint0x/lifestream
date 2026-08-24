use super::*;

#[tokio::test]
async fn creator_can_inspect_and_reconcile_live_moderation_action_by_id() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);
    let stream_id = insert_live_stream_for_creator(state.db.sqlite_adapter(), &creator).await?;
    let action_id = format!("test-lma-admin-{}", Uuid::new_v4().simple());
    let created_at = (Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
    let expired_at = (Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO live_moderation_actions (
            id, stream_id, creator_id, subject_user_id, actor_user_id, action_type, reason,
            state, expires_at, created_at, revoked_at
        ) VALUES (?, ?, ?, ?, ?, 'mute', ?, 'active', ?, ?, NULL)
        "#,
    )
    .bind(&action_id)
    .bind(&stream_id)
    .bind(&creator.id)
    .bind("usr-2")
    .bind(&creator.user_id)
    .bind("creator moderation reconciliation check")
    .bind(&expired_at)
    .bind(&created_at)
    .execute(state.db.sqlite_adapter())
    .await?;

    let inspected = get_live_moderation_action(
        State(state.clone()),
        headers.clone(),
        Path((stream_id.clone(), action_id.clone())),
    )
    .await?
    .0;
    let stored_before: String =
        sqlx::query("SELECT state FROM live_moderation_actions WHERE id = ?")
            .bind(&action_id)
            .fetch_one(state.db.sqlite_adapter())
            .await?
            .get("state");

    assert_eq!(inspected.id, action_id);
    assert_eq!(inspected.state, "active");
    assert_eq!(stored_before, "active");

    let report = reconcile_live_moderation_action(
        State(state.clone()),
        headers,
        Path((stream_id.clone(), action_id.clone())),
    )
    .await?
    .0;
    let stored_after: String =
        sqlx::query("SELECT state FROM live_moderation_actions WHERE id = ?")
            .bind(&action_id)
            .fetch_one(state.db.sqlite_adapter())
            .await?
            .get("state");

    assert_eq!(report.action_id, action_id);
    assert_eq!(report.action.id, action_id);
    assert_eq!(report.action.stream_id, stream_id);
    assert_eq!(report.action.creator_id, creator.id);
    assert_eq!(report.action.state, "expired");
    assert_eq!(stored_after, "expired");
    assert_eq!(report.actions.len(), 1);
    assert_eq!(report.actions[0].action_type, "action_expired");
    assert_eq!(report.actions[0].previous_state.as_deref(), Some("active"));
    assert_eq!(report.actions[0].next_state.as_deref(), Some("expired"));
    Ok(())
}

#[tokio::test]
async fn expired_live_moderation_action_is_reported_expired_before_reconciliation() -> AppResult<()>
{
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);
    let stream_id = insert_live_stream_for_creator(state.db.sqlite_adapter(), &creator).await?;
    let action_id = format!("test-lma-{}", Uuid::new_v4().simple());
    let created_at = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
    let expired_at = (Utc::now() - chrono::Duration::minutes(5)).to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO live_moderation_actions (
            id, stream_id, creator_id, subject_user_id, actor_user_id, action_type, reason,
            state, expires_at, created_at, revoked_at
        ) VALUES (?, ?, ?, ?, ?, 'mute', ?, 'active', ?, ?, NULL)
        "#,
    )
    .bind(&action_id)
    .bind(&stream_id)
    .bind(&creator.id)
    .bind("usr-2")
    .bind(&creator.user_id)
    .bind("expired moderation")
    .bind(&expired_at)
    .bind(&created_at)
    .execute(state.db.sqlite_adapter())
    .await?;

    let by_id = fetch_live_moderation_action_by_id(state.db.sqlite_adapter(), &action_id).await?;
    let listed = list_live_moderation_actions(State(state.clone()), headers, Path(stream_id))
        .await?
        .0;
    let stored_state: String =
        sqlx::query("SELECT state FROM live_moderation_actions WHERE id = ?")
            .bind(&action_id)
            .fetch_one(state.db.sqlite_adapter())
            .await?
            .get("state");

    assert_eq!(by_id.state, "expired");
    assert_eq!(stored_state, "expired");
    assert!(
        listed
            .iter()
            .any(|action| action.id == action_id && action.state == "expired")
    );
    Ok(())
}

#[tokio::test]
async fn expired_live_moderation_lookup_self_heals_stored_state_and_active_gate() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let stream_id = insert_live_stream_for_creator(state.db.sqlite_adapter(), &creator).await?;
    let action_id = format!("test-lma-{}", Uuid::new_v4().simple());
    let created_at = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
    let expired_at = (Utc::now() - chrono::Duration::minutes(5)).to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO live_moderation_actions (
            id, stream_id, creator_id, subject_user_id, actor_user_id, action_type, reason,
            state, expires_at, created_at, revoked_at
        ) VALUES (?, ?, ?, ?, ?, 'mute', ?, 'active', ?, ?, NULL)
        "#,
    )
    .bind(&action_id)
    .bind(&stream_id)
    .bind(&creator.id)
    .bind("usr-2")
    .bind(&creator.user_id)
    .bind("expired moderation gate")
    .bind(&expired_at)
    .bind(&created_at)
    .execute(state.db.sqlite_adapter())
    .await?;

    let active =
        fetch_active_live_moderation_action(state.db.sqlite_adapter(), &stream_id, "usr-2").await?;
    let stored = fetch_live_moderation_action_by_id(state.db.sqlite_adapter(), &action_id).await?;
    let audit =
        fetch_moderation_audit_log(state.db.sqlite_adapter(), &creator.id, Some(&stream_id))
            .await?;

    assert!(active.is_none());
    assert_eq!(stored.state, "expired");
    assert!(audit.iter().any(|entry| {
        entry.event_type == "moderation_action_expired"
            && entry.payload["actionId"] == Value::String(action_id.clone())
    }));
    Ok(())
}

#[tokio::test]
async fn media_root_writable_probe_accepts_writable_directory() -> AppResult<()> {
    let media_root = std::env::temp_dir().join(format!("vanta-health-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&media_root).await?;

    let status = check_media_root_writable(&media_root).await;

    assert!(status.ready);
    assert!(status.detail.contains("is writable"));
    Ok(())
}

#[tokio::test]
async fn codec_binary_probe_rejects_missing_binary() -> AppResult<()> {
    let status = check_binary_available("binary-that-does-not-exist-vanta").await;

    assert!(!status.ready);
    assert!(status.detail.contains("is unavailable"));
    Ok(())
}

#[tokio::test]
async fn runtime_dependencies_fail_closed_when_codec_binary_is_missing() -> AppResult<()> {
    let (state, _) = setup_test_state().await?;
    state.background_worker.mark_success().await;

    let runtime = check_runtime_dependencies_with_binaries(
        state.as_ref(),
        "ffmpeg",
        "binary-that-does-not-exist-vanta",
    )
    .await;

    assert!(runtime.database);
    assert!(runtime.dependencies.media_root.ready);
    assert!(runtime.dependencies.ffmpeg.ready);
    assert!(!runtime.dependencies.ffprobe.ready);
    assert!(!runtime.ready);
    Ok(())
}

#[tokio::test]
async fn runtime_dependencies_fail_closed_when_background_worker_never_succeeded() -> AppResult<()>
{
    let (state, _) = setup_test_state().await?;

    let runtime =
        check_runtime_dependencies_with_binaries(state.as_ref(), "ffmpeg", "ffprobe").await;

    assert!(runtime.database);
    assert!(!runtime.dependencies.background_worker.ready);
    assert!(
        runtime
            .dependencies
            .background_worker
            .detail
            .contains("has not completed")
    );
    assert!(!runtime.ready);
    Ok(())
}

#[tokio::test]
async fn runtime_dependencies_fail_closed_when_background_worker_is_stale() -> AppResult<()> {
    let (state, _) = setup_test_state().await?;
    state.background_worker.mark_success().await;
    tokio::time::sleep(Duration::from_secs(
        BACKGROUND_WORKER_STALE_AFTER_SECONDS + 1,
    ))
    .await;

    let runtime =
        check_runtime_dependencies_with_binaries(state.as_ref(), "ffmpeg", "ffprobe").await;

    assert!(!runtime.dependencies.background_worker.ready);
    assert!(
        runtime
            .dependencies
            .background_worker
            .detail
            .contains("stale")
    );
    assert!(!runtime.ready);
    Ok(())
}
