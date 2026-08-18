use super::*;

#[tokio::test]
async fn creator_live_authoritative_reads_reconcile_expired_collaboration_truth() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let host_token = insert_creator_auth_session(&state.pool, &creator).await?;
    let host_headers = auth_headers(&host_token);
    let collab_creator = fetch_creator_profile(&state.pool, "crt-atlas").await?;
    let collab_token = insert_creator_auth_session(&state.pool, &collab_creator).await?;
    let collab_headers = auth_headers(&collab_token);
    let broadcast = insert_ready_collaboration_broadcast(&state.pool, &creator).await?;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO users (id, handle, display_name, avatar, tier, joined_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind("usr-pending-live-summary")
    .bind("pending_live_summary")
    .bind("Pending Live Summary")
    .bind("https://cdn.lifestream.local/avatar/pending-live-summary.jpg")
    .bind("free")
    .bind(&now)
    .execute(&state.pool)
    .await?;

    let session = create_collaboration_session(
        State(state.clone()),
        host_headers.clone(),
        Json(CreateCollaborationSessionRequest {
            broadcast_id: Some(broadcast.id.clone()),
            title: Some("creator live authoritative state".to_string()),
            chat_mode: Some("shared".to_string()),
            recording_policy: Some("host_archive".to_string()),
        }),
    )
    .await?
    .0;

    let pending_invite = create_collaboration_invite(
        State(state.clone()),
        host_headers.clone(),
        Path(session.id.clone()),
        Json(CreateCollaborationInviteRequest {
            invitee_user_id: "usr-pending-live-summary".to_string(),
            role: "guest".to_string(),
            mirror_to_guest_channel: false,
            message: Some("pending for creator live summary".to_string()),
            expires_in_minutes: Some(30),
        }),
    )
    .await?
    .0;

    let accepted_invite = create_collaboration_invite(
        State(state.clone()),
        host_headers.clone(),
        Path(session.id.clone()),
        Json(CreateCollaborationInviteRequest {
            invitee_user_id: collab_creator.user_id.clone(),
            role: "guest".to_string(),
            mirror_to_guest_channel: true,
            message: Some("grant candidate".to_string()),
            expires_in_minutes: Some(30),
        }),
    )
    .await?
    .0;
    let participant = accept_collaboration_invite(
        State(state.clone()),
        collab_headers,
        Path(accepted_invite.id.clone()),
    )
    .await?
    .0;
    let participant = update_collaboration_participant(
        State(state.clone()),
        host_headers.clone(),
        Path((session.id.clone(), participant.id.clone())),
        Json(UpdateCollaborationParticipantRequest {
            state: Some("live".to_string()),
            publish_to_host: Some(true),
            mirror_to_guest_channel: Some(true),
            can_speak_in_chat: Some(true),
        }),
    )
    .await?
    .0;
    let grant =
        issue_mirror_grant_for_participant(&state, &session, &participant, &creator.user_id)
            .await?;

    let expired_at = (Utc::now() - chrono::Duration::minutes(5)).to_rfc3339();
    sqlx::query(
        "UPDATE collaboration_invites SET expires_at = ?, state = 'pending', responded_at = NULL WHERE id = ?",
    )
    .bind(&expired_at)
    .bind(&pending_invite.id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE collaboration_mirror_grants SET expires_at = ?, state = 'issued', revoked_at = NULL, activated_at = NULL WHERE id = ?",
    )
    .bind(&expired_at)
    .bind(&grant.id)
    .execute(&state.pool)
    .await?;

    let (mut subscription, _) = state
        .realtime
        .join(&creator_live_channel_id(&creator.id))
        .await;

    let control = fetch_authoritative_creator_live_control_response(&state, &creator.id).await?;
    let runtime = fetch_authoritative_creator_live_runtime_response(&state, &creator.id).await?;
    let refreshed_invite =
        fetch_collaboration_invite_by_id(&state.pool, &pending_invite.id).await?;
    let refreshed_grant = fetch_collaboration_mirror_grant_by_id(&state.pool, &grant.id).await?;
    publish_creator_live_state(&state, &creator.id).await?;
    let published = tokio::time::timeout(Duration::from_secs(1), subscription.recv())
        .await
        .map_err(|_| {
            AppError::Internal("timed out waiting for creator live state publication".to_string())
        })?
        .map_err(|error| {
            AppError::Internal(format!(
                "failed receiving creator live state publication: {error}"
            ))
        })?;

    assert_eq!(refreshed_invite.state, "expired");
    assert_eq!(refreshed_grant.state, "expired");
    assert_eq!(control.collaboration.pending_invite_count, 0);
    assert_eq!(control.collaboration.issued_grant_count, 0);
    assert_eq!(
        control
            .collaboration
            .active_control
            .as_ref()
            .map(|item| item.pending_invite_count),
        Some(0)
    );
    assert_eq!(
        control
            .collaboration
            .active_control
            .as_ref()
            .map(|item| item.issued_grant_count),
        Some(0)
    );
    assert!(
        runtime
            .collaboration
            .active_control
            .as_ref()
            .is_some_and(|item| item
                .runtime
                .grants
                .iter()
                .any(|current| { current.id == grant.id && current.state == "expired" }))
    );
    match published {
        WsEvent::CreatorLiveState { control, runtime } => {
            assert_eq!(control.collaboration.pending_invite_count, 0);
            assert_eq!(control.collaboration.issued_grant_count, 0);
            assert_eq!(
                control
                    .collaboration
                    .active_control
                    .as_ref()
                    .map(|item| item.pending_invite_count),
                Some(0)
            );
            assert_eq!(
                control
                    .collaboration
                    .active_control
                    .as_ref()
                    .map(|item| item.issued_grant_count),
                Some(0)
            );
            assert!(
                runtime
                    .collaboration
                    .active_control
                    .as_ref()
                    .is_some_and(|item| item
                        .runtime
                        .grants
                        .iter()
                        .any(|current| { current.id == grant.id && current.state == "expired" }))
            );
        }
        other => panic!("unexpected event: {other:?}"),
    }

    state
        .realtime
        .leave(&creator_live_channel_id(&creator.id))
        .await;
    Ok(())
}

