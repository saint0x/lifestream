use super::*;
use crate::api::control::fetch_live_runtime_targets_for_session;

#[tokio::test]
async fn collaboration_invite_read_self_heals_expired_pending_invite() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let host_token = insert_creator_auth_session(&state.pool, &creator).await?;
    let host_headers = auth_headers(&host_token);
    let collab_creator = fetch_creator_profile(&state.pool, "crt-atlas").await?;
    let collab_token = insert_creator_auth_session(&state.pool, &collab_creator).await?;
    let collab_headers = auth_headers(&collab_token);
    let broadcast = insert_ready_collaboration_broadcast(&state.pool, &creator).await?;

    let session = create_collaboration_session(
        State(state.clone()),
        host_headers.clone(),
        Json(CreateCollaborationSessionRequest {
            broadcast_id: Some(broadcast.id.clone()),
            title: Some("expired invite read".to_string()),
            chat_mode: Some("shared".to_string()),
            recording_policy: Some("host_archive".to_string()),
        }),
    )
    .await?
    .0;

    let invite = create_collaboration_invite(
        State(state.clone()),
        host_headers,
        Path(session.id.clone()),
        Json(CreateCollaborationInviteRequest {
            invitee_user_id: "usr-2".to_string(),
            role: "co_streamer".to_string(),
            mirror_to_guest_channel: true,
            message: Some("expire on read".to_string()),
            expires_in_minutes: Some(30),
        }),
    )
    .await?
    .0;

    sqlx::query(
        "UPDATE collaboration_invites SET expires_at = ?, state = 'pending', responded_at = NULL WHERE id = ?",
    )
    .bind((Utc::now() - chrono::Duration::minutes(1)).to_rfc3339())
    .bind(&invite.id)
    .execute(&state.pool)
    .await?;

    let inbox = list_my_collaboration_invites(State(state.clone()), collab_headers)
        .await?
        .0;
    let refreshed_invite = fetch_collaboration_invite_by_id(&state.pool, &invite.id).await?;
    let events = fetch_collaboration_events(&state.pool, &session.id, 0, 100).await?;

    assert!(inbox.iter().all(|item| item.id != invite.id));
    assert_eq!(refreshed_invite.state, "expired");
    assert!(events.iter().any(|event| {
        event.event_type == "invite_expired"
            && event.payload["inviteId"] == Value::String(invite.id.clone())
    }));
    Ok(())
}

#[tokio::test]
async fn collaboration_runtime_read_self_heals_expired_mirror_grant() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let host_token = insert_creator_auth_session(&state.pool, &creator).await?;
    let host_headers = auth_headers(&host_token);
    let collab_creator = fetch_creator_profile(&state.pool, "crt-atlas").await?;
    let collab_token = insert_creator_auth_session(&state.pool, &collab_creator).await?;
    let collab_headers = auth_headers(&collab_token);
    let (session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;

    let grant = issue_mirror_grant_for_participant(&state, &session, &participant, "usr-1").await?;
    sqlx::query(
        "UPDATE collaboration_mirror_grants SET expires_at = ?, state = 'issued', revoked_at = NULL WHERE id = ?",
    )
    .bind((Utc::now() - chrono::Duration::minutes(1)).to_rfc3339())
    .bind(&grant.id)
    .execute(&state.pool)
    .await?;

    let runtime = get_my_collaboration_runtime(
        State(state.clone()),
        collab_headers.clone(),
        Path(session.id.clone()),
    )
    .await?
    .0;
    let refreshed_grant = fetch_collaboration_mirror_grant_by_id(&state.pool, &grant.id).await?;
    let participant_events = list_my_collaboration_events(
        State(state.clone()),
        collab_headers,
        Path(session.id.clone()),
        Query(CollaborationEventsQuery {
            after_seq: None,
            limit: Some(100),
        }),
    )
    .await?
    .0;
    let host_runtime =
        get_creator_collaboration_runtime(State(state), host_headers, Path(session.id))
            .await?
            .0;

    assert_eq!(refreshed_grant.state, "expired");
    assert!(
        runtime
            .grants
            .iter()
            .any(|item| item.id == grant.id && item.state == "expired")
    );
    assert!(participant_events.iter().any(|event| {
        event.event_type == "mirror_grant_expired"
            && event.payload["grantId"] == Value::String(grant.id.clone())
    }));
    assert!(
        host_runtime
            .grants
            .iter()
            .any(|item| item.id == grant.id && item.state == "expired")
    );
    Ok(())
}

