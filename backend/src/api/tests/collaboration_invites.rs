use super::*;

#[tokio::test]
async fn ending_session_publishes_invite_revoked_event() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(&state.pool, &creator).await?;
    let headers = auth_headers(&token);
    let broadcast = insert_ready_collaboration_broadcast(&state.pool, &creator).await?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO users (id, handle, display_name, avatar, tier, joined_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind("usr-pending")
    .bind("pending_guest")
    .bind("Pending Guest")
    .bind("https://cdn.lifestream.local/avatar/pending-guest.jpg")
    .bind("free")
    .bind(&now)
    .execute(&state.pool)
    .await?;

    let session = create_collaboration_session(
        State(state.clone()),
        headers.clone(),
        Json(CreateCollaborationSessionRequest {
            broadcast_id: Some(broadcast.id.clone()),
            title: Some("invite revoke regression".to_string()),
            chat_mode: Some("shared".to_string()),
            recording_policy: Some("host_archive".to_string()),
        }),
    )
    .await?
    .0;

    let invite = create_collaboration_invite(
        State(state.clone()),
        headers.clone(),
        Path(session.id.clone()),
        Json(CreateCollaborationInviteRequest {
            invitee_user_id: "usr-pending".to_string(),
            role: "guest".to_string(),
            mirror_to_guest_channel: false,
            message: Some("hold pending".to_string()),
            expires_in_minutes: Some(30),
        }),
    )
    .await?
    .0;

    let (mut subscription, _) = state
        .realtime
        .join(&collaboration_channel_id(&session.id))
        .await;

    let ended = end_collaboration_session(State(state.clone()), headers, Path(session.id.clone()))
        .await?
        .0;
    assert_eq!(ended.status, "ended");

    let mut saw_invite_revoked = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    while tokio::time::Instant::now() < deadline && !saw_invite_revoked {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let event = tokio::time::timeout(remaining, subscription.recv())
            .await
            .map_err(|_| {
                AppError::Internal(
                    "timed out waiting for collaboration invite revoke event".to_string(),
                )
            })?
            .map_err(|error| {
                AppError::Internal(format!(
                    "failed receiving collaboration realtime event: {error}"
                ))
            })?;
        if let WsEvent::CollaborationEvent { event } = event {
            if event.event_type == "invite_revoked"
                && event.payload["inviteId"] == Value::String(invite.id.clone())
                && event.payload["reason"] == Value::String("session_ended".to_string())
            {
                saw_invite_revoked = true;
            }
        }
    }

    assert!(saw_invite_revoked);
    let events = fetch_collaboration_events(&state.pool, &session.id, 0, 100).await?;
    assert!(events.iter().any(|event| {
        event.event_type == "invite_revoked"
            && event.payload["inviteId"] == Value::String(invite.id.clone())
            && event.payload["reason"] == Value::String("session_ended".to_string())
    }));

    state
        .realtime
        .leave(&collaboration_channel_id(&session.id))
        .await;
    Ok(())
}