#[tokio::test]
async fn metrics_report_creator_live_socket_presence_from_reconciled_state() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let stale_seen_at =
        (Utc::now() - chrono::Duration::seconds(WS_PRESENCE_TTL_SECONDS + 30)).to_rfc3339();
    let fresh_seen_at = Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO creator_live_socket_sessions (
            id, creator_id, user_id, session_token_hash, connected_at, last_seen_at, disconnected_at
        ) VALUES (?, ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(format!("cls-{}", Uuid::new_v4().simple()))
    .bind(&creator.id)
    .bind("usr-1")
    .bind(hash_token("creator-live-stale-metric"))
    .bind(&stale_seen_at)
    .bind(&stale_seen_at)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO creator_live_socket_sessions (
            id, creator_id, user_id, session_token_hash, connected_at, last_seen_at, disconnected_at
        ) VALUES (?, ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(format!("cls-{}", Uuid::new_v4().simple()))
    .bind(&creator.id)
    .bind("usr-1")
    .bind(hash_token("creator-live-fresh-metric"))
    .bind(&fresh_seen_at)
    .bind(&fresh_seen_at)
    .execute(&state.pool)
    .await?;

    let response = metrics(State(state.clone())).await?;
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let text =
        String::from_utf8(body.to_vec()).map_err(|error| AppError::Internal(error.to_string()))?;
    let stale_disconnected_at: Option<String> = sqlx::query(
        "SELECT disconnected_at FROM creator_live_socket_sessions WHERE creator_id = ? AND user_id = ? AND session_token_hash = ?",
    )
    .bind(&creator.id)
    .bind("usr-1")
    .bind(hash_token("creator-live-stale-metric"))
    .fetch_one(&state.pool)
    .await?
    .get("disconnected_at");

    assert!(stale_disconnected_at.is_some());
    assert!(text.contains("lifestream_presence_creator_live_sockets 1"));
    assert!(text.contains("lifestream_live_ingest_active_sessions"));
    assert!(text.contains("lifestream_live_ingest_ready_outputs"));
    assert!(text.contains("lifestream_live_ingest_artifact_attention_outputs"));
    assert!(text.contains("lifestream_live_ingest_manifest_path_missing_outputs"));
    assert!(text.contains("lifestream_live_ingest_archive_path_missing_outputs"));
    assert!(text.contains("lifestream_live_ingest_advisory_critical_samples"));
    assert!(text.contains("lifestream_live_ingest_peak_host_channel_targets"));
    assert!(text.contains("lifestream_live_ingest_peak_mirror_channel_targets"));
    assert!(text.contains("lifestream_live_ingest_peak_archive_targets"));
    assert!(text.contains("lifestream_live_ingest_peak_active_targets"));
    assert!(text.contains("lifestream_live_ingest_peak_degraded_targets"));
    assert!(text.contains("lifestream_live_ingest_peak_armed_targets"));
    assert!(text.contains("lifestream_live_ingest_peak_pending_source_targets"));
    Ok(())
}

#[tokio::test]
async fn live_viewer_presence_counts_distinct_viewers_not_socket_tabs() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let now = Utc::now().to_rfc3339();
    let stream_id = insert_live_stream_for_creator(&state.pool, &creator).await?;

    sqlx::query(
        r#"
        INSERT INTO live_viewer_sessions (
            id, stream_id, user_id, session_token_hash, connected_at, last_seen_at, disconnected_at
        ) VALUES (?, ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(format!("lvs-test-{}", Uuid::new_v4().simple()))
    .bind(&stream_id)
    .bind(Some("usr-2"))
    .bind(hash_token("viewer-tab-1"))
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO live_viewer_sessions (
            id, stream_id, user_id, session_token_hash, connected_at, last_seen_at, disconnected_at
        ) VALUES (?, ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(format!("lvs-test-{}", Uuid::new_v4().simple()))
    .bind(&stream_id)
    .bind(Some("usr-2"))
    .bind(hash_token("viewer-tab-2"))
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO live_viewer_sessions (
            id, stream_id, user_id, session_token_hash, connected_at, last_seen_at, disconnected_at
        ) VALUES (?, ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(format!("lvs-test-{}", Uuid::new_v4().simple()))
    .bind(&stream_id)
    .bind(Option::<&str>::None)
    .bind(hash_token("anon-viewer"))
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;

    let per_stream = count_active_live_viewer_sessions(&state.pool, &stream_id).await?;
    let all_active = count_all_active_live_viewer_sessions(&state.pool).await?;
    let preview = get_live_viewer_preview(State(state.clone()), Path(stream_id.to_string()))
        .await?
        .0;

    assert_eq!(per_stream, 2);
    assert!(all_active >= 2);
    assert_eq!(preview.total_viewers, 2);
    assert_eq!(preview.sample_users, vec!["atlas_codes".to_string()]);
    Ok(())
}

#[tokio::test]
async fn clip_requests_dedupe_within_active_window() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_user_auth_session(&state.pool, "usr-viewer", &["user"]).await?;
    let stream_id = insert_live_stream_for_creator(&state.pool, &creator).await?;

    create_clip_request(
        State(state.clone()),
        auth_headers(&token),
        Path(stream_id.clone()),
    )
    .await?;
    create_clip_request(
        State(state.clone()),
        auth_headers(&token),
        Path(stream_id.clone()),
    )
    .await?;

    let clip_count: i64 = sqlx::query(
        "SELECT COUNT(*) AS count FROM live_stream_clip_requests WHERE stream_id = ? AND user_id = ?",
    )
    .bind(&stream_id)
    .bind("usr-viewer")
    .fetch_one(&state.pool)
    .await?
    .get("count");
    assert_eq!(clip_count, 1);

    sqlx::query(
        "UPDATE live_stream_clip_requests SET created_at = ? WHERE stream_id = ? AND user_id = ?",
    )
    .bind((Utc::now() - chrono::Duration::seconds(31)).to_rfc3339())
    .bind(&stream_id)
    .bind("usr-viewer")
    .execute(&state.pool)
    .await?;

    create_clip_request(
        State(state.clone()),
        auth_headers(&token),
        Path(stream_id.clone()),
    )
    .await?;

    let refreshed_clip_count: i64 = sqlx::query(
        "SELECT COUNT(*) AS count FROM live_stream_clip_requests WHERE stream_id = ? AND user_id = ?",
    )
    .bind(&stream_id)
    .bind("usr-viewer")
    .fetch_one(&state.pool)
    .await?
    .get("count");
    assert_eq!(refreshed_clip_count, 2);
    Ok(())
}

#[tokio::test]
async fn live_stream_listing_orders_by_effective_viewers_not_stale_snapshot() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let other_creator = fetch_creator_profile(&state.pool, "crt-atlas").await?;
    reset_creator_live_state(&state.pool, &other_creator).await?;

    let primary_stream_id = insert_live_stream_for_creator(&state.pool, &creator).await?;
    let secondary_stream_id = insert_live_stream_for_creator(&state.pool, &other_creator).await?;
    let now = Utc::now().to_rfc3339();

    sqlx::query("UPDATE live_streams SET viewers = 500 WHERE id = ?")
        .bind(&primary_stream_id)
        .execute(&state.pool)
        .await?;
    sqlx::query("UPDATE live_streams SET viewers = 5 WHERE id = ?")
        .bind(&secondary_stream_id)
        .execute(&state.pool)
        .await?;

    for token in ["atlas-viewer-a", "atlas-viewer-b", "atlas-viewer-c"] {
        sqlx::query(
            r#"
            INSERT INTO live_viewer_sessions (
                id, stream_id, user_id, session_token_hash, connected_at, last_seen_at, disconnected_at
            ) VALUES (?, ?, ?, ?, ?, ?, NULL)
            "#,
        )
        .bind(format!("lvs-test-{}", Uuid::new_v4().simple()))
        .bind(&secondary_stream_id)
        .bind(Option::<&str>::None)
        .bind(hash_token(token))
        .bind(&now)
        .bind(&now)
        .execute(&state.pool)
        .await?;
    }

    let streams = fetch_live_streams(&state.pool, None).await?;
    assert!(streams.len() >= 2);
    assert_eq!(streams[0].id, primary_stream_id);
    assert_eq!(streams[0].viewers, 500);
    assert_eq!(streams[1].id, secondary_stream_id);
    assert_eq!(streams[1].viewers, 5);

    sqlx::query("UPDATE live_streams SET viewers = 1 WHERE id = ?")
        .bind(&primary_stream_id)
        .execute(&state.pool)
        .await?;
    sqlx::query("UPDATE live_streams SET viewers = 0 WHERE id = ?")
        .bind(&secondary_stream_id)
        .execute(&state.pool)
        .await?;

    let refreshed = fetch_live_streams(&state.pool, None).await?;
    assert!(refreshed.len() >= 2);
    assert_eq!(refreshed[0].id, secondary_stream_id);
    assert_eq!(refreshed[0].viewers, 3);
    assert_eq!(refreshed[1].id, primary_stream_id);
    assert_eq!(refreshed[1].viewers, 1);
    Ok(())
}

#[tokio::test]
async fn category_live_totals_follow_active_stream_truth_not_snapshot_columns() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let other_creator = fetch_creator_profile(&state.pool, "crt-atlas").await?;
    reset_creator_live_state(&state.pool, &other_creator).await?;

    sqlx::query("UPDATE creator_profiles SET default_category = 'Gaming' WHERE id = ?")
        .bind(&creator.id)
        .execute(&state.pool)
        .await?;
    sqlx::query("UPDATE creator_profiles SET default_category = 'Music' WHERE id = ?")
        .bind(&other_creator.id)
        .execute(&state.pool)
        .await?;

    let refreshed_creator = fetch_creator_profile(&state.pool, &creator.id).await?;
    let refreshed_other_creator = fetch_creator_profile(&state.pool, &other_creator.id).await?;
    let gaming_stream_id = insert_live_stream_for_creator(&state.pool, &refreshed_creator).await?;
    let music_stream_id =
        insert_live_stream_for_creator(&state.pool, &refreshed_other_creator).await?;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE categories SET live_viewers = 9999, live_channels = 99 WHERE slug = 'gaming'",
    )
    .execute(&state.pool)
    .await?;
    sqlx::query("UPDATE categories SET live_viewers = 1, live_channels = 1 WHERE slug = 'music'")
        .execute(&state.pool)
        .await?;
    sqlx::query("UPDATE live_streams SET viewers = 0 WHERE id = ?")
        .bind(&gaming_stream_id)
        .execute(&state.pool)
        .await?;
    sqlx::query("UPDATE live_streams SET viewers = 2 WHERE id = ?")
        .bind(&music_stream_id)
        .execute(&state.pool)
        .await?;

    for token in ["gaming-viewer-a", "gaming-viewer-b", "gaming-viewer-c"] {
        sqlx::query(
            r#"
            INSERT INTO live_viewer_sessions (
                id, stream_id, user_id, session_token_hash, connected_at, last_seen_at, disconnected_at
            ) VALUES (?, ?, ?, ?, ?, ?, NULL)
            "#,
        )
        .bind(format!("lvs-test-{}", Uuid::new_v4().simple()))
        .bind(&gaming_stream_id)
        .bind(Option::<&str>::None)
        .bind(hash_token(token))
        .bind(&now)
        .bind(&now)
        .execute(&state.pool)
        .await?;
    }

    let categories = fetch_categories(&state.pool).await?;
    let gaming = categories
        .iter()
        .find(|category| category.slug == "gaming")
        .expect("gaming category present");
    let music = categories
        .iter()
        .find(|category| category.slug == "music")
        .expect("music category present");

    assert_eq!(gaming.live_viewers, 3);
    assert_eq!(gaming.live_channels, 1);
    assert_eq!(music.live_viewers, 2);
    assert_eq!(music.live_channels, 1);
    assert_eq!(categories[0].slug, "gaming");

    let gaming_detail = fetch_category_by_slug(&state.pool, "gaming").await?;
    assert_eq!(gaming_detail.live_viewers, 3);
    assert_eq!(gaming_detail.live_channels, 1);
    Ok(())
}

