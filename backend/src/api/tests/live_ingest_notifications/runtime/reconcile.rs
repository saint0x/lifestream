use super::*;

#[test]
fn collaboration_topology_changes_rebuild_runtime_artifacts_for_active_ingest() -> AppResult<()> {
    std::thread::Builder::new()
        .name("runtime-collab-topology-rebuild".to_string())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime")
                .block_on(
                    collaboration_topology_changes_rebuild_runtime_artifacts_for_active_ingest_async(),
                )
        })
        .expect("runtime collab topology rebuild thread")
        .join()
        .expect("runtime collab topology rebuild join")
}

async fn collaboration_topology_changes_rebuild_runtime_artifacts_for_active_ingest_async()
-> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let host_token = insert_creator_auth_session(&state.pool, &creator).await?;
    let host_headers = auth_headers(&host_token);
    let guest_creator = fetch_creator_profile(&state.pool, "crt-atlas").await?;
    let guest_token = insert_creator_auth_session(&state.pool, &guest_creator).await?;
    let guest_headers = auth_headers(&guest_token);
    let broadcast = insert_ready_collaboration_broadcast(&state.pool, &creator).await?;
    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-collab-topology-rebuild".to_string(),
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
    let manifest_relative_path = runtime_manifest_path(&connected.session);
    write_test_media_file(
        &state,
        &manifest_relative_path,
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:2\n",
    )
    .await?;

    let runtime_output = report_live_runtime(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers,
        Json(UpdateLiveRuntimeStateRequest {
            runtime_state: "healthy".to_string(),
            packaging_status: "ready".to_string(),
            archive_status: "not_started".to_string(),
            manifest_relative_path: Some(manifest_relative_path.clone()),
            archive_relative_path: None,
            last_error: None,
        }),
    )
    .await?
    .0;
    assert_eq!(runtime_output.runtime_state, "healthy");
    assert_eq!(runtime_output.packaging_status, "ready");

    let engine_relative_path = format!(
        "runtime/{}/{}/{}/collaboration/engine.json",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );
    assert!(!tokio::fs::try_exists(media_path_for_relative(&state, &engine_relative_path)).await?);

    let session = create_collaboration_session(
        State(state.clone()),
        host_headers.clone(),
        Json(CreateCollaborationSessionRequest {
            broadcast_id: Some(broadcast.id.clone()),
            title: Some("runtime topology rebuild".to_string()),
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
            invitee_user_id: guest_creator.user_id.clone(),
            role: "guest".to_string(),
            mirror_to_guest_channel: true,
            message: Some("join the active runtime".to_string()),
            expires_in_minutes: Some(30),
        }),
    )
    .await?
    .0;
    let participant =
        accept_collaboration_invite(State(state.clone()), guest_headers, Path(invite.id.clone()))
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
            media_transport: None,
            contribution_endpoint_url: None,
            return_endpoint_url: None,
        }),
    )
    .await?
    .0;
    let _grant = crate::api::collabs::issue_collaboration_mirror_grant(
        State(state.clone()),
        host_headers,
        Path((session.id.clone(), participant.id.clone())),
    )
    .await?
    .0;

    let host_program_relative_path = format!(
        "runtime/{}/{}/{}/collaboration/programs/col-program-host-{}.json",
        connected.session.creator_id,
        connected.session.broadcast_id,
        connected.session.id,
        session.id
    );
    let guest_audio_relative_path = format!(
        "runtime/{}/{}/{}/collaboration/audio/{}.json",
        connected.session.creator_id,
        connected.session.broadcast_id,
        connected.session.id,
        participant.id
    );
    let bundle_relative_path = format!(
        "runtime/{}/{}/{}/collaboration/runtime.json",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );
    let media_relative_path = format!(
        "runtime/{}/{}/{}/collaboration/media.json",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );
    let launch_relative_path = format!(
        "runtime/{}/{}/{}/collaboration/launch.json",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );

    for relative_path in [
        &engine_relative_path,
        &host_program_relative_path,
        &guest_audio_relative_path,
        &bundle_relative_path,
        &media_relative_path,
        &launch_relative_path,
    ] {
        assert!(
            tokio::fs::metadata(media_path_for_relative(&state, relative_path))
                .await
                .map_err(AppError::Io)?
                .len()
                > 0,
            "expected runtime artifact to be rebuilt at {relative_path}"
        );
    }

    let refreshed_output = crate::api::control::fetch_live_runtime_output_for_session(
        &state.pool,
        &connected.session.id,
    )
    .await?;
    let refreshed_output = refreshed_output.expect("live runtime output");
    assert_eq!(refreshed_output.runtime_state, "healthy");
    assert_eq!(refreshed_output.packaging_status, "ready");
    assert!(refreshed_output.last_error.is_none());

    Ok(())
}