#[tokio::test]
async fn host_can_revoke_pending_collaboration_invite_and_emit_event() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(&state.pool, &creator).await?;
    let headers = auth_headers(&token);
    let broadcast = insert_ready_collaboration_broadcast(&state.pool, &creator).await?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO users (id, handle, display_name, avatar, tier, joined_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind("usr-revoke-pending")
    .bind("revoke_pending")
    .bind("Revoke Pending")
    .bind("https://cdn.lifestream.local/avatar/revoke-pending.jpg")
    .bind("free")
    .bind(&now)
    .execute(&state.pool)
    .await?;

    let session = create_collaboration_session(
        State(state.clone()),
        headers.clone(),
        Json(CreateCollaborationSessionRequest {
            broadcast_id: Some(broadcast.id.clone()),
            title: Some("invite revoke direct".to_string()),
            chat_mode: Some("shared".to_string()),
            recording_policy: Some("host_archive".to_string()),
        }),
    )
    .await?
    .0;

    let invite = create_collaboration_invite(
        State(state.clone()),
        headers.clone(),
        Path(session.id.clone()),
        Json(CreateCollaborationInviteRequest {
            invitee_user_id: "usr-revoke-pending".to_string(),
            role: "guest".to_string(),
            mirror_to_guest_channel: false,
            message: Some("please join".to_string()),
            expires_in_minutes: Some(30),
        }),
    )
    .await?
    .0;

    let (mut subscription, _) = state
        .realtime
        .join(&collaboration_channel_id(&session.id))
        .await;

    let revoked = revoke_collaboration_invite(
        State(state.clone()),
        headers,
        Path((session.id.clone(), invite.id.clone())),
    )
    .await?
    .0;
    assert_eq!(revoked.state, "revoked");
    assert!(revoked.responded_at.is_some());

    let event = tokio::time::timeout(Duration::from_secs(1), subscription.recv())
        .await
        .map_err(|_| {
            AppError::Internal(
                "timed out waiting for collaboration invite revoke event".to_string(),
            )
        })?
        .map_err(|error| {
            AppError::Internal(format!(
                "failed receiving collaboration realtime event: {error}"
            ))
        })?;
    match event {
        WsEvent::CollaborationEvent { event } => {
            assert_eq!(event.event_type, "invite_revoked");
            assert_eq!(event.payload["inviteId"], Value::String(invite.id.clone()));
            assert_eq!(
                event.payload["reason"],
                Value::String("host_revoked".to_string())
            );
        }
        other => panic!("unexpected realtime event for invite revoke: {other:?}"),
    }

    let events = fetch_collaboration_events(&state.pool, &session.id, 0, 100).await?;
    assert!(events.iter().any(|event| {
        event.event_type == "invite_revoked"
            && event.payload["inviteId"] == Value::String(invite.id.clone())
            && event.payload["reason"] == Value::String("host_revoked".to_string())
    }));
    state
        .realtime
        .leave(&collaboration_channel_id(&session.id))
        .await;
    Ok(())
}

#[tokio::test]
async fn collaboration_socket_host_command_can_revoke_invite() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_collaboration_broadcast(&state.pool, &creator).await?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO users (id, handle, display_name, avatar, tier, joined_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind("usr-socket-revoke")
    .bind("socket_revoke")
    .bind("Socket Revoke")
    .bind("https://cdn.lifestream.local/avatar/socket-revoke.jpg")
    .bind("free")
    .bind(&now)
    .execute(&state.pool)
    .await?;

    let session = create_collaboration_session(
        State(state.clone()),
        auth_headers(&insert_creator_auth_session(&state.pool, &creator).await?),
        Json(CreateCollaborationSessionRequest {
            broadcast_id: Some(broadcast.id.clone()),
            title: Some("invite revoke socket".to_string()),
            chat_mode: Some("shared".to_string()),
            recording_policy: Some("host_archive".to_string()),
        }),
    )
    .await?
    .0;
    let invite = create_collaboration_invite(
        State(state.clone()),
        auth_headers(&insert_creator_auth_session(&state.pool, &creator).await?),
        Path(session.id.clone()),
        Json(CreateCollaborationInviteRequest {
            invitee_user_id: "usr-socket-revoke".to_string(),
            role: "guest".to_string(),
            mirror_to_guest_channel: false,
            message: Some("socket revoke".to_string()),
            expires_in_minutes: Some(30),
        }),
    )
    .await?
    .0;
    let identity = RequestIdentity {
        session_id: "host-collab-socket-revoke-invite".to_string(),
        user_id: creator.user_id.clone(),
        creator_id: Some(creator.id.clone()),
        scopes: vec!["user".to_string(), "creator".to_string()],
    };
    let session_view =
        fetch_current_collaboration_socket_session_view(&state, &session.id, &identity).await?;

    let outcome = execute_collaboration_socket_command(
        &state,
        &session.id,
        &identity,
        &session_view,
        CollaborationSocketCommand::RevokeInvite {
            invite_id: invite.id.clone(),
        },
    )
    .await?;

    assert_eq!(outcome.command_type, "revokeInvite");
    assert_eq!(outcome.state.as_deref(), Some("revoked"));
    let refreshed = fetch_collaboration_invite_by_id(&state.pool, &invite.id).await?;
    assert_eq!(refreshed.state, "revoked");
    assert!(refreshed.responded_at.is_some());
    Ok(())
}