#[tokio::test]
async fn live_viewer_resume_token_cannot_be_rebound_to_another_user() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let stream_id = insert_live_stream_for_creator(&state.pool, &creator).await?;
    let first_identity = RequestIdentity {
        session_id: "viewer-session-a".to_string(),
        user_id: "usr-2".to_string(),
        creator_id: Some("crt-atlas".to_string()),
        scopes: vec!["user".to_string(), "creator".to_string()],
    };
    let second_identity = RequestIdentity {
        session_id: "viewer-session-b".to_string(),
        user_id: "usr-1".to_string(),
        creator_id: Some("crt-deepsaint".to_string()),
        scopes: vec!["user".to_string(), "creator".to_string()],
    };

    let (session_token, resumed, _) =
        register_live_viewer_session(&state.pool, &stream_id, Some(&first_identity), None).await?;
    assert!(!resumed);

    let error = register_live_viewer_session(
        &state.pool,
        &stream_id,
        Some(&second_identity),
        Some(&session_token),
    )
    .await
    .expect_err("resume token must stay bound to the original authenticated viewer");

    assert!(matches!(error, AppError::Forbidden));
    Ok(())
}

#[tokio::test]
async fn anonymous_live_viewer_resume_token_can_upgrade_to_authenticated_user() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let stream_id = insert_live_stream_for_creator(&state.pool, &creator).await?;
    let identity = RequestIdentity {
        session_id: "viewer-session-upgrade".to_string(),
        user_id: "usr-2".to_string(),
        creator_id: Some("crt-atlas".to_string()),
        scopes: vec!["user".to_string(), "creator".to_string()],
    };

    let (session_token, resumed, _) =
        register_live_viewer_session(&state.pool, &stream_id, None, None).await?;
    assert!(!resumed);

    let (_same_token, resumed, _) = register_live_viewer_session(
        &state.pool,
        &stream_id,
        Some(&identity),
        Some(&session_token),
    )
    .await?;
    assert!(resumed);

    let row = sqlx::query(
        "SELECT user_id FROM live_viewer_sessions WHERE stream_id = ? AND session_token_hash = ?",
    )
    .bind(&stream_id)
    .bind(hash_token(&session_token))
    .fetch_one(&state.pool)
    .await?;
    let bound_user_id: Option<String> = row.get("user_id");
    assert_eq!(bound_user_id.as_deref(), Some("usr-2"));
    Ok(())
}