#[test]
fn runtime_reconcile_detects_missing_collaboration_engine_artifact() -> AppResult<()> {
    std::thread::Builder::new()
        .name("runtime-reconcile-collab-artifact".to_string())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime")
                .block_on(runtime_reconcile_detects_missing_collaboration_engine_artifact_async())
        })
        .expect("runtime reconcile collab artifact thread")
        .join()
        .expect("runtime reconcile collab artifact join")
}

async fn runtime_reconcile_detects_missing_collaboration_engine_artifact_async() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-collab-runtime-reconcile".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;

    let manifest_relative_path = runtime_manifest_path(&connected.session);
    write_test_media_file(
        &state,
        &manifest_relative_path,
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:2\n",
    )
    .await?;

    let (collaboration_session, collaboration_participant) =
        insert_shared_chat_collaboration_for_current_broadcast(
            &state.pool,
            &creator,
            "crt-atlas",
            "usr-2",
            true,
        )
        .await?;
    let _grant = issue_mirror_grant_for_participant(
        &state,
        &collaboration_session,
        &collaboration_participant,
        &creator.user_id,
    )
    .await?;

    let mut ingest_headers = HeaderMap::new();
    ingest_headers.insert(
        "x-ingest-token",
        HeaderValue::from_str(&connected.ingest_token).unwrap(),
    );
    let output = report_live_runtime(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers,
        Json(UpdateLiveRuntimeStateRequest {
            runtime_state: "healthy".to_string(),
            packaging_status: "ready".to_string(),
            archive_status: "not_started".to_string(),
            manifest_relative_path: Some(manifest_relative_path.clone()),
            archive_relative_path: None,
            last_error: None,
        }),
    )
    .await?
    .0;
    assert_eq!(output.runtime_state, "healthy");

    let engine_relative_path = format!(
        "runtime/{}/{}/{}/collaboration/engine.json",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );
    tokio::fs::remove_file(media_path_for_relative(&state, &engine_relative_path))
        .await
        .map_err(AppError::Io)?;

    let session =
        fetch_live_ingest_session_by_id(&state.pool, &creator.id, &connected.session.id).await?;
    let reconciled = reconcile_live_runtime_output_artifacts(&state, &session)
        .await?
        .expect("reconciled runtime output");
    assert_eq!(reconciled.runtime_state, "packaging_degraded");
    assert_eq!(reconciled.packaging_status, "degraded");
    assert!(
        reconciled
            .last_error
            .as_deref()
            .is_some_and(|error| error.contains("collaboration engine artifact missing"))
    );

    let record =
        fetch_creator_live_ingest_session_record(&state.pool, &creator.id, &connected.session.id)
            .await?;
    assert!(
        record
            .artifact_health
            .as_ref()
            .and_then(|health| health.collaboration.as_ref())
            .is_some_and(|state| state.state == "invalid")
    );
    assert!(
        record
            .recent_events
            .iter()
            .any(|event| event.event_type == "runtime_artifact_reconciled")
    );

    Ok(())
}