#[tokio::test]
async fn collaboration_invite_inbox_only_returns_pending_invites() -> AppResult<()> {
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
    .bind("usr-pending-inbox")
    .bind("pending_inbox")
    .bind("Pending Inbox")
    .bind("https://cdn.lifestream.local/avatar/pending-inbox.jpg")
    .bind("free")
    .bind(&now)
    .execute(&state.pool)
    .await?;
    let pending_token = "test-pending-inbox-token".to_string();
    sqlx::query(
        r#"
        INSERT INTO auth_sessions (
            id, user_id, label, token_hash, scopes_json, created_at, expires_at, revoked_at, last_used_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(format!("sess-pending-inbox-{}", Uuid::new_v4().simple()))
    .bind("usr-pending-inbox")
    .bind("pending-inbox-session")
    .bind(hash_token(&pending_token))
    .bind(json!(["user"]).to_string())
    .bind(&now)
    .bind((Utc::now() + chrono::Duration::hours(2)).to_rfc3339())
    .bind(Option::<String>::None)
    .bind(&now)
    .execute(&state.pool)
    .await?;
    let pending_headers = auth_headers(&pending_token);

    let session = create_collaboration_session(
        State(state.clone()),
        host_headers.clone(),
        Json(CreateCollaborationSessionRequest {
            broadcast_id: Some(broadcast.id.clone()),
            title: Some("pending invite inbox".to_string()),
            chat_mode: Some("shared".to_string()),
            recording_policy: Some("host_archive".to_string()),
        }),
    )
    .await?
    .0;

    let invite = create_collaboration_invite(
        State(state.clone()),
        host_headers.clone(),
        Path(session.id.clone()),
        Json(CreateCollaborationInviteRequest {
            invitee_user_id: "usr-2".to_string(),
            role: "co_streamer".to_string(),
            mirror_to_guest_channel: true,
            message: Some("pending invite".to_string()),
            expires_in_minutes: Some(30),
        }),
    )
    .await?
    .0;

    let inbox_before = list_my_collaboration_invites(State(state.clone()), collab_headers.clone())
        .await?
        .0;
    assert!(inbox_before.iter().any(|item| item.id == invite.id));

    let accepted = accept_collaboration_invite(
        State(state.clone()),
        collab_headers.clone(),
        Path(invite.id.clone()),
    )
    .await?
    .0;
    assert_eq!(accepted.state, "backstage");

    let inbox_after_accept =
        list_my_collaboration_invites(State(state.clone()), collab_headers.clone())
            .await?
            .0;
    assert!(inbox_after_accept.iter().all(|item| item.id != invite.id));

    let pending_reinvite = create_collaboration_invite(
        State(state.clone()),
        host_headers.clone(),
        Path(session.id.clone()),
        Json(CreateCollaborationInviteRequest {
            invitee_user_id: "usr-pending-inbox".to_string(),
            role: "guest".to_string(),
            mirror_to_guest_channel: false,
            message: Some("revoked invite".to_string()),
            expires_in_minutes: Some(30),
        }),
    )
    .await?
    .0;

    let inbox_before_end =
        list_my_collaboration_invites(State(state.clone()), pending_headers.clone())
            .await?
            .0;
    assert!(
        inbox_before_end
            .iter()
            .any(|item| item.id == pending_reinvite.id)
    );

    let ended = end_collaboration_session(State(state.clone()), host_headers, Path(session.id))
        .await?
        .0;
    assert_eq!(ended.status, "ended");

    let inbox_after_end = list_my_collaboration_invites(State(state), pending_headers)
        .await?
        .0;
    assert!(
        inbox_after_end
            .iter()
            .all(|item| item.id != pending_reinvite.id)
    );

    Ok(())
}

#[tokio::test]
async fn participant_collaboration_runtime_only_exposes_own_grants() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let host_token = insert_creator_auth_session(&state.pool, &creator).await?;
    let host_headers = auth_headers(&host_token);
    let collab_creator = fetch_creator_profile(&state.pool, "crt-atlas").await?;
    let collab_token = insert_creator_auth_session(&state.pool, &collab_creator).await?;
    let collab_headers = auth_headers(&collab_token);
    let now = Utc::now().to_rfc3339();
    insert_test_user_with_creator_profile(
        &state.pool,
        "usr-collab-other",
        "collab_other",
        "Collab Other",
        "crt-guest-other",
        "guest_other",
        "Guest Other",
    )
    .await?;

    let other_guest_token = "test-guest-other-token".to_string();
    sqlx::query(
        r#"
        INSERT INTO auth_sessions (
            id, user_id, label, token_hash, scopes_json, created_at, expires_at, revoked_at, last_used_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(format!("sess-guest-other-{}", Uuid::new_v4().simple()))
    .bind("usr-collab-other")
    .bind("guest-other-session")
    .bind(hash_token(&other_guest_token))
    .bind(json!(["user", "creator", "creator:write"]).to_string())
    .bind(&now)
    .bind((Utc::now() + chrono::Duration::hours(2)).to_rfc3339())
    .bind(Option::<String>::None)
    .bind(&now)
    .execute(&state.pool)
    .await?;
    let other_guest_headers = auth_headers(&other_guest_token);

    let (session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;
    let other_participant = insert_collaboration_participant(
        &state.pool,
        &session.id,
        "usr-collab-other",
        Some("crt-guest-other"),
        "co_streamer",
        "live",
        true,
        true,
        true,
    )
    .await?;
    let expires_at = (Utc::now() + chrono::Duration::minutes(30)).to_rfc3339();
    let primary_grant =
        insert_mirror_grant(&state.pool, &session, &participant, &expires_at).await?;
    let other_grant =
        insert_mirror_grant(&state.pool, &session, &other_participant, &expires_at).await?;

    let participant_runtime = get_my_collaboration_runtime(
        State(state.clone()),
        collab_headers,
        Path(session.id.clone()),
    )
    .await?
    .0;
    assert_eq!(participant_runtime.grants.len(), 1);
    assert_eq!(participant_runtime.grants[0].id, primary_grant.id);
    assert!(
        participant_runtime
            .topology
            .members
            .iter()
            .any(|member| member.participant_id == other_participant.id)
    );

    let other_runtime = get_my_collaboration_runtime(
        State(state.clone()),
        other_guest_headers,
        Path(session.id.clone()),
    )
    .await?
    .0;
    assert_eq!(other_runtime.grants.len(), 1);
    assert_eq!(other_runtime.grants[0].id, other_grant.id);

    let host_runtime =
        get_creator_collaboration_runtime(State(state), host_headers, Path(session.id))
            .await?
            .0;
    assert_eq!(host_runtime.grants.len(), 2);
    assert!(
        host_runtime
            .grants
            .iter()
            .any(|grant| grant.id == primary_grant.id)
    );
    assert!(
        host_runtime
            .grants
            .iter()
            .any(|grant| grant.id == other_grant.id)
    );

    Ok(())
}

#[tokio::test]
async fn participant_collaboration_events_hide_other_invites_and_grants() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let host_token = insert_creator_auth_session(&state.pool, &creator).await?;
    let host_headers = auth_headers(&host_token);
    let collab_creator = fetch_creator_profile(&state.pool, "crt-atlas").await?;
    let collab_token = insert_creator_auth_session(&state.pool, &collab_creator).await?;
    let collab_headers = auth_headers(&collab_token);
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO users (id, handle, display_name, avatar, tier, joined_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind("usr-hidden-invite")
    .bind("hidden_invite")
    .bind("Hidden Invite")
    .bind("https://cdn.lifestream.local/avatar/hidden-invite.jpg")
    .bind("free")
    .bind(&now)
    .execute(&state.pool)
    .await?;
    insert_test_user_with_creator_profile(
        &state.pool,
        "usr-hidden-guest",
        "hidden_guest",
        "Hidden Guest",
        "crt-hidden-guest",
        "hidden_guest_creator",
        "Hidden Guest Creator",
    )
    .await?;

    let (session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;
    let self_grant =
        issue_mirror_grant_for_participant(&state, &session, &participant, "usr-1").await?;

    let hidden_invite = create_collaboration_invite(
        State(state.clone()),
        host_headers.clone(),
        Path(session.id.clone()),
        Json(CreateCollaborationInviteRequest {
            invitee_user_id: "usr-hidden-invite".to_string(),
            role: "guest".to_string(),
            mirror_to_guest_channel: false,
            message: Some("hidden from guest".to_string()),
            expires_in_minutes: Some(30),
        }),
    )
    .await?
    .0;

    let hidden_participant = insert_collaboration_participant(
        &state.pool,
        &session.id,
        "usr-hidden-guest",
        Some("crt-hidden-guest"),
        "co_streamer",
        "live",
        true,
        true,
        true,
    )
    .await?;
    let hidden_grant =
        issue_mirror_grant_for_participant(&state, &session, &hidden_participant, "usr-1").await?;

    let participant_events = list_my_collaboration_events(
        State(state.clone()),
        collab_headers.clone(),
        Path(session.id.clone()),
        Query(CollaborationEventsQuery {
            after_seq: None,
            limit: Some(100),
        }),
    )
    .await?
    .0;
    assert!(participant_events.iter().any(|event| {
        event.event_type == "mirror_grant_issued"
            && event.payload["grantId"] == Value::String(self_grant.id.clone())
    }));
    assert!(participant_events.iter().all(|event| {
        !(event.event_type == "invite_created"
            && event.payload["inviteId"] == Value::String(hidden_invite.id.clone()))
    }));
    assert!(participant_events.iter().all(|event| {
        !(event.event_type == "mirror_grant_issued"
            && event.payload["grantId"] == Value::String(hidden_grant.id.clone()))
    }));

    let participant_runtime = get_my_collaboration_runtime(
        State(state.clone()),
        collab_headers,
        Path(session.id.clone()),
    )
    .await?
    .0;
    assert!(participant_runtime.recent_events.iter().any(|event| {
        event.event_type == "mirror_grant_issued"
            && event.payload["grantId"] == Value::String(self_grant.id.clone())
    }));
    assert!(participant_runtime.recent_events.iter().all(|event| {
        !(event.event_type == "invite_created"
            && event.payload["inviteId"] == Value::String(hidden_invite.id.clone()))
    }));
    assert!(participant_runtime.recent_events.iter().all(|event| {
        !(event.event_type == "mirror_grant_issued"
            && event.payload["grantId"] == Value::String(hidden_grant.id.clone()))
    }));

    let host_events = list_creator_collaboration_events(
        State(state),
        host_headers,
        Path(session.id),
        Query(CollaborationEventsQuery {
            after_seq: None,
            limit: Some(100),
        }),
    )
    .await?
    .0;
    assert!(host_events.iter().any(|event| {
        event.event_type == "invite_created"
            && event.payload["inviteId"] == Value::String(hidden_invite.id.clone())
    }));
    assert!(host_events.iter().any(|event| {
        event.event_type == "mirror_grant_issued"
            && event.payload["grantId"] == Value::String(hidden_grant.id.clone())
    }));

    Ok(())
}