#[tokio::test]
async fn collaboration_control_read_self_heals_stale_socket_presence() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let host_token = insert_creator_auth_session(&state.pool, &creator).await?;
    let host_headers = auth_headers(&host_token);
    let (session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;
    let cutoff =
        (Utc::now() - chrono::Duration::seconds(WS_PRESENCE_TTL_SECONDS + 30)).to_rfc3339();

    insert_collaboration_socket_session(
        &state.pool,
        &session.id,
        "usr-2",
        Some("crt-atlas"),
        &participant.id,
        &cutoff,
        &cutoff,
        None,
    )
    .await?;

    let control = get_creator_collaboration_control(
        State(state.clone()),
        host_headers,
        Path(session.id.clone()),
    )
    .await?
    .0;

    assert_eq!(control.stale_socket_count, 0);
    assert!(control.socket_sessions.iter().any(|socket| {
        socket.participant_id.as_deref() == Some(participant.id.as_str())
            && socket.disconnected_at.is_some()
    }));
    Ok(())
}

#[tokio::test]
async fn host_can_inspect_and_reconcile_collaboration_socket_session_by_id() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let host_token = insert_creator_auth_session(&state.pool, &creator).await?;
    let host_headers = auth_headers(&host_token);
    let (session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;
    let cutoff =
        (Utc::now() - chrono::Duration::seconds(WS_PRESENCE_TTL_SECONDS + 30)).to_rfc3339();
    let socket_id = format!("css-test-{}", Uuid::new_v4().simple());
    sqlx::query(
        r#"
        INSERT INTO collaboration_socket_sessions (
            id, collaboration_session_id, user_id, creator_id, participant_id,
            session_token_hash, connected_at, last_seen_at, disconnected_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(&socket_id)
    .bind(&session.id)
    .bind("usr-2")
    .bind("crt-atlas")
    .bind(&participant.id)
    .bind(hash_token(&format!("socket-{socket_id}")))
    .bind(&cutoff)
    .bind(&cutoff)
    .execute(&state.pool)
    .await?;

    let inspected = get_creator_collaboration_socket_session(
        State(state.clone()),
        host_headers.clone(),
        Path((session.id.clone(), socket_id.clone())),
    )
    .await?
    .0;
    let disconnected_before: Option<String> =
        sqlx::query("SELECT disconnected_at FROM collaboration_socket_sessions WHERE id = ?")
            .bind(&socket_id)
            .fetch_one(&state.pool)
            .await?
            .get("disconnected_at");

    assert_eq!(inspected.id, socket_id);
    assert!(inspected.is_stale);
    assert!(inspected.disconnected_at.is_none());
    assert!(disconnected_before.is_none());

    let report = reconcile_creator_collaboration_socket_session(
        State(state.clone()),
        host_headers,
        Path((session.id.clone(), socket_id.clone())),
    )
    .await?
    .0;
    let disconnected_after: Option<String> =
        sqlx::query("SELECT disconnected_at FROM collaboration_socket_sessions WHERE id = ?")
            .bind(&socket_id)
            .fetch_one(&state.pool)
            .await?
            .get("disconnected_at");

    assert_eq!(report.session_id, session.id);
    assert_eq!(report.socket_session_id, socket_id);
    assert_eq!(report.socket_session.id, report.socket_session_id);
    assert!(report.socket_session.disconnected_at.is_some());
    assert_eq!(report.actions.len(), 1);
    assert_eq!(report.actions[0].action_type, "socket_disconnected");
    assert_eq!(
        report.actions[0].previous_state.as_deref(),
        Some("connected")
    );
    assert_eq!(
        report.actions[0].next_state.as_deref(),
        Some("disconnected")
    );
    assert!(disconnected_after.is_some());
    Ok(())
}

#[tokio::test]
async fn collaboration_session_read_self_heals_dead_source_broadcast() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let host_token = insert_creator_auth_session(&state.pool, &creator).await?;
    let host_headers = auth_headers(&host_token);
    let collab_creator = fetch_creator_profile(&state.pool, "crt-atlas").await?;
    let collab_token = insert_creator_auth_session(&state.pool, &collab_creator).await?;
    let collab_headers = auth_headers(&collab_token);
    let broadcast = insert_ready_collaboration_broadcast(&state.pool, &creator).await?;

    let session = create_collaboration_session(
        State(state.clone()),
        host_headers.clone(),
        Json(CreateCollaborationSessionRequest {
            broadcast_id: Some(broadcast.id.clone()),
            title: Some("read-heal dead source".to_string()),
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
            message: Some("join dead source".to_string()),
            expires_in_minutes: Some(30),
        }),
    )
    .await?
    .0;
    let _participant = accept_collaboration_invite(
        State(state.clone()),
        collab_headers,
        Path(invite.id.clone()),
    )
    .await?
    .0;

    sqlx::query(
        "UPDATE broadcasts SET status = 'ended', ended_at = ?, duration_sec = 1 WHERE id = ?",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(&broadcast.id)
    .execute(&state.pool)
    .await?;

    let session_view = get_creator_collaboration_session(
        State(state.clone()),
        host_headers,
        Path(session.id.clone()),
    )
    .await?
    .0;
    let participant_error = get_my_collaboration_session(
        State(state.clone()),
        auth_headers(&collab_token),
        Path(session.id.clone()),
    )
    .await
    .expect_err("ended collaboration should no longer be visible to the departed participant");
    let events = fetch_collaboration_events(&state.pool, &session.id, 0, 100).await?;

    assert_eq!(session_view.status, "ended");
    assert!(matches!(participant_error, AppError::Forbidden));
    assert!(events.iter().any(|event| {
        event.event_type == "session_ended"
            && event.payload["details"]["reason"]
                == Value::String("source broadcast is no longer active".to_string())
    }));
    Ok(())
}

#[tokio::test]
async fn collaboration_presence_counts_distinct_participants_not_socket_tabs() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let (session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;
    let host_participant = session
        .participants
        .iter()
        .find(|item| item.role == "host")
        .cloned()
        .expect("host participant exists");
    let now = Utc::now().to_rfc3339();

    insert_collaboration_socket_session(
        &state.pool,
        &session.id,
        "usr-2",
        Some("crt-atlas"),
        &participant.id,
        &now,
        &now,
        None,
    )
    .await?;
    insert_collaboration_socket_session(
        &state.pool,
        &session.id,
        "usr-2",
        Some("crt-atlas"),
        &participant.id,
        &now,
        &now,
        None,
    )
    .await?;
    insert_collaboration_socket_session(
        &state.pool,
        &session.id,
        "usr-1",
        Some("crt-deepsaint"),
        &host_participant.id,
        &now,
        &now,
        None,
    )
    .await?;

    let per_session = count_active_collaboration_socket_sessions(&state.pool, &session.id).await?;
    let all_active = count_all_active_collaboration_socket_sessions(&state.pool).await?;
    let runtime = build_collaboration_runtime_response_for_host(
        &state.pool,
        fetch_collaboration_session_by_id(&state.pool, &session.id).await?,
    )
    .await?;

    assert_eq!(per_session, 2);
    assert!(all_active >= 2);
    assert_eq!(runtime.topology.connected_participants, 2);
    Ok(())
}

#[tokio::test]
async fn collaboration_runtime_topology_exposes_contributions_and_split_archive_outputs()
-> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let (session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;
    sqlx::query(
        "UPDATE collaboration_sessions SET recording_policy = 'split_archive' WHERE id = ?",
    )
    .bind(&session.id)
    .execute(&state.pool)
    .await?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO live_ingest_sessions (
            id, creator_id, broadcast_id, stream_key_hash, ingest_token_hash, protocol,
            contribution_class, contribution_state, ingest_server, status, bitrate_kbps, viewers,
            dropped_frames, connected_at, last_heartbeat_at, disconnected_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'connected', ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(format!("ing-test-{}", Uuid::new_v4().simple()))
    .bind(&creator.id)
    .bind(&session.source_broadcast_id)
    .bind(hash_token(&creator.stream_key))
    .bind(hash_token(&format!(
        "fixture-ingest-token-{}",
        Uuid::new_v4().simple()
    )))
    .bind("rtmp")
    .bind("rtmp_push")
    .bind("healthy")
    .bind("topology-test-ingest")
    .bind(6100_i64)
    .bind(144_i64)
    .bind(0_i64)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;
    let _grant =
        issue_mirror_grant_for_participant(&state, &session, &participant, "usr-1").await?;

    let runtime = build_collaboration_runtime_response_for_host(
        &state.pool,
        fetch_collaboration_session_by_id(&state.pool, &session.id).await?,
    )
    .await?;
    let host_contribution = runtime
        .topology
        .contributions
        .iter()
        .find(|item| item.participant_id == runtime.session.participant.id)
        .expect("host contribution should exist");
    let guest_contribution = runtime
        .topology
        .contributions
        .iter()
        .find(|item| item.participant_id == participant.id)
        .expect("guest contribution should exist");

    assert_eq!(host_contribution.transport_class, "rtmp_push");
    assert_eq!(host_contribution.contribution_state, "healthy");
    assert!(host_contribution.ingest_session_id.is_some());
    assert_eq!(guest_contribution.transport_class, "collaboration_socket");
    assert_eq!(guest_contribution.contribution_state, "awaiting_socket");
    assert!(runtime.topology.mix_minus_required);
    assert!(runtime.topology.programs.iter().any(|program| {
        program.program_kind == "host_program"
            && program
                .source_participant_ids
                .contains(&runtime.session.participant.id)
            && program
                .output_ids
                .iter()
                .any(|output_id| output_id == &format!("col-out-host-{}", session.id))
    }));
    assert!(runtime.topology.programs.iter().any(|program| {
        program.program_kind == "guest_program"
            && program
                .output_ids
                .iter()
                .any(|output_id| output_id == &format!("col-out-mirror-{}", participant.id))
    }));
    assert!(runtime.topology.audio.iter().any(|route| {
        route.participant_id == participant.id
            && route.route_kind == "mix_minus_return"
            && route.receive_program_audio
            && route.excluded_participant_ids == vec![participant.id.clone()]
    }));
    assert!(
        runtime
            .topology
            .outputs
            .iter()
            .any(|output| output.output_kind == "host_channel"
                && output.target_creator_id.as_deref() == Some(creator.id.as_str()))
    );
    assert!(
        runtime
            .topology
            .outputs
            .iter()
            .any(|output| output.output_kind == "mirror_channel"
                && output.target_creator_id.as_deref() == Some("crt-atlas"))
    );
    let mirror_output = runtime
        .topology
        .outputs
        .iter()
        .find(|output| {
            output.output_kind == "mirror_channel"
                && output.target_creator_id.as_deref() == Some("crt-atlas")
        })
        .expect("mirror output should exist");
    assert!(
        mirror_output
            .source_participant_ids
            .contains(&runtime.session.participant.id)
    );
    assert!(
        mirror_output
            .source_participant_ids
            .contains(&participant.id)
    );
    assert!(
        runtime
            .topology
            .outputs
            .iter()
            .filter(|output| output.output_kind == "archive" && output.recording_enabled)
            .count()
            >= 2
    );
    assert!(
        guest_contribution
            .attached_output_ids
            .iter()
            .any(|output_id| output_id == &format!("col-out-archive-{}", participant.id))
    );

    Ok(())
}

#[tokio::test]
async fn collaboration_runtime_topology_skips_guest_outputs_without_authorized_mirror_route()
-> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let (session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO live_ingest_sessions (
            id, creator_id, broadcast_id, stream_key_hash, ingest_token_hash, protocol,
            contribution_class, contribution_state, ingest_server, status, bitrate_kbps, viewers,
            dropped_frames, connected_at, last_heartbeat_at, disconnected_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'connected', ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(format!("ing-test-{}", Uuid::new_v4().simple()))
    .bind(&creator.id)
    .bind(&session.source_broadcast_id)
    .bind(hash_token(&creator.stream_key))
    .bind(hash_token(&format!(
        "fixture-ingest-token-{}",
        Uuid::new_v4().simple()
    )))
    .bind("rtmp")
    .bind("rtmp_push")
    .bind("healthy")
    .bind("topology-test-unauthorized")
    .bind(6100_i64)
    .bind(144_i64)
    .bind(0_i64)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;

    let runtime = build_collaboration_runtime_response_for_host(
        &state.pool,
        fetch_collaboration_session_by_id(&state.pool, &session.id).await?,
    )
    .await?;
    let guest_contribution = runtime
        .topology
        .contributions
        .iter()
        .find(|item| item.participant_id == participant.id)
        .expect("guest contribution should exist");
    let guest_member = runtime
        .topology
        .members
        .iter()
        .find(|item| item.participant_id == participant.id)
        .expect("guest member should exist");

    assert_eq!(guest_member.mirror_pickup_state, "eligible");
    assert!(
        runtime
            .topology
            .outputs
            .iter()
            .all(|output| output.id != format!("col-out-mirror-{}", participant.id))
    );
    assert!(
        runtime
            .topology
            .outputs
            .iter()
            .all(|output| output.id != format!("col-out-archive-{}", participant.id))
    );
    assert!(
        guest_contribution
            .attached_output_ids
            .iter()
            .all(|output_id| output_id != &format!("col-out-mirror-{}", participant.id))
    );
    assert!(
        guest_contribution
            .attached_output_ids
            .iter()
            .all(|output_id| output_id != &format!("col-out-archive-{}", participant.id))
    );
    assert!(runtime.topology.audio.iter().any(|route| {
        route.participant_id == participant.id
            && route.route_kind == "inactive"
            && !route.receive_program_audio
    }));

    Ok(())
}

#[tokio::test]
async fn collaboration_event_syncs_runtime_targets_for_active_ingest() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let (session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;

    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-collab-sync".to_string(),
            broadcast_id: Some(session.source_broadcast_id.clone()),
        }),
    )
    .await?
    .0;

    let before = fetch_live_runtime_targets_for_session(&state.pool, &connected.session.id).await?;
    assert!(
        before
            .iter()
            .any(|target| target.target_kind == "host_channel")
    );
    assert!(before.iter().all(|target| {
        !(target.target_kind == "mirror_channel"
            && target.target_creator_id.as_deref() == Some("crt-atlas"))
    }));

    issue_mirror_grant_for_participant(&state, &session, &participant, &creator.user_id).await?;

    let after = fetch_live_runtime_targets_for_session(&state.pool, &connected.session.id).await?;
    let mirror_target = after
        .iter()
        .find(|target| {
            target.target_kind == "mirror_channel"
                && target.target_creator_id.as_deref() == Some("crt-atlas")
        })
        .expect("mirror target should be persisted after topology publication");

    assert_eq!(mirror_target.route_state, "issued");
    assert!(mirror_target.playback_enabled);
    assert!(mirror_target.relative_path.is_some());

    let runtime = fetch_creator_live_runtime_response(&state.pool, &creator.id).await?;
    assert!(runtime.active_runtime_targets.iter().any(|target| {
        target.target_kind == "mirror_channel"
            && target.target_creator_id.as_deref() == Some("crt-atlas")
            && target.route_state == "issued"
    }));

    Ok(())
}

#[tokio::test]
async fn creator_live_read_self_heals_stale_socket_presence() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let cutoff =
        (Utc::now() - chrono::Duration::seconds(WS_PRESENCE_TTL_SECONDS + 30)).to_rfc3339();

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
    .bind(hash_token("creator-live-stale"))
    .bind(&cutoff)
    .bind(&cutoff)
    .execute(&state.pool)
    .await?;

    let _control = fetch_creator_live_control_response(&state.pool, &creator.id).await?;

    let disconnected_at: Option<String> = sqlx::query(
        "SELECT disconnected_at FROM creator_live_socket_sessions WHERE creator_id = ? AND user_id = ? AND session_token_hash = ?",
    )
    .bind(&creator.id)
    .bind("usr-1")
    .bind(hash_token("creator-live-stale"))
    .fetch_one(&state.pool)
    .await?
    .get("disconnected_at");
    let active_count = count_all_active_creator_live_socket_sessions(&state.pool).await?;

    assert!(disconnected_at.is_some());
    assert_eq!(active_count, 0);
    Ok(())
}