#[tokio::test]
async fn runtime_report_reconciles_missing_manifest_into_packaging_drift() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-runtime-drift".to_string(),
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
    let manifest_relative_path = runtime_manifest_path(&connected.session);

    let runtime_output = report_live_runtime(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers,
        Json(UpdateLiveRuntimeStateRequest {
            runtime_state: "healthy".to_string(),
            packaging_status: "ready".to_string(),
            archive_status: "not_started".to_string(),
            manifest_relative_path: Some(manifest_relative_path.clone()),
            archive_relative_path: None,
            last_error: None,
        }),
    )
    .await?
    .0;

    tokio::fs::remove_file(media_path_for_relative(&state, &manifest_relative_path))
        .await
        .map_err(AppError::Io)?;

    assert_eq!(runtime_output.runtime_state, "healthy");
    assert_eq!(runtime_output.packaging_status, "ready");
    assert!(runtime_output.last_error.is_none());

    let session =
        fetch_live_ingest_session_by_id(&state.pool, &creator.id, &connected.session.id).await?;
    let reconciled = reconcile_live_runtime_output_artifacts(&state, &session)
        .await?
        .expect("reconciled runtime output");
    assert_eq!(reconciled.runtime_state, "packaging_degraded");
    assert_eq!(reconciled.packaging_status, "degraded");
    assert!(
        reconciled
            .last_error
            .as_deref()
            .is_some_and(|error| error.contains(manifest_relative_path.as_str()))
    );

    let record =
        fetch_creator_live_ingest_session_record(&state.pool, &creator.id, &connected.session.id)
            .await?;
    assert!(
        record
            .recent_events
            .iter()
            .any(|event| event.event_type == "runtime_artifact_reconciled")
    );
    assert!(
        record
            .recent_telemetry
            .iter()
            .any(|sample| sample.sample_kind == "runtime_artifact_reconciled")
    );
    assert_eq!(
        record
            .telemetry_summary
            .runtime_artifact_reconciliation_samples,
        1
    );
    assert_eq!(record.telemetry_summary.artifact_attention_samples, 1);
    assert_eq!(record.telemetry_summary.manifest_path_missing_samples, 1);
    assert_eq!(
        record
            .telemetry_summary
            .last_manifest_artifact_state
            .as_deref(),
        Some("missing")
    );
    assert_eq!(
        record.telemetry_summary.last_advisory_status.as_deref(),
        Some("repairable")
    );

    Ok(())
}

#[tokio::test]
async fn runtime_report_rejects_ready_packaging_without_manifest() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-runtime-invalid-manifest".to_string(),
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
    let manifest_relative_path = runtime_manifest_path(&connected.session);

    let error = report_live_runtime(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers,
        Json(UpdateLiveRuntimeStateRequest {
            runtime_state: "healthy".to_string(),
            packaging_status: "ready".to_string(),
            archive_status: "not_started".to_string(),
            manifest_relative_path: Some("live/wrong/path/master.m3u8".to_string()),
            archive_relative_path: None,
            last_error: None,
        }),
    )
    .await
    .expect_err("ready packaging with a non-canonical manifest must be rejected");
    assert!(
        matches!(error, AppError::BadRequest(message) if message.contains("backend-owned runtime path") && message.contains(manifest_relative_path.as_str()))
    );

    Ok(())
}

#[tokio::test]
async fn runtime_report_rejects_recovery_to_healthy_after_termination() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(&state.pool, &creator).await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-runtime-invalid-terminal".to_string(),
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
    let manifest_relative_path = runtime_manifest_path(&connected.session);
    write_test_media_file(
        &state,
        &manifest_relative_path,
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:2\n",
    )
    .await?;

    let _ = report_live_runtime(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers.clone(),
        Json(UpdateLiveRuntimeStateRequest {
            runtime_state: "healthy".to_string(),
            packaging_status: "ready".to_string(),
            archive_status: "not_started".to_string(),
            manifest_relative_path: Some(manifest_relative_path.clone()),
            archive_relative_path: None,
            last_error: None,
        }),
    )
    .await?;

    let _ = terminate_creator_live_ingest(
        State(state.clone()),
        auth_headers(&token),
        Path(connected.session.id.clone()),
        Json(TerminateLiveIngestRequest {
            reason: Some("terminal runtime transition validation".to_string()),
        }),
    )
    .await?;

    let error = report_live_runtime(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers,
        Json(UpdateLiveRuntimeStateRequest {
            runtime_state: "healthy".to_string(),
            packaging_status: "ready".to_string(),
            archive_status: "not_started".to_string(),
            manifest_relative_path: Some(manifest_relative_path.clone()),
            archive_relative_path: None,
            last_error: None,
        }),
    )
    .await
    .expect_err("terminal ingest sessions must not resume healthy runtime reports");
    assert!(
        matches!(error, AppError::BadRequest(message) if message.contains("terminal ingest sessions"))
    );

    Ok(())
}