#[tokio::test]
async fn stale_live_viewer_disconnect_cannot_tombstone_resumed_session() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let stream_id = insert_live_stream_for_creator(&state.pool, &creator).await?;
    let identity = RequestIdentity {
        session_id: "viewer-session-race".to_string(),
        user_id: "usr-2".to_string(),
        creator_id: Some("crt-atlas".to_string()),
        scopes: vec!["user".to_string(), "creator".to_string()],
    };

    let (session_token, resumed, first_lease) =
        register_live_viewer_session(&state.pool, &stream_id, Some(&identity), None).await?;
    assert!(!resumed);
    tokio::time::sleep(Duration::from_millis(5)).await;
    let (_same_token, resumed, second_lease) = register_live_viewer_session(
        &state.pool,
        &stream_id,
        Some(&identity),
        Some(&session_token),
    )
    .await?;
    assert!(resumed);
    assert_ne!(first_lease, second_lease);

    disconnect_live_viewer_session(&state.pool, &stream_id, &session_token, &first_lease).await?;

    let row = sqlx::query(
        "SELECT connected_at, disconnected_at FROM live_viewer_sessions WHERE stream_id = ? AND session_token_hash = ?",
    )
    .bind(&stream_id)
    .bind(hash_token(&session_token))
    .fetch_one(&state.pool)
    .await?;
    let connected_at: String = row.get("connected_at");
    let disconnected_at: Option<String> = row.get("disconnected_at");
    assert_eq!(connected_at, second_lease);
    assert!(disconnected_at.is_none());
    Ok(())
}