#[tokio::test]
async fn creator_can_inspect_and_reconcile_live_socket_session_by_id() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(&state.pool, &creator).await?;
    let headers = auth_headers(&token);
    let cutoff =
        (Utc::now() - chrono::Duration::seconds(WS_PRESENCE_TTL_SECONDS + 30)).to_rfc3339();
    let socket_id = format!("cls-test-{}", Uuid::new_v4().simple());

    sqlx::query(
        r#"
        INSERT INTO creator_live_socket_sessions (
            id, creator_id, user_id, session_token_hash, connected_at, last_seen_at, disconnected_at
        ) VALUES (?, ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(&socket_id)
    .bind(&creator.id)
    .bind("usr-1")
    .bind(hash_token(&format!("creator-live-socket-{socket_id}")))
    .bind(&cutoff)
    .bind(&cutoff)
    .execute(&state.pool)
    .await?;

    let inspected = get_creator_live_socket_session(
        State(state.clone()),
        headers.clone(),
        Path(socket_id.clone()),
    )
    .await?
    .0;
    let disconnected_before: Option<String> = sqlx::query(
        "SELECT disconnected_at FROM creator_live_socket_sessions WHERE creator_id = ? AND id = ?",
    )
    .bind(&creator.id)
    .bind(&socket_id)
    .fetch_one(&state.pool)
    .await?
    .get("disconnected_at");

    assert_eq!(inspected.id, socket_id);
    assert!(inspected.is_stale);
    assert!(inspected.disconnected_at.is_none());
    assert!(disconnected_before.is_none());

    let report = reconcile_creator_live_socket_session(
        State(state.clone()),
        headers,
        Path(socket_id.clone()),
    )
    .await?
    .0;
    let disconnected_after: Option<String> = sqlx::query(
        "SELECT disconnected_at FROM creator_live_socket_sessions WHERE creator_id = ? AND id = ?",
    )
    .bind(&creator.id)
    .bind(&socket_id)
    .fetch_one(&state.pool)
    .await?
    .get("disconnected_at");

    assert_eq!(report.creator_id, creator.id);
    assert_eq!(report.socket_session_id, socket_id);
    assert_eq!(report.socket_session.id, report.socket_session_id);
    assert!(report.socket_session.disconnected_at.is_some());
    assert_eq!(report.actions.len(), 1);
    assert_eq!(report.actions[0].action_type, "socket_disconnected");
    assert_eq!(
        report.actions[0].previous_state.as_deref(),
        Some("connected")
    );
    assert_eq!(
        report.actions[0].next_state.as_deref(),
        Some("disconnected")
    );
    assert!(disconnected_after.is_some());
    Ok(())
}
