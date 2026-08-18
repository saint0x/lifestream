use super::*;

#[tokio::test]
async fn expired_mirror_grant_cannot_be_redeemed_before_reconciliation() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let (_session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;
    let expired_at = (Utc::now() - chrono::Duration::minutes(1)).to_rfc3339();
    let grant = insert_mirror_grant(
        &state.pool,
        &fetch_collaboration_session_by_id(&state.pool, &participant.session_id).await?,
        &participant,
        &expired_at,
    )
    .await?;
    let identity = RequestIdentity {
        session_id: "test-session".to_string(),
        user_id: "usr-2".to_string(),
        creator_id: Some("crt-atlas".to_string()),
        scopes: vec!["user".to_string(), "creator".to_string()],
    };

    let error = redeem_collaboration_mirror_grant_internal(&state, &identity, &grant.id)
        .await
        .expect_err("expired grant should not redeem");
    match error {
        AppError::BadRequest(message) => {
            assert!(message.contains("expired"));
        }
        other => panic!("unexpected error: {other:?}"),
    }

    let refreshed_grant = fetch_collaboration_mirror_grant_by_id(&state.pool, &grant.id).await?;
    assert_eq!(refreshed_grant.state, "issued");
    assert!(refreshed_grant.activated_at.is_none());
    Ok(())
}