#[tokio::test]
async fn runtime_report_allows_archive_completion_after_disconnect() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-runtime-archive-complete".to_string(),
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
    let manifest_relative_path = runtime_manifest_path(&connected.session);
    let archive_relative_path = runtime_archive_path(&connected.session);
    write_test_media_file(
        &state,
        &manifest_relative_path,
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:2\n",
    )
    .await?;
    write_test_media_file(&state, &archive_relative_path, "archive-complete").await?;

    let _ = report_live_runtime(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers.clone(),
        Json(UpdateLiveRuntimeStateRequest {
            runtime_state: "healthy".to_string(),
            packaging_status: "ready".to_string(),
            archive_status: "not_started".to_string(),
            manifest_relative_path: Some(manifest_relative_path.clone()),
            archive_relative_path: None,
            last_error: None,
        }),
    )
    .await?;

    let _ = disconnect_live_ingest(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers.clone(),
    )
    .await?;

    let output = report_live_runtime(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers,
        Json(UpdateLiveRuntimeStateRequest {
            runtime_state: "archive_complete".to_string(),
            packaging_status: "ready".to_string(),
            archive_status: "complete".to_string(),
            manifest_relative_path: Some(manifest_relative_path.clone()),
            archive_relative_path: Some(archive_relative_path.clone()),
            last_error: None,
        }),
    )
    .await?
    .0;

    assert_eq!(output.runtime_state, "archive_complete");
    assert_eq!(output.archive_status, "complete");
    assert_eq!(
        output.archive_relative_path.as_deref(),
        Some(archive_relative_path.as_str())
    );

    Ok(())
}