#[tokio::test]
async fn stale_creator_live_disconnect_cannot_tombstone_resumed_session() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;

    let (session_token, resumed, first_lease) =
        register_creator_live_socket_session(&state.pool, &creator.id, "usr-1", None).await?;
    assert!(!resumed);
    tokio::time::sleep(Duration::from_millis(5)).await;
    let (_same_token, resumed, second_lease) = register_creator_live_socket_session(
        &state.pool,
        &creator.id,
        "usr-1",
        Some(&session_token),
    )
    .await?;
    assert!(resumed);
    assert_ne!(first_lease, second_lease);

    disconnect_creator_live_socket_session(&state.pool, &creator.id, &session_token, &first_lease)
        .await?;

    let row = sqlx::query(
        "SELECT connected_at, disconnected_at FROM creator_live_socket_sessions WHERE creator_id = ? AND user_id = ? AND session_token_hash = ?",
    )
    .bind(&creator.id)
    .bind("usr-1")
    .bind(hash_token(&session_token))
    .fetch_one(&state.pool)
    .await?;
    let connected_at: String = row.get("connected_at");
    let disconnected_at: Option<String> = row.get("disconnected_at");
    assert_eq!(connected_at, second_lease);
    assert!(disconnected_at.is_none());
    Ok(())
}