#[tokio::test]
async fn issue_mirror_grant_rejects_non_mirrored_or_non_live_participants() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let (session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;

    let mut not_mirrored = participant.clone();
    not_mirrored.mirror_to_guest_channel = false;
    let error = issue_mirror_grant_for_participant(&state, &session, &not_mirrored, "usr-1")
        .await
        .expect_err("non-mirrored participant should be rejected");
    match error {
        AppError::BadRequest(message) => {
            assert!(message.contains("mirrored guest channel pickup"));
        }
        other => panic!("unexpected error: {other:?}"),
    }

    let mut backstage = participant.clone();
    backstage.state = "backstage".to_string();
    let error = issue_mirror_grant_for_participant(&state, &session, &backstage, "usr-1")
        .await
        .expect_err("backstage participant should be rejected");
    match error {
        AppError::BadRequest(message) => {
            assert!(message.contains("live participants"));
        }
        other => panic!("unexpected error: {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn collaboration_socket_host_command_can_remove_participant() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let (session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;
    let identity = RequestIdentity {
        session_id: "host-collab-socket-remove".to_string(),
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
        CollaborationSocketCommand::RemoveParticipant {
            participant_id: participant.id.clone(),
        },
    )
    .await?;

    assert_eq!(outcome.command_type, "removeParticipant");
    assert_eq!(
        outcome.participant_id.as_deref(),
        Some(participant.id.as_str())
    );
    assert_eq!(outcome.state.as_deref(), Some("removed"));

    let refreshed = fetch_collaboration_participant_by_id(&state.pool, &participant.id).await?;
    assert_eq!(refreshed.state, "removed");
    assert!(refreshed.left_at.is_some());

    let events = fetch_collaboration_events(&state.pool, &session.id, 0, 100).await?;
    assert!(events.iter().any(|event| {
        event.event_type == "participant_removed"
            && event.payload["participantId"] == Value::String(participant.id.clone())
    }));
    Ok(())
}

#[tokio::test]
async fn collaboration_socket_host_commands_can_issue_and_revoke_mirror_grants() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let (session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;
    let identity = RequestIdentity {
        session_id: "host-collab-socket-grants".to_string(),
        user_id: creator.user_id.clone(),
        creator_id: Some(creator.id.clone()),
        scopes: vec!["user".to_string(), "creator".to_string()],
    };
    let session_view =
        fetch_current_collaboration_socket_session_view(&state, &session.id, &identity).await?;

    let issued = execute_collaboration_socket_command(
        &state,
        &session.id,
        &identity,
        &session_view,
        CollaborationSocketCommand::IssueMirrorGrant {
            participant_id: participant.id.clone(),
        },
    )
    .await?;
    assert_eq!(issued.command_type, "issueMirrorGrant");
    assert_eq!(
        issued.participant_id.as_deref(),
        Some(participant.id.as_str())
    );

    let issued_grants =
        fetch_collaboration_mirror_grants_for_participant(&state.pool, &participant.id).await?;
    assert_eq!(issued_grants.len(), 1);
    assert_eq!(issued_grants[0].state, "issued");

    let revoked = execute_collaboration_socket_command(
        &state,
        &session.id,
        &identity,
        &session_view,
        CollaborationSocketCommand::RevokeMirrorGrants {
            participant_id: participant.id.clone(),
        },
    )
    .await?;
    assert_eq!(revoked.command_type, "revokeMirrorGrants");
    assert_eq!(
        revoked.participant_id.as_deref(),
        Some(participant.id.as_str())
    );

    let refreshed_grants =
        fetch_collaboration_mirror_grants_for_participant(&state.pool, &participant.id).await?;
    assert_eq!(refreshed_grants.len(), 1);
    assert_eq!(refreshed_grants[0].state, "revoked");
    assert!(refreshed_grants[0].revoked_at.is_some());

    let events = fetch_collaboration_events(&state.pool, &session.id, 0, 100).await?;
    assert!(events.iter().any(|event| {
        event.event_type == "mirror_grant_issued"
            && event.participant_id.as_deref() == Some(participant.id.as_str())
            && event.payload["guestCreatorId"] == Value::String("crt-atlas".to_string())
    }));
    assert!(events.iter().any(|event| {
        event.event_type == "mirror_grant_revoked"
            && event.payload["participantId"] == Value::String(participant.id.clone())
            && event.payload["reason"] == Value::String("host_revoked".to_string())
    }));
    Ok(())
}

#[tokio::test]
async fn redeeming_mirror_grant_requires_creator_scope_for_guest_channel() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let (session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;
    let grant = issue_mirror_grant_for_participant(&state, &session, &participant, "usr-1").await?;
    let identity = RequestIdentity {
        session_id: "test-session".to_string(),
        user_id: "usr-2".to_string(),
        creator_id: None,
        scopes: vec!["user".to_string()],
    };

    let error = redeem_collaboration_mirror_grant_internal(&state, &identity, &grant.id)
        .await
        .expect_err("redeeming a guest-channel pickup without creator scope must fail");
    assert!(matches!(error, AppError::Forbidden));
    Ok(())
}

#[tokio::test]
async fn redeeming_mirror_grant_materializes_guest_pickup_broadcast() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let guest_creator = fetch_creator_profile(&state.pool, "crt-atlas").await?;
    reset_creator_live_state(&state.pool, &guest_creator).await?;
    let (session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;
    let grant = issue_mirror_grant_for_participant(&state, &session, &participant, "usr-1").await?;
    let (mut subscription, _) = state
        .realtime
        .join(&creator_live_channel_id("crt-atlas"))
        .await;
    let identity = RequestIdentity {
        session_id: "test-session".to_string(),
        user_id: "usr-2".to_string(),
        creator_id: Some("crt-atlas".to_string()),
        scopes: vec!["user".to_string(), "creator".to_string()],
    };

    let redeemed = redeem_collaboration_mirror_grant_internal(&state, &identity, &grant.id).await?;
    assert_eq!(redeemed.state, "active");
    assert!(redeemed.activated_at.is_some());

    let pickups =
        fetch_collaboration_mirror_pickups_for_participant(&state.pool, &participant.id).await?;
    assert_eq!(pickups.len(), 1);
    let pickup = &pickups[0];
    assert_eq!(pickup.state, "active");
    assert_eq!(pickup.grant_id, grant.id);
    assert_eq!(pickup.source_broadcast_id, session.source_broadcast_id);

    let guest_broadcast =
        fetch_broadcast_by_id(&state.pool, "crt-atlas", &pickup.guest_broadcast_id).await?;
    assert_eq!(guest_broadcast.status, "live");
    assert_eq!(guest_broadcast.title, "Collaboration Validation");

    let guest_profile = fetch_creator_profile(&state.pool, "crt-atlas").await?;
    assert_eq!(guest_profile.live_status, "live");
    assert_eq!(
        guest_profile.current_broadcast_id.as_deref(),
        Some(pickup.guest_broadcast_id.as_str())
    );

    let runtime = build_collaboration_runtime_response_for_host(
        &state.pool,
        fetch_collaboration_session_by_id(&state.pool, &session.id).await?,
    )
    .await?;
    let guest_member = runtime
        .topology
        .members
        .iter()
        .find(|member| member.participant_id == participant.id)
        .expect("guest member present");
    assert_eq!(guest_member.mirror_pickup_state, "active");
    assert_eq!(
        guest_member.mirror_pickup_broadcast_id.as_deref(),
        Some(pickup.guest_broadcast_id.as_str())
    );
    assert_eq!(runtime.pickups.len(), 1);

    let published = tokio::time::timeout(Duration::from_secs(1), subscription.recv())
        .await
        .map_err(|_| {
            AppError::Internal(
                "timed out waiting for guest creator live state publication".to_string(),
            )
        })?
        .map_err(|error| {
            AppError::Internal(format!(
                "failed receiving guest creator live state publication: {error}"
            ))
        })?;
    match published {
        WsEvent::CreatorLiveState { control, runtime } => {
            assert_eq!(control.snapshot.profile.id, "crt-atlas");
            assert_eq!(control.snapshot.profile.live_status, "live");
            assert_eq!(
                control.snapshot.profile.current_broadcast_id.as_deref(),
                Some(pickup.guest_broadcast_id.as_str())
            );
            assert_eq!(
                runtime.snapshot.profile.current_broadcast_id.as_deref(),
                Some(pickup.guest_broadcast_id.as_str())
            );
        }
        other => panic!("unexpected event: {other:?}"),
    }
    state
        .realtime
        .leave(&creator_live_channel_id("crt-atlas"))
        .await;
    Ok(())
}

#[tokio::test]
async fn redeeming_mirror_grant_keeps_grant_issued_when_guest_has_other_broadcast() -> AppResult<()>
{
    let (state, creator) = setup_test_state().await?;
    let guest_creator = fetch_creator_profile(&state.pool, "crt-atlas").await?;
    reset_creator_live_state(&state.pool, &guest_creator).await?;
    let (session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;
    let grant = issue_mirror_grant_for_participant(&state, &session, &participant, "usr-1").await?;
    let conflicting_broadcast = insert_ready_broadcast(&state.pool, &guest_creator).await?;
    let identity = RequestIdentity {
        session_id: "test-session".to_string(),
        user_id: "usr-2".to_string(),
        creator_id: Some("crt-atlas".to_string()),
        scopes: vec!["user".to_string(), "creator".to_string()],
    };

    let error = redeem_collaboration_mirror_grant_internal(&state, &identity, &grant.id)
        .await
        .expect_err("guest pickup should fail when the guest already has another broadcast");
    match error {
        AppError::BadRequest(message) => {
            assert!(message.contains("already has another active or pending broadcast"));
        }
        other => panic!("unexpected error: {other:?}"),
    }

    let refreshed_grant = fetch_collaboration_mirror_grant_by_id(&state.pool, &grant.id).await?;
    let pickups =
        fetch_collaboration_mirror_pickups_for_participant(&state.pool, &participant.id).await?;
    let refreshed_guest = fetch_creator_profile(&state.pool, "crt-atlas").await?;

    assert_eq!(refreshed_grant.state, "issued");
    assert!(refreshed_grant.activated_at.is_none());
    assert!(pickups.is_empty());
    assert_eq!(refreshed_guest.live_status, "ready");
    assert_eq!(
        refreshed_guest.current_broadcast_id.as_deref(),
        Some(conflicting_broadcast.id.as_str())
    );
    Ok(())
}

#[tokio::test]
async fn revoking_mirror_grant_ends_guest_pickup_broadcast() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let guest_creator = fetch_creator_profile(&state.pool, "crt-atlas").await?;
    reset_creator_live_state(&state.pool, &guest_creator).await?;
    let (session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;
    let grant = issue_mirror_grant_for_participant(&state, &session, &participant, "usr-1").await?;
    let identity = RequestIdentity {
        session_id: "test-session".to_string(),
        user_id: "usr-2".to_string(),
        creator_id: Some("crt-atlas".to_string()),
        scopes: vec!["user".to_string(), "creator".to_string()],
    };

    let _ = redeem_collaboration_mirror_grant_internal(&state, &identity, &grant.id).await?;
    let (mut subscription, _) = state
        .realtime
        .join(&creator_live_channel_id("crt-atlas"))
        .await;
    revoke_collaboration_mirror_grants_for_participant(
        &state,
        &session.id,
        &participant.id,
        Some("usr-1".to_string()),
        &Utc::now().to_rfc3339(),
        "test_revoke",
    )
    .await?;

    let pickup = fetch_collaboration_mirror_pickups_for_participant(&state.pool, &participant.id)
        .await?
        .into_iter()
        .next()
        .expect("pickup should exist");
    assert_eq!(pickup.state, "revoked");
    assert!(pickup.ended_at.is_some());

    let guest_broadcast =
        fetch_broadcast_by_id(&state.pool, "crt-atlas", &pickup.guest_broadcast_id).await?;
    assert_eq!(guest_broadcast.status, "ended");
    assert!(guest_broadcast.ended_at.is_some());

    let guest_profile = fetch_creator_profile(&state.pool, "crt-atlas").await?;
    assert_eq!(guest_profile.live_status, "offline");
    assert!(guest_profile.current_broadcast_id.is_none());

    let published = tokio::time::timeout(Duration::from_secs(1), subscription.recv())
        .await
        .map_err(|_| {
            AppError::Internal(
                "timed out waiting for guest creator live teardown publication".to_string(),
            )
        })?
        .map_err(|error| {
            AppError::Internal(format!(
                "failed receiving guest creator live teardown publication: {error}"
            ))
        })?;
    match published {
        WsEvent::CreatorLiveState { control, runtime } => {
            assert_eq!(control.snapshot.profile.id, "crt-atlas");
            assert_eq!(control.snapshot.profile.live_status, "offline");
            assert!(control.snapshot.profile.current_broadcast_id.is_none());
            assert_eq!(runtime.snapshot.profile.live_status, "offline");
            assert!(runtime.snapshot.profile.current_broadcast_id.is_none());
        }
        other => panic!("unexpected event: {other:?}"),
    }
    state
        .realtime
        .leave(&creator_live_channel_id("crt-atlas"))
        .await;
    Ok(())
}

#[tokio::test]
async fn redeeming_mirror_grant_propagates_host_viewers_to_guest_channel() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let guest_creator = fetch_creator_profile(&state.pool, "crt-atlas").await?;
    reset_creator_live_state(&state.pool, &guest_creator).await?;
    let (session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;
    let host_broadcast =
        fetch_broadcast_by_id(&state.pool, &creator.id, &session.source_broadcast_id).await?;
    let viewers = 777_i64;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE creator_profiles SET live_status = 'live', current_broadcast_id = ? WHERE id = ?",
    )
    .bind(&session.source_broadcast_id)
    .bind(&creator.id)
    .execute(&state.pool)
    .await?;
    sqlx::query("UPDATE streamers SET is_live = 1 WHERE handle = ?")
        .bind(&creator.handle)
        .execute(&state.pool)
        .await?;
    ensure_live_stream_row(&state.pool, &creator, &host_broadcast, viewers).await?;
    sqlx::query(
        r#"
        INSERT INTO live_ingest_sessions (
            id, creator_id, broadcast_id, stream_key_hash, ingest_token_hash, protocol,
            ingest_server, status, bitrate_kbps, viewers, dropped_frames, connected_at,
            last_heartbeat_at, disconnected_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, 'connected', ?, ?, ?, ?, ?, NULL)
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
    .bind("test-ingest-viewers")
    .bind(6400_i64)
    .bind(viewers)
    .bind(0_i64)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;

    let grant = issue_mirror_grant_for_participant(&state, &session, &participant, "usr-1").await?;
    let identity = RequestIdentity {
        session_id: "test-session".to_string(),
        user_id: "usr-2".to_string(),
        creator_id: Some("crt-atlas".to_string()),
        scopes: vec!["user".to_string(), "creator".to_string()],
    };

    let redeemed = redeem_collaboration_mirror_grant_internal(&state, &identity, &grant.id).await?;
    assert_eq!(redeemed.state, "active");

    let pickup = fetch_collaboration_mirror_pickups_for_participant(&state.pool, &participant.id)
        .await?
        .into_iter()
        .next()
        .expect("guest pickup should exist");
    let guest_stream =
        fetch_live_stream_by_id(&state.pool, &format!("lv-{}-live", guest_creator.handle)).await?;
    let guest_control = fetch_creator_live_control_response(&state.pool, &guest_creator.id).await?;

    assert_eq!(guest_stream.viewers, viewers);
    assert_eq!(guest_control.current_viewers, viewers);
    assert_eq!(
        guest_control
            .snapshot
            .profile
            .current_broadcast_id
            .as_deref(),
        Some(pickup.guest_broadcast_id.as_str())
    );
    Ok(())
}

#[tokio::test]
async fn mirrored_guest_channel_is_publicly_listed_and_can_issue_live_playback() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let guest_creator = fetch_creator_profile(&state.pool, "crt-atlas").await?;
    reset_creator_live_state(&state.pool, &guest_creator).await?;
    let (session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;
    let host_broadcast =
        fetch_broadcast_by_id(&state.pool, &creator.id, &session.source_broadcast_id).await?;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE creator_profiles SET live_status = 'live', current_broadcast_id = ? WHERE id = ?",
    )
    .bind(&session.source_broadcast_id)
    .bind(&creator.id)
    .execute(&state.pool)
    .await?;
    sqlx::query("UPDATE streamers SET is_live = 1 WHERE handle = ?")
        .bind(&creator.handle)
        .execute(&state.pool)
        .await?;
    ensure_live_stream_row(&state.pool, &creator, &host_broadcast, 321).await?;

    let playback_row = sqlx::query(
        r#"
        SELECT id, poster_relative_path, playback_relative_path
        FROM media_assets
        WHERE status IN ('ready', 'published')
          AND playback_relative_path IS NOT NULL
        ORDER BY created_at ASC
        LIMIT 1
        "#,
    )
    .fetch_one(&state.pool)
    .await?;
    sqlx::query(
        r#"
        UPDATE live_streams
        SET playback_asset_id = ?, poster_relative_path = ?, playback_relative_path = ?
        WHERE id = ?
        "#,
    )
    .bind(playback_row.get::<String, _>("id"))
    .bind(playback_row.get::<Option<String>, _>("poster_relative_path"))
    .bind(playback_row.get::<Option<String>, _>("playback_relative_path"))
    .bind(format!("lv-{}-live", creator.handle))
    .execute(&state.pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO live_ingest_sessions (
            id, creator_id, broadcast_id, stream_key_hash, ingest_token_hash, protocol,
            ingest_server, status, bitrate_kbps, viewers, dropped_frames, connected_at,
            last_heartbeat_at, disconnected_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, 'connected', ?, ?, ?, ?, ?, NULL)
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
    .bind("test-ingest-mirror-playback")
    .bind(5400_i64)
    .bind(321_i64)
    .bind(0_i64)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;

    let grant = issue_mirror_grant_for_participant(&state, &session, &participant, "usr-1").await?;
    let identity = RequestIdentity {
        session_id: "test-session".to_string(),
        user_id: "usr-2".to_string(),
        creator_id: Some("crt-atlas".to_string()),
        scopes: vec!["user".to_string(), "creator".to_string()],
    };
    let _ = redeem_collaboration_mirror_grant_internal(&state, &identity, &grant.id).await?;
    let guest_profile = fetch_creator_profile(&state.pool, &guest_creator.id).await?;
    let guest_broadcast_id = guest_profile
        .current_broadcast_id
        .clone()
        .expect("guest pickup should materialize a broadcast");
    let mirror_manifest_relative_path = format!(
        "live/{}/{}/col-out-mirror-{}/master.m3u8",
        guest_creator.id, guest_broadcast_id, participant.id
    );

    let guest_stream_id = format!("lv-{}-live", guest_creator.handle);
    let live_streams = list_live_streams(State(state.clone())).await?.0;
    assert!(
        live_streams
            .iter()
            .any(|stream| stream.id == guest_stream_id)
    );
    write_test_media_file(
        &state,
        &mirror_manifest_relative_path,
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:2\n",
    )
    .await?;
    let source_session_id = sqlx::query(
        "SELECT id FROM live_ingest_sessions WHERE creator_id = ? AND broadcast_id = ? ORDER BY connected_at DESC LIMIT 1",
    )
    .bind(&creator.id)
    .bind(&session.source_broadcast_id)
    .fetch_one(&state.pool)
    .await?
    .get::<String, _>("id");
    sqlx::query(
        r#"
        INSERT INTO live_runtime_outputs (
            id, session_id, creator_id, broadcast_id, runtime_state, packaging_status,
            archive_status, manifest_relative_path, archive_relative_path, last_error,
            last_runtime_event_at, created_at, updated_at
        ) VALUES (?, ?, ?, ?, 'healthy', 'ready', 'not_started', ?, NULL, NULL, ?, ?, ?)
        "#,
    )
    .bind(format!("lro-test-{}", Uuid::new_v4().simple()))
    .bind(&source_session_id)
    .bind(&creator.id)
    .bind(&session.source_broadcast_id)
    .bind(playback_row.get::<Option<String>, _>("playback_relative_path"))
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO live_runtime_targets (
            id, session_id, creator_id, broadcast_id, target_kind, target_key, target_label,
            route_state, target_creator_id, target_broadcast_id, playback_enabled,
            recording_enabled, mix_minus_required, relative_path, source_participant_ids_json,
            created_at, updated_at
        ) VALUES (?, ?, ?, ?, 'mirror_channel', ?, 'mirror channel', 'active', ?, ?, 1, 0, 0, ?, '[]', ?, ?)
        "#,
    )
    .bind(format!("lrt-test-{}", Uuid::new_v4().simple()))
    .bind(&source_session_id)
    .bind(&creator.id)
    .bind(&session.source_broadcast_id)
    .bind(format!("col-out-mirror-{}", participant.id))
    .bind(&guest_creator.id)
    .bind(&guest_broadcast_id)
    .bind(&mirror_manifest_relative_path)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;
    sqlx::query("UPDATE live_streams SET playback_relative_path = ? WHERE id = ?")
        .bind(&mirror_manifest_relative_path)
        .bind(&guest_stream_id)
        .execute(&state.pool)
        .await?;
    let target = fetch_live_stream_playback_target(&state.pool, &guest_stream_id).await?;
    assert_eq!(target.playback_relative_path, mirror_manifest_relative_path);

    let playback = create_live_playback_session(
        State(state.clone()),
        HeaderMap::new(),
        Path(guest_stream_id.clone()),
    )
    .await?
    .0;
    assert_eq!(playback.session.content_id, guest_stream_id);
    assert_eq!(playback.session.content_kind, "live");
    assert_eq!(playback.content_kind, "live");
    assert!(playback.manifest_url.contains("/api/v1/playback/sessions/"));
    assert_eq!(playback.audio_tracks.len(), target.asset.audio_tracks.len());
    assert_eq!(
        playback.caption_tracks.len(),
        target.asset.caption_tracks.len()
    );
    assert_eq!(
        playback.preview_tracks.len(),
        target.asset.preview_tracks.len()
    );
    assert_eq!(
        playback.default_audio_track_id,
        target.asset.default_audio_track_id
    );
    assert!(
        playback.audio_tracks.iter().all(|track| track.published),
        "live playback should expose published audio tracks"
    );
    assert!(
        playback
            .audio_tracks
            .iter()
            .filter(|track| track.playlist_path.is_some())
            .all(|track| track
                .playlist_url
                .as_deref()
                .is_some_and(|url| url.contains(&playback.playback_token))),
        "live playback track URLs should be tokenized"
    );

    let fetched = get_playback_session(
        State(state.clone()),
        Path(playback.session.id.clone()),
        Query(PlaybackAccessQuery {
            playback_token: Some(playback.playback_token.clone()),
            hls_msn: None,
            hls_part: None,
        }),
    )
    .await?
    .0;
    assert_eq!(fetched.audio_tracks.len(), playback.audio_tracks.len());
    assert_eq!(
        fetched.default_audio_track_id,
        playback.default_audio_track_id
    );

    let refreshed = refresh_playback_session(
        State(state.clone()),
        Path(playback.session.id.clone()),
        Query(PlaybackAccessQuery {
            playback_token: Some(playback.playback_token.clone()),
            hls_msn: None,
            hls_part: None,
        }),
    )
    .await?
    .0;
    assert_ne!(refreshed.playback_token, playback.playback_token);
    assert_eq!(refreshed.audio_tracks.len(), playback.audio_tracks.len());

    let stale_error = get_playback_session(
        State(state.clone()),
        Path(playback.session.id.clone()),
        Query(PlaybackAccessQuery {
            playback_token: Some(playback.playback_token.clone()),
            hls_msn: None,
            hls_part: None,
        }),
    )
    .await
    .expect_err("old live playback token should be invalid after refresh");
    assert!(matches!(stale_error, AppError::Unauthorized));

    let refreshed_fetch = get_playback_session(
        State(state.clone()),
        Path(playback.session.id.clone()),
        Query(PlaybackAccessQuery {
            playback_token: Some(refreshed.playback_token.clone()),
            hls_msn: None,
            hls_part: None,
        }),
    )
    .await?
    .0;
    assert_eq!(refreshed_fetch.playback_token, refreshed.playback_token);
    Ok(())
}

#[tokio::test]
async fn removing_participant_publishes_mirror_grant_revoked_event() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(&state.pool, &creator).await?;
    let headers = auth_headers(&token);
    let (session, participant) =
        insert_active_collaboration_session(&state.pool, &creator, "crt-atlas", "usr-2").await?;
    let grant = issue_mirror_grant_for_participant(&state, &session, &participant, "usr-1").await?;

    let (mut subscription, _) = state
        .realtime
        .join(&collaboration_channel_id(&session.id))
        .await;

    let removed = remove_collaboration_participant(
        State(state.clone()),
        headers,
        Path((session.id.clone(), participant.id.clone())),
    )
    .await?
    .0;
    assert_eq!(removed.state, "removed");

    let mut saw_grant_revoked = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    while tokio::time::Instant::now() < deadline && !saw_grant_revoked {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let event = tokio::time::timeout(remaining, subscription.recv())
            .await
            .map_err(|_| {
                AppError::Internal(
                    "timed out waiting for collaboration mirror grant revoke event".to_string(),
                )
            })?
            .map_err(|error| {
                AppError::Internal(format!(
                    "failed receiving collaboration realtime event: {error}"
                ))
            })?;
        if let WsEvent::CollaborationEvent { event } = event {
            if event.event_type == "mirror_grant_revoked"
                && event.payload["grantId"] == Value::String(grant.id.clone())
                && event.payload["reason"] == Value::String("participant_removed".to_string())
            {
                saw_grant_revoked = true;
            }
        }
    }

    assert!(saw_grant_revoked);
    let events = fetch_collaboration_events(&state.pool, &session.id, 0, 100).await?;
    assert!(events.iter().any(|event| {
        event.event_type == "mirror_grant_revoked"
            && event.payload["grantId"] == Value::String(grant.id.clone())
            && event.payload["reason"] == Value::String("participant_removed".to_string())
    }));

    state
        .realtime
        .leave(&collaboration_channel_id(&session.id))
        .await;
    Ok(())
}