#[tokio::test]
async fn background_runtime_reconciliation_promotes_archive_finalizing_to_complete() -> AppResult<()>
{
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-runtime-archive-reconcile".to_string(),
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
    let manifest_relative_path = runtime_manifest_path(&connected.session);
    let archive_relative_path = runtime_archive_path(&connected.session);
    write_test_media_file(
        &state,
        &manifest_relative_path,
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:2\n",
    )
    .await?;
    write_test_media_file(&state, &archive_relative_path, "archive-complete").await?;

    let _ = report_live_runtime(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers.clone(),
        Json(UpdateLiveRuntimeStateRequest {
            runtime_state: "healthy".to_string(),
            packaging_status: "ready".to_string(),
            archive_status: "not_started".to_string(),
            manifest_relative_path: Some(manifest_relative_path.clone()),
            archive_relative_path: None,
            last_error: None,
        }),
    )
    .await?;

    let _ = disconnect_live_ingest(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers,
    )
    .await?;

    sqlx::query(
        "UPDATE live_runtime_outputs SET runtime_state = 'archive_finalizing', packaging_status = 'ready', archive_status = 'finalizing', archive_relative_path = ?, last_error = ?, updated_at = ?, last_runtime_event_at = ? WHERE session_id = ?",
    )
    .bind(&archive_relative_path)
    .bind("waiting for final archive closeout")
    .bind(Utc::now().to_rfc3339())
    .bind(Utc::now().to_rfc3339())
    .bind(&connected.session.id)
    .execute(&state.pool)
    .await?;

    let (mut subscription, _) = state
        .realtime
        .join(&creator_live_channel_id(&creator.id))
        .await;

    let reconciled = reconcile_live_runtime_output_artifacts_background(state.clone()).await?;
    assert!(reconciled >= 1);

    let record =
        fetch_creator_live_ingest_session_record(&state.pool, &creator.id, &connected.session.id)
            .await?;
    let output = record.runtime_output.expect("runtime output should exist");
    assert_eq!(output.runtime_state, "archive_complete");
    assert_eq!(output.packaging_status, "ready");
    assert_eq!(output.archive_status, "complete");
    assert_eq!(
        output.archive_relative_path.as_deref(),
        Some(archive_relative_path.as_str())
    );
    assert!(output.last_error.is_none());
    assert!(
        record
            .recent_events
            .iter()
            .any(|event| event.event_type == "runtime_archive_completed")
    );
    assert!(
        record
            .recent_telemetry
            .iter()
            .any(|sample| sample.sample_kind == "runtime_archive_completed")
    );
    assert_eq!(
        record.telemetry_summary.runtime_archive_completion_samples,
        1
    );
    assert_eq!(
        record
            .telemetry_summary
            .last_manifest_artifact_state
            .as_deref(),
        Some("declared")
    );
    assert_eq!(
        record
            .telemetry_summary
            .last_archive_artifact_state
            .as_deref(),
        Some("declared")
    );
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
    match published {
        WsEvent::CreatorLiveState { control, runtime } => {
            assert_eq!(control.snapshot.profile.id, creator.id);
            assert!(runtime.active_runtime_output.is_none());
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
async fn runtime_termination_closes_session_with_distinct_terminal_event() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-runtime-terminate".to_string(),
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
    let terminated = terminate_live_ingest(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers,
        Json(TerminateLiveIngestRequest {
            reason: Some("encoder exited fatally".to_string()),
        }),
    )
    .await?
    .0;

    assert_eq!(terminated.status, "terminated");

    let record =
        fetch_creator_live_ingest_session_record(&state.pool, &creator.id, &connected.session.id)
            .await?;
    let runtime_output = record.runtime_output.expect("runtime output should exist");

    assert_eq!(runtime_output.runtime_state, "disconnected");
    assert!(
        record
            .recent_events
            .iter()
            .any(|event| event.event_type == "runtime_terminated"
                && event.payload["details"]["reason"]
                    == Value::String("encoder exited fatally".to_string()))
    );
    assert!(
        record
            .recent_telemetry
            .iter()
            .any(|sample| sample.sample_kind == "session_state"
                && sample.detail["eventType"] == Value::String("runtime_terminated".to_string()))
    );

    Ok(())
}

#[tokio::test]
async fn background_runtime_artifact_reconciliation_repairs_missing_manifest_without_reads()
-> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-runtime-background".to_string(),
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
    let manifest_relative_path = runtime_manifest_path(&connected.session);

    let _ = report_live_runtime(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers,
        Json(UpdateLiveRuntimeStateRequest {
            runtime_state: "healthy".to_string(),
            packaging_status: "ready".to_string(),
            archive_status: "not_started".to_string(),
            manifest_relative_path: Some(manifest_relative_path.clone()),
            archive_relative_path: None,
            last_error: None,
        }),
    )
    .await?;

    tokio::fs::remove_file(media_path_for_relative(&state, &manifest_relative_path))
        .await
        .map_err(AppError::Io)?;

    sqlx::query(
        "UPDATE live_runtime_outputs SET runtime_state = 'healthy', packaging_status = 'ready', archive_status = 'not_started', last_error = NULL WHERE session_id = ?",
    )
    .bind(&connected.session.id)
    .execute(&state.pool)
    .await?;

    let reconciled = reconcile_live_runtime_output_artifacts_background(state.clone()).await?;
    assert!(reconciled >= 1);

    let record =
        fetch_creator_live_ingest_session_record(&state.pool, &creator.id, &connected.session.id)
            .await?;
    let output = record.runtime_output.expect("runtime output should exist");
    assert_eq!(output.runtime_state, "packaging_degraded");
    assert_eq!(output.packaging_status, "degraded");
    assert!(
        output
            .last_error
            .as_deref()
            .is_some_and(|error| error.contains(manifest_relative_path.as_str()))
    );
    assert!(
        record
            .recent_events
            .iter()
            .any(|event| event.event_type == "runtime_artifact_reconciled")
    );
    assert_eq!(
        record
            .telemetry_summary
            .runtime_artifact_reconciliation_samples,
        1
    );

    Ok(())
}