#[tokio::test]
async fn stale_collaboration_disconnect_cannot_tombstone_resumed_session() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let (_session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;
    let identity = RequestIdentity {
        session_id: "collab-session-race".to_string(),
        user_id: "usr-2".to_string(),
        creator_id: Some("crt-atlas".to_string()),
        scopes: vec!["user".to_string(), "creator".to_string()],
    };
    let session_view =
        fetch_current_collaboration_socket_session_view(&state, &participant.session_id, &identity)
            .await?;

    let (session_token, resumed, first_lease) =
        register_collaboration_socket_session(&state.pool, &session_view, &identity, None).await?;
    assert!(!resumed);
    tokio::time::sleep(Duration::from_millis(5)).await;
    let (_same_token, resumed, second_lease) = register_collaboration_socket_session(
        &state.pool,
        &session_view,
        &identity,
        Some(&session_token),
    )
    .await?;
    assert!(resumed);
    assert_ne!(first_lease, second_lease);

    disconnect_collaboration_socket_session(
        &state.pool,
        &participant.session_id,
        &session_token,
        &first_lease,
    )
    .await?;

    let row = sqlx::query(
        "SELECT connected_at, disconnected_at FROM collaboration_socket_sessions WHERE collaboration_session_id = ? AND user_id = ? AND session_token_hash = ?",
    )
    .bind(&participant.session_id)
    .bind("usr-2")
    .bind(hash_token(&session_token))
    .fetch_one(&state.pool)
    .await?;
    let connected_at: String = row.get("connected_at");
    let disconnected_at: Option<String> = row.get("disconnected_at");
    assert_eq!(connected_at, second_lease);
    assert!(disconnected_at.is_none());
    Ok(())
}
