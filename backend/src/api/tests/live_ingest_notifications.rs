use super::*;
use crate::api::ingestctl::canonical_live_runtime_archive_staging_relative_path;
use crate::api::ingestctl::canonical_live_runtime_spec_relative_path;
use crate::api::ingestctl::reconcile_live_runtime_output_artifacts_background;

fn runtime_manifest_path(session: &LiveIngestSession) -> String {
    canonical_live_runtime_manifest_relative_path(session)
}

fn runtime_archive_path(session: &LiveIngestSession) -> String {
    canonical_live_runtime_archive_relative_path(session)
}

fn runtime_archive_staging_path(session: &LiveIngestSession) -> String {
    canonical_live_runtime_archive_staging_relative_path(session)
}

fn runtime_spec_path(session: &LiveIngestSession) -> String {
    canonical_live_runtime_spec_relative_path(session)
}

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
            ingest_latency_ms: None,
            source_probe: None,
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
    assert_eq!(
        record
            .artifact_health
            .as_ref()
            .map(|health| health.status.as_str()),
        Some("pending")
    );
    assert_eq!(
        record
            .artifact_health
            .as_ref()
            .map(|health| health.manifest.state.as_str()),
        Some("pending")
    );
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
async fn ingest_heartbeat_persists_source_probe_and_normalized_contribution_state() -> AppResult<()>
{
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(&state.pool, &creator).await?;
    let headers = auth_headers(&token);
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;

    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "SRT".to_string(),
            ingest_server: "test-ingest-srt".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;
    assert_eq!(connected.session.protocol, "srt");
    assert_eq!(connected.session.contribution_class, "srt_caller");
    assert_eq!(connected.session.contribution_state, "awaiting_probe");

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
            bitrate_kbps: 7200,
            viewers: 88,
            dropped_frames: 3,
            cpu_percent: Some(22),
            free_disk_gb: Some(410.5),
            ingest_latency_ms: Some(480),
            source_probe: Some(crate::models::LiveSourceProbeInput {
                container_format: Some("mpegts".to_string()),
                video_codec: Some("h264".to_string()),
                audio_codec: Some("aac".to_string()),
                width: Some(1920),
                height: Some(1080),
                frame_rate: Some(59.94),
                audio_sample_rate_hz: Some(48_000),
                audio_channels: Some(2),
            }),
        }),
    )
    .await?
    .0;

    assert_eq!(heartbeat.contribution_state, "healthy");
    assert_eq!(heartbeat.ingest_latency_ms, Some(480));
    let source_probe = heartbeat
        .source_probe
        .as_ref()
        .expect("heartbeat should persist source probe");
    assert_eq!(source_probe.container_format.as_deref(), Some("mpegts"));
    assert_eq!(source_probe.video_codec.as_deref(), Some("h264"));
    assert_eq!(source_probe.audio_codec.as_deref(), Some("aac"));
    assert_eq!(source_probe.width, Some(1920));
    assert_eq!(source_probe.height, Some(1080));
    assert_eq!(source_probe.audio_sample_rate_hz, Some(48_000));
    assert_eq!(source_probe.audio_channels, Some(2));
    let source_validation = heartbeat
        .source_validation
        .as_ref()
        .expect("heartbeat should persist source validation");
    assert_eq!(source_validation.state, "valid");
    assert!(source_validation.issues.is_empty());

    let record = get_creator_live_ingest_session_by_id(
        State(state.clone()),
        headers,
        Path(connected.session.id.clone()),
    )
    .await?
    .0;
    assert_eq!(record.session.protocol, "srt");
    assert_eq!(record.session.contribution_class, "srt_caller");
    assert_eq!(record.session.contribution_state, "healthy");
    assert_eq!(record.session.ingest_latency_ms, Some(480));
    assert_eq!(record.telemetry_summary.probe_samples, 1);
    assert_eq!(record.telemetry_summary.validation_issue_samples, 0);
    assert_eq!(record.telemetry_summary.repairable_validation_samples, 0);
    assert_eq!(record.telemetry_summary.advisory_critical_samples, 0);
    assert_eq!(record.telemetry_summary.advisory_repairable_samples, 0);
    assert_eq!(record.telemetry_summary.collaboration_samples, 0);
    assert_eq!(
        record.telemetry_summary.last_contribution_state.as_deref(),
        Some("healthy")
    );
    assert_eq!(record.telemetry_summary.last_ingest_latency_ms, Some(480));
    assert!(record.telemetry_summary.last_source_probe_present);
    assert_eq!(
        record.telemetry_summary.last_source_validation_state.as_deref(),
        Some("valid")
    );
    assert_eq!(
        record.telemetry_summary.last_advisory_status.as_deref(),
        Some("healthy")
    );
    assert_eq!(
        record
            .session
            .source_validation
            .as_ref()
            .map(|item| item.state.as_str()),
        Some("valid")
    );
    assert_eq!(record.runtime_advisory.status, "healthy");
    assert!(!record.runtime_advisory.requires_operator_action);
    assert_eq!(record.runtime_advisory.blocking_issue_count, 0);
    assert_eq!(record.runtime_advisory.repairable_issue_count, 0);
    assert_eq!(
        record.recent_telemetry[0].detail["session"]["contributionState"],
        "healthy"
    );
    assert_eq!(
        record.recent_telemetry[0].detail["session"]["sourceProbePresent"],
        true
    );
    assert_eq!(
        record.recent_telemetry[0].detail["session"]["sourceProbe"]["videoCodec"],
        "h264"
    );
    assert_eq!(
        record.recent_telemetry[0].detail["session"]["sourceValidation"]["state"],
        "valid"
    );
    assert_eq!(record.recent_telemetry[0].detail["advisory"]["status"], "healthy");
    assert_eq!(
        record.recent_telemetry[0].detail["runtimeOutput"]["state"],
        "pending_attach"
    );
    assert_eq!(
        record.recent_telemetry[0].detail["outputs"]["playbackReady"],
        false
    );
    assert_eq!(
        record.recent_telemetry[0].detail["collaboration"]["present"],
        false
    );
    assert_eq!(
        record
            .recent_events
            .iter()
            .find(|event| event.event_type == "heartbeat_recorded")
            .and_then(|event| event.payload["contributionState"].as_str()),
        Some("healthy")
    );
    assert_eq!(
        record
            .recent_events
            .iter()
            .find(|event| event.event_type == "heartbeat_recorded")
            .and_then(|event| event.payload["sourceValidation"]["state"].as_str()),
        Some("valid")
    );

    let spec_path = media_path_for_relative(&state, &runtime_spec_path(&connected.session));
    let spec: Value = serde_json::from_str(
        &tokio::fs::read_to_string(&spec_path)
            .await
            .map_err(AppError::Io)?,
    )?;
    assert_eq!(spec["advisory"]["status"], "healthy");
    assert_eq!(spec["session"]["sourceValidation"]["state"], "valid");

    Ok(())
}

#[tokio::test]
async fn ingest_heartbeat_marks_unsupported_source_validation_as_degraded() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let auth_token = insert_creator_auth_session(&state.pool, &creator).await?;
    let headers = auth_headers(&auth_token);
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;

    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-unsupported-source".to_string(),
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
            bitrate_kbps: 4800,
            viewers: 17,
            dropped_frames: 0,
            cpu_percent: Some(14),
            free_disk_gb: Some(256.0),
            ingest_latency_ms: Some(320),
            source_probe: Some(crate::models::LiveSourceProbeInput {
                container_format: Some("mp4".to_string()),
                video_codec: Some("vp9".to_string()),
                audio_codec: Some("aac".to_string()),
                width: Some(1920),
                height: Some(1080),
                frame_rate: Some(30.0),
                audio_sample_rate_hz: Some(48_000),
                audio_channels: Some(2),
            }),
        }),
    )
    .await?
    .0;

    assert_eq!(heartbeat.contribution_state, "degraded");
    let validation = heartbeat
        .source_validation
        .as_ref()
        .expect("unsupported source should persist validation report");
    assert_eq!(validation.state, "unsupported");
    assert!(
        validation
            .issues
            .iter()
            .any(|issue| issue.code == "container_format" && !issue.repairable)
    );
    assert!(
        validation
            .issues
            .iter()
            .any(|issue| issue.code == "video_codec" && !issue.repairable)
    );

    let record = get_creator_live_ingest_session_by_id(
        State(state.clone()),
        headers,
        Path(connected.session.id.clone()),
    )
    .await?
    .0;
    assert_eq!(record.session.contribution_state, "degraded");
    assert_eq!(
        record
            .session
            .source_validation
            .as_ref()
            .map(|item| item.state.as_str()),
        Some("unsupported")
    );
    assert_eq!(record.telemetry_summary.validation_issue_samples, 1);
    assert_eq!(record.telemetry_summary.repairable_validation_samples, 0);
    assert_eq!(
        record.telemetry_summary.last_source_validation_state.as_deref(),
        Some("unsupported")
    );
    assert_eq!(
        record.telemetry_summary.last_advisory_status.as_deref(),
        Some("critical")
    );
    assert_eq!(record.telemetry_summary.advisory_critical_samples, 1);
    assert_eq!(record.telemetry_summary.advisory_repairable_samples, 0);
    assert_eq!(record.runtime_advisory.status, "critical");
    assert!(record.runtime_advisory.requires_operator_action);
    assert!(record
        .runtime_advisory
        .recommended_actions
        .iter()
        .any(|action| action.code == "container_format"));
    assert!(record
        .runtime_advisory
        .recommended_actions
        .iter()
        .any(|action| action.code == "video_codec"));
    assert!(record.recent_telemetry.iter().any(|sample| {
        sample.detail["session"]["contributionState"] == "degraded"
            && sample.detail["session"]["sourceValidation"]["state"] == "unsupported"
            && sample.detail["advisory"]["status"] == "critical"
    }));
    assert_eq!(
        record
            .recent_events
            .iter()
            .find(|event| event.event_type == "heartbeat_recorded")
            .and_then(|event| event.payload["sourceValidation"]["state"].as_str()),
        Some("unsupported")
    );

    let spec_path = media_path_for_relative(&state, &runtime_spec_path(&connected.session));
    let spec: Value = serde_json::from_str(
        &tokio::fs::read_to_string(&spec_path)
            .await
            .map_err(AppError::Io)?,
    )?;
    assert_eq!(spec["advisory"]["status"], "critical");
    assert_eq!(spec["session"]["contributionState"], "degraded");
    assert_eq!(spec["session"]["sourceValidation"]["state"], "unsupported");

    Ok(())
}

#[tokio::test]
async fn ingest_heartbeat_surfaces_repairable_source_validation_to_operator_views() -> AppResult<()>
{
    let (state, creator) = setup_test_state().await?;
    let auth_token = insert_creator_auth_session(&state.pool, &creator).await?;
    let headers = auth_headers(&auth_token);
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;

    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "srt".to_string(),
            ingest_server: "test-ingest-repairable-source".to_string(),
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
            bitrate_kbps: 3600,
            viewers: 9,
            dropped_frames: 0,
            cpu_percent: Some(11),
            free_disk_gb: Some(300.0),
            ingest_latency_ms: Some(260),
            source_probe: Some(crate::models::LiveSourceProbeInput {
                container_format: Some("mpegts".to_string()),
                video_codec: Some("h264".to_string()),
                audio_codec: Some("aac".to_string()),
                width: Some(1280),
                height: Some(720),
                frame_rate: Some(10.0),
                audio_sample_rate_hz: Some(96_000),
                audio_channels: Some(6),
            }),
        }),
    )
    .await?
    .0;

    assert_eq!(heartbeat.contribution_state, "healthy");
    assert_eq!(
        heartbeat
            .source_validation
            .as_ref()
            .map(|item| item.state.as_str()),
        Some("repairable")
    );

    let record = get_creator_live_ingest_session_by_id(
        State(state.clone()),
        headers.clone(),
        Path(connected.session.id.clone()),
    )
    .await?
    .0;
    assert_eq!(
        record
            .session
            .source_validation
            .as_ref()
            .map(|item| item.state.as_str()),
        Some("repairable")
    );
    assert_eq!(record.telemetry_summary.validation_issue_samples, 1);
    assert_eq!(record.telemetry_summary.repairable_validation_samples, 1);
    assert_eq!(record.telemetry_summary.advisory_critical_samples, 0);
    assert_eq!(record.telemetry_summary.advisory_repairable_samples, 1);
    assert_eq!(
        record.telemetry_summary.last_source_validation_state.as_deref(),
        Some("repairable")
    );
    assert_eq!(
        record.telemetry_summary.last_advisory_status.as_deref(),
        Some("repairable")
    );
    assert_eq!(record.runtime_advisory.status, "repairable");
    assert!(!record.runtime_advisory.requires_operator_action);
    assert!(record
        .runtime_advisory
        .recommended_actions
        .iter()
        .any(|action| action.code == "frame_rate_out_of_range"));
    assert!(record
        .runtime_advisory
        .recommended_actions
        .iter()
        .any(|action| action.code == "audio_sample_rate_nonstandard"));
    assert!(record
        .runtime_advisory
        .recommended_actions
        .iter()
        .any(|action| action.code == "audio_channels_excessive"));

    let runtime = fetch_creator_live_runtime_response(&state.pool, &creator.id).await?;
    assert_eq!(runtime.runtime_advisory.status, "repairable");
    assert_eq!(
        runtime
            .artifact_health
            .as_ref()
            .map(|health| health.status.as_str()),
        Some("pending")
    );
    assert_eq!(runtime.telemetry_summary.last_advisory_status.as_deref(), Some("repairable"));
    assert!(runtime.recent_telemetry.iter().any(|sample| {
        sample.detail["session"]["sourceValidation"]["state"] == "repairable"
            && sample.detail["advisory"]["status"] == "repairable"
    }));

    let spec_path = media_path_for_relative(&state, &runtime_spec_path(&connected.session));
    let spec: Value = serde_json::from_str(
        &tokio::fs::read_to_string(&spec_path)
            .await
            .map_err(AppError::Io)?,
    )?;
    assert_eq!(spec["advisory"]["status"], "repairable");
    assert_eq!(spec["session"]["sourceValidation"]["state"], "repairable");

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
    assert_eq!(
        record
            .artifact_health
            .as_ref()
            .map(|health| health.status.as_str()),
        Some("pending")
    );
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

#[tokio::test]
async fn runtime_reports_persist_output_state_for_creator_and_terminal_session() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let auth_token = insert_creator_auth_session(&state.pool, &creator).await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-runtime-state".to_string(),
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
    assert_eq!(runtime_output.archive_status, "not_started");
    assert_eq!(
        runtime_output.manifest_relative_path.as_deref(),
        Some(manifest_relative_path.as_str())
    );

    let runtime = fetch_creator_live_runtime_response(&state.pool, &creator.id).await?;
    let active_runtime_output = runtime
        .active_runtime_output
        .expect("active runtime output");
    assert_eq!(active_runtime_output.session_id, connected.session.id);
    assert_eq!(active_runtime_output.runtime_state, "healthy");
    assert_eq!(active_runtime_output.packaging_status, "ready");
    assert!(!runtime.active_runtime_targets.is_empty());
    assert!(runtime.active_runtime_targets.iter().any(|target| {
        target.target_kind == "archive"
            && target.route_state == "not_started"
            && target.recording_enabled
    }));
    assert!(runtime.telemetry_summary.total_samples >= 2);
    assert!(runtime.telemetry_summary.degraded_samples >= 0);
    assert!(runtime.telemetry_summary.packaging_degraded_samples >= 0);
    assert!(runtime.telemetry_summary.archive_failure_samples >= 0);
    assert!(runtime.telemetry_summary.reconnect_events >= 0);
    assert!(runtime.telemetry_summary.total_dropped_frames >= 0);
    assert_eq!(
        runtime.telemetry_summary.last_packaging_status.as_deref(),
        Some("ready")
    );
    assert_eq!(runtime.telemetry_summary.peak_runtime_target_count, 1);
    assert_eq!(runtime.telemetry_summary.peak_playback_target_count, 0);
    assert_eq!(runtime.telemetry_summary.peak_recording_target_count, 1);
    assert_eq!(runtime.telemetry_summary.peak_variant_target_count, 0);
    assert_eq!(runtime.telemetry_summary.peak_collaboration_target_count, 1);
    assert_eq!(runtime.telemetry_summary.last_runtime_target_count, Some(1));
    assert_eq!(runtime.telemetry_summary.last_playback_target_count, Some(0));
    assert_eq!(runtime.telemetry_summary.last_recording_target_count, Some(1));
    assert_eq!(runtime.telemetry_summary.last_variant_target_count, Some(0));
    assert_eq!(runtime.telemetry_summary.last_collaboration_target_count, Some(1));
    assert!(!runtime.recent_runtime_outputs.is_empty());
    assert_eq!(
        runtime.recent_runtime_outputs[0].session_id,
        connected.session.id
    );
    assert!(!runtime.recent_runtime_targets.is_empty());
    assert!(
        runtime
            .recent_runtime_targets
            .iter()
            .all(|target| target.session_id == connected.session.id)
    );
    assert!(runtime.recent_telemetry.len() >= 2);
    assert_eq!(runtime.recent_telemetry[0].sample_kind, "runtime_report");
    assert_eq!(runtime.recent_telemetry[0].detail["targets"]["count"], 1);
    assert_eq!(
        runtime.recent_telemetry[0].detail["targets"]["recordingEnabledCount"],
        1
    );
    assert_eq!(
        runtime.recent_telemetry[0].detail["targets"]["playbackEnabledCount"],
        0
    );
    assert_eq!(runtime.recent_telemetry[0].detail["targets"]["variantCount"], 0);

    let record =
        fetch_creator_live_ingest_session_record(&state.pool, &creator.id, &connected.session.id)
            .await?;
    assert_eq!(
        record
            .runtime_output
            .as_ref()
            .and_then(|item| item.manifest_relative_path.as_deref()),
        Some(manifest_relative_path.as_str())
    );
    assert!(
        record
            .recent_events
            .iter()
            .any(|event| event.event_type == "runtime_reported")
    );
    assert!(
        record
            .recent_events
            .iter()
            .any(|event| event.event_type == "runtime_targets_synced")
    );
    assert!(!record.runtime_targets.is_empty());
    assert!(record.runtime_targets.iter().any(|target| {
        target.target_kind == "archive"
            && target.target_key == "primary"
            && target.recording_enabled
    }));
    assert_eq!(record.recent_telemetry.len(), 2);
    assert!(
        record
            .recent_telemetry
            .iter()
            .any(|item| item.sample_kind == "session_connected")
    );
    assert!(record.telemetry_summary.total_samples >= 2);
    assert_eq!(record.telemetry_summary.degraded_samples, 0);
    assert_eq!(record.telemetry_summary.packaging_degraded_samples, 0);
    assert_eq!(record.telemetry_summary.archive_failure_samples, 0);
    assert_eq!(
        record.telemetry_summary.last_runtime_state.as_deref(),
        Some("healthy")
    );

    let terminated = terminate_creator_live_ingest(
        State(state.clone()),
        auth_headers(&auth_token),
        Path(connected.session.id.clone()),
        Json(TerminateLiveIngestRequest {
            reason: Some("runtime state persistence validation".to_string()),
        }),
    )
    .await?
    .0;
    assert_eq!(terminated.status, "terminated");

    let terminal_record =
        fetch_creator_live_ingest_session_record(&state.pool, &creator.id, &connected.session.id)
            .await?;
    let terminal_runtime_output = terminal_record
        .runtime_output
        .expect("terminal runtime output");
    assert_eq!(terminal_runtime_output.runtime_state, "disconnected");
    assert_eq!(terminal_runtime_output.packaging_status, "ready");
    assert_eq!(terminal_runtime_output.archive_status, "not_started");
    assert_eq!(
        terminal_runtime_output.manifest_relative_path.as_deref(),
        Some(manifest_relative_path.as_str())
    );
    assert_eq!(
        terminal_record.recent_telemetry[0].sample_kind,
        "session_state"
    );
    assert_eq!(
        terminal_record.recent_telemetry[0].runtime_state,
        "disconnected"
    );
    assert_eq!(
        terminal_record.recent_telemetry[0].packaging_status,
        "ready"
    );
    assert_eq!(
        terminal_record.recent_telemetry[0].archive_status,
        "not_started"
    );

    Ok(())
}

#[tokio::test]
async fn runtime_spec_is_provisioned_and_tracks_live_runtime_transitions() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let auth_token = insert_creator_auth_session(&state.pool, &creator).await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-runtime-spec".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;

    let manifest_relative_path = runtime_manifest_path(&connected.session);
    let archive_relative_path = runtime_archive_path(&connected.session);
    let archive_staging_relative_path = runtime_archive_staging_path(&connected.session);
    let spec_relative_path = runtime_spec_path(&connected.session);
    let spec_full_path = media_path_for_relative(&state, &spec_relative_path);

    let initial_spec: Value = serde_json::from_str(
        &tokio::fs::read_to_string(&spec_full_path)
            .await
            .map_err(AppError::Io)?,
    )?;
    assert_eq!(initial_spec["session"]["id"], connected.session.id);
    assert_eq!(initial_spec["session"]["status"], "connected");
    assert_eq!(initial_spec["session"]["sessionOrdinal"], 1);
    assert_eq!(initial_spec["session"]["reconnectSession"], false);
    assert_eq!(initial_spec["artifactHealth"]["status"], "pending");
    assert_eq!(initial_spec["artifactHealth"]["manifest"]["state"], "pending");
    assert_eq!(initial_spec["artifactHealth"]["archive"]["state"], "pending");
    assert_eq!(initial_spec["runtime"]["state"], "pending_attach");
    assert_eq!(initial_spec["runtime"]["runtimeClass"], "standard_hls");
    assert_eq!(initial_spec["runtime"]["latencyProfile"], "standard");
    assert_eq!(initial_spec["runtime"]["segmentFormat"], "mpegts");
    assert_eq!(initial_spec["runtime"]["partialSegmentsEnabled"], false);
    assert_eq!(initial_spec["runtime"]["blockingReloadEnabled"], false);
    assert_eq!(initial_spec["runtime"]["targetSegmentDurationSec"], 6);
    assert_eq!(initial_spec["runtime"]["holdBackSegments"], 3);
    assert_eq!(initial_spec["runtime"]["discontinuitySequence"], 0);
    assert_eq!(initial_spec["runtime"]["ladderPolicy"], "awaiting_probe");
    assert_eq!(initial_spec["runtime"]["contentClass"], "unknown");
    assert_eq!(
        initial_spec["expectedPaths"]["manifestRelativePath"],
        manifest_relative_path
    );
    assert_eq!(
        initial_spec["expectedPaths"]["archiveRelativePath"],
        archive_relative_path
    );
    assert_eq!(
        initial_spec["expectedPaths"]["specRelativePath"],
        spec_relative_path
    );
    assert_eq!(initial_spec["packaging"]["runtimeClass"], "standard_hls");
    assert_eq!(initial_spec["packaging"]["latencyProfile"], "standard");
    assert_eq!(initial_spec["packaging"]["playlistMode"], "event");
    assert_eq!(initial_spec["packaging"]["segmentFormat"], "mpegts");
    assert_eq!(initial_spec["packaging"]["segmentDurationSec"], 6);
    assert_eq!(initial_spec["packaging"]["partialSegmentsEnabled"], false);
    assert_eq!(initial_spec["packaging"]["blockingReloadEnabled"], false);
    assert_eq!(initial_spec["packaging"]["targetLatencyMs"], 18000);
    assert_eq!(initial_spec["archive"]["enabled"], true);
    assert_eq!(initial_spec["archive"]["recordingMode"], "single_output");
    assert_eq!(initial_spec["archive"]["targetContainer"], "mp4");
    assert_eq!(initial_spec["archive"]["outputCount"], 1);
    assert_eq!(
        initial_spec["archive"]["outputRelativePath"],
        archive_relative_path.clone()
    );
    assert_eq!(
        initial_spec["archive"]["outputRelativePaths"],
        json!([archive_relative_path.clone()])
    );
    assert_eq!(
        initial_spec["reconnectPolicy"]["replacementMode"],
        "new_session_per_reconnect"
    );
    assert_eq!(initial_spec["reconnectPolicy"]["graceWindowSec"], 20);
    assert_eq!(initial_spec["health"]["status"], "critical");
    assert_eq!(initial_spec["health"]["currentCpuPercent"], 0);
    assert_eq!(initial_spec["health"]["currentFreeDiskGb"], 0.0);
    assert_eq!(initial_spec["health"]["cpuWarnPercent"], 85);
    assert_eq!(initial_spec["health"]["cpuCriticalPercent"], 95);
    assert_eq!(initial_spec["health"]["freeDiskWarnGb"], 20.0);
    assert_eq!(initial_spec["health"]["freeDiskCriticalGb"], 5.0);
    assert_eq!(initial_spec["health"]["ingestLatencyWarnMs"], 1500);
    assert_eq!(initial_spec["health"]["ingestLatencyCriticalMs"], 3000);
    assert_eq!(initial_spec["health"]["droppedFramesWarn"], 100);
    assert_eq!(initial_spec["health"]["droppedFramesCritical"], 1000);
    assert_eq!(
        initial_spec["packaging"]["variantStrategy"],
        "awaiting_probe"
    );
    assert_eq!(initial_spec["packaging"]["ladderPolicy"], "awaiting_probe");
    assert_eq!(initial_spec["packaging"]["contentClass"], "unknown");
    assert_eq!(initial_spec["packaging"]["discontinuitySequence"], 0);
    assert_eq!(initial_spec["packaging"]["variants"], json!([]));
    assert!(initial_spec["collaboration"].is_null());
    assert_eq!(
        initial_spec["telemetry"]["heartbeatSampleKind"],
        "heartbeat"
    );
    assert!(
        tokio::fs::try_exists(
            media_path_for_relative(&state, &manifest_relative_path)
                .parent()
                .expect("manifest parent")
        )
        .await?
    );
    assert!(
        tokio::fs::try_exists(
            media_path_for_relative(&state, &archive_relative_path)
                .parent()
                .expect("archive parent")
        )
        .await?
    );
    assert!(
        tokio::fs::try_exists(
            media_path_for_relative(&state, &archive_staging_relative_path)
                .parent()
                .expect("archive staging parent")
        )
        .await?
    );

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

    write_test_media_file(
        &state,
        &manifest_relative_path,
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:2\n",
    )
    .await?;
    let mut ingest_headers = HeaderMap::new();
    ingest_headers.insert(
        "x-ingest-token",
        HeaderValue::from_str(&connected.ingest_token).unwrap(),
    );
    let _ = heartbeat_live_ingest(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers.clone(),
        Json(IngestHeartbeatRequest {
            bitrate_kbps: 7200,
            viewers: 88,
            dropped_frames: 3,
            cpu_percent: Some(18),
            free_disk_gb: Some(410.5),
            ingest_latency_ms: Some(480),
            source_probe: Some(crate::models::LiveSourceProbeInput {
                container_format: Some("mpegts".to_string()),
                video_codec: Some("h264".to_string()),
                audio_codec: Some("aac".to_string()),
                width: Some(1920),
                height: Some(1080),
                frame_rate: Some(59.94),
                audio_sample_rate_hz: Some(48_000),
                audio_channels: Some(2),
            }),
        }),
    )
    .await?;

    let heartbeat_spec: Value = serde_json::from_str(
        &tokio::fs::read_to_string(&spec_full_path)
            .await
            .map_err(AppError::Io)?,
    )?;
    assert_eq!(heartbeat_spec["session"]["contributionState"], "healthy");
    assert_eq!(heartbeat_spec["session"]["ingestLatencyMs"], 480);
    assert_eq!(
        heartbeat_spec["session"]["sourceProbe"]["videoCodec"],
        "h264"
    );
    assert_eq!(heartbeat_spec["artifactHealth"]["status"], "pending");
    assert_eq!(heartbeat_spec["runtime"]["state"], "pending_attach");
    assert_eq!(
        heartbeat_spec["collaboration"]["sessionId"],
        collaboration_session.id
    );
    assert_eq!(heartbeat_spec["collaboration"]["mixMinusRequired"], true);
    assert_eq!(heartbeat_spec["collaboration"]["audioMixMode"], "mix_minus");

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

    let ready_spec: Value = serde_json::from_str(
        &tokio::fs::read_to_string(&spec_full_path)
            .await
            .map_err(AppError::Io)?,
    )?;
    assert_eq!(ready_spec["runtime"]["state"], "healthy");
    assert_eq!(ready_spec["runtime"]["packagingStatus"], "ready");
    assert_eq!(ready_spec["artifactHealth"]["status"], "checked");
    assert_eq!(ready_spec["artifactHealth"]["manifest"]["state"], "valid");
    assert_eq!(ready_spec["artifactHealth"]["archive"]["state"], "pending");
    assert_eq!(ready_spec["runtime"]["runtimeClass"], "standard_hls");
    assert_eq!(ready_spec["runtime"]["latencyProfile"], "standard");
    assert_eq!(ready_spec["runtime"]["segmentFormat"], "mpegts");
    assert_eq!(ready_spec["runtime"]["ladderPolicy"], "probe_high_motion_1080p");
    assert_eq!(ready_spec["runtime"]["contentClass"], "high_motion");
    assert_eq!(
        ready_spec["runtime"]["manifestRelativePath"],
        manifest_relative_path
    );
    assert_eq!(ready_spec["health"]["status"], "ok");
    assert_eq!(ready_spec["health"]["currentCpuPercent"], 18);
    assert_eq!(ready_spec["health"]["currentFreeDiskGb"], 410.5);
    assert_eq!(ready_spec["health"]["currentIngestLatencyMs"], 480);
    assert_eq!(ready_spec["health"]["currentDroppedFrames"], 3);
    assert_eq!(ready_spec["packaging"]["status"], "ready");
    assert_eq!(ready_spec["packaging"]["latencyProfile"], "standard");
    assert_eq!(ready_spec["packaging"]["segmentFormat"], "mpegts");
    assert_eq!(ready_spec["packaging"]["variantStrategy"], "probe_derived");
    assert_eq!(ready_spec["packaging"]["ladderPolicy"], "probe_high_motion_1080p");
    assert_eq!(ready_spec["packaging"]["contentClass"], "high_motion");
    assert_eq!(ready_spec["packaging"]["variants"][0]["label"], "240p");
    assert_eq!(ready_spec["packaging"]["variants"][4]["label"], "1080p");
    assert_eq!(
        ready_spec["collaboration"]["sessionId"],
        collaboration_session.id
    );
    assert_eq!(ready_spec["collaboration"]["status"], "active");
    assert_eq!(
        ready_spec["collaboration"]["sourceBroadcastId"],
        connected.session.broadcast_id
    );
    assert_eq!(ready_spec["collaboration"]["chatMode"], "shared");
    assert_eq!(
        ready_spec["collaboration"]["recordingPolicy"],
        "host_archive"
    );
    assert_eq!(ready_spec["archive"]["recordingMode"], "host_archive");
    assert_eq!(ready_spec["archive"]["outputCount"], 1);
    assert_eq!(ready_spec["collaboration"]["sharedChat"], true);
    assert_eq!(ready_spec["collaboration"]["mixMinusRequired"], true);
    assert_eq!(ready_spec["collaboration"]["audioMixMode"], "mix_minus");
    assert_eq!(
        ready_spec["telemetry"]["runtimeReportSampleKind"],
        "runtime_report"
    );
    assert_eq!(
        ready_spec["collaboration"]["mirroredCreatorIds"][0],
        "crt-atlas"
    );
    assert!(
        ready_spec["collaboration"]["outputs"]
            .as_array()
            .expect("collaboration outputs array")
            .iter()
            .any(|output| {
                output["outputKind"] == "host_channel"
                    && output["routeState"] == "active"
                    && output["targetBroadcastId"] == connected.session.broadcast_id
            })
    );
    assert!(
        ready_spec["collaboration"]["outputs"]
            .as_array()
            .expect("collaboration outputs array")
            .iter()
            .any(|output| {
                output["outputKind"] == "mirror_channel"
                    && output["routeState"] == "issued"
                    && output["targetCreatorId"] == "crt-atlas"
                    && output["mixMinusRequired"] == true
            })
    );
    assert!(
        ready_spec["collaboration"]["programs"]
            .as_array()
            .expect("collaboration programs array")
            .iter()
            .any(|program| {
                program["programKind"] == "host_program"
                    && program["outputIds"]
                        .as_array()
                        .is_some_and(|outputs| {
                            outputs.iter().any(|output_id| {
                                output_id
                                    == &Value::String(format!(
                                        "col-out-host-{}",
                                        collaboration_session.id
                                    ))
                            })
                        })
            })
    );
    assert!(
        ready_spec["collaboration"]["audio"]
            .as_array()
            .expect("collaboration audio routes array")
            .iter()
            .any(|route| {
                route["participantId"] == collaboration_participant.id
                    && route["routeKind"] == "mix_minus_return"
                    && route["receiveProgramAudio"] == true
                    && route["excludedParticipantIds"] == json!([collaboration_participant.id])
            })
    );
    assert!(
        ready_spec["collaboration"]["contributions"]
            .as_array()
            .expect("collaboration contributions array")
            .iter()
            .any(|contribution| {
                contribution["participantId"] == collaboration_participant.id
                    && contribution["mixMinusRequired"] == true
                    && contribution["attachedOutputIds"]
                        .as_array()
                        .is_some_and(|outputs| {
                            outputs.iter().any(|output| {
                                output
                                    == &Value::String(format!(
                                        "col-out-mirror-{}",
                                        collaboration_participant.id
                                    ))
                            })
                        })
            })
    );
    let runtime = fetch_creator_live_runtime_response(&state.pool, &creator.id).await?;
    assert!(runtime.telemetry_summary.probe_samples >= 1);
    assert!(runtime.telemetry_summary.collaboration_samples >= 1);
    assert!(runtime.telemetry_summary.mix_minus_samples >= 1);
    assert!(runtime.telemetry_summary.packaging_ready_samples >= 1);
    assert_eq!(
        runtime
            .telemetry_summary
            .last_collaboration_session_id
            .as_deref(),
        Some(collaboration_session.id.as_str())
    );
    assert_eq!(
        runtime
            .telemetry_summary
            .last_collaboration_participant_count,
        Some(2)
    );
    assert_eq!(runtime.telemetry_summary.last_active_output_routes, Some(1));
    assert_eq!(
        runtime.telemetry_summary.last_audio_mix_mode.as_deref(),
        Some("mix_minus")
    );
    assert_eq!(runtime.telemetry_summary.ll_hls_samples, 0);
    assert_eq!(runtime.telemetry_summary.peak_discontinuity_sequence, 0);
    assert_eq!(
        runtime
            .artifact_health
            .as_ref()
            .map(|health| health.manifest.state.as_str()),
        Some("declared")
    );
    assert_eq!(
        runtime.telemetry_summary.last_runtime_class.as_deref(),
        Some("standard_hls")
    );
    assert_eq!(
        runtime.telemetry_summary.last_latency_profile.as_deref(),
        Some("standard")
    );
    assert_eq!(
        runtime.telemetry_summary.last_ladder_policy.as_deref(),
        Some("probe_high_motion_1080p")
    );
    assert_eq!(
        runtime.telemetry_summary.last_content_class.as_deref(),
        Some("high_motion")
    );
    let playback_enabled_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| target.playback_enabled)
        .count() as i64;
    let recording_enabled_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| target.recording_enabled)
        .count() as i64;
    let variant_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| target.target_kind == "variant")
        .count() as i64;
    let collaboration_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| {
            matches!(
                target.target_kind.as_str(),
                "host_channel" | "mirror_channel" | "archive"
            )
        })
        .count() as i64;
    let host_channel_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| target.target_kind == "host_channel")
        .count() as i64;
    let mirror_channel_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| target.target_kind == "mirror_channel")
        .count() as i64;
    let shared_program_mirror_channel_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| {
            target.target_kind == "mirror_channel"
                && target.mix_minus_required
                && target.source_participant_ids.len() > 1
        })
        .count() as i64;
    let guest_isolated_mirror_channel_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| {
            target.target_kind == "mirror_channel"
                && !(target.mix_minus_required && target.source_participant_ids.len() > 1)
        })
        .count() as i64;
    let archive_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| target.target_kind == "archive")
        .count() as i64;
    let active_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| target.route_state == "active")
        .count() as i64;
    let degraded_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| target.route_state == "degraded")
        .count() as i64;
    let armed_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| target.route_state == "armed")
        .count() as i64;
    let pending_source_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| target.route_state == "pending_source")
        .count() as i64;
    assert_eq!(
        runtime.telemetry_summary.peak_runtime_target_count,
        runtime.active_runtime_targets.len() as i64
    );
    assert_eq!(
        runtime.telemetry_summary.peak_playback_target_count,
        playback_enabled_targets
    );
    assert_eq!(
        runtime.telemetry_summary.peak_recording_target_count,
        recording_enabled_targets
    );
    assert_eq!(
        runtime.telemetry_summary.peak_variant_target_count,
        variant_targets
    );
    assert_eq!(
        runtime.telemetry_summary.peak_collaboration_target_count,
        collaboration_targets
    );
    assert_eq!(
        runtime.telemetry_summary.peak_host_channel_count,
        host_channel_targets
    );
    assert_eq!(
        runtime.telemetry_summary.peak_mirror_channel_count,
        mirror_channel_targets
    );
    assert_eq!(
        runtime
            .telemetry_summary
            .peak_shared_program_mirror_channel_count,
        shared_program_mirror_channel_targets
    );
    assert_eq!(
        runtime
            .telemetry_summary
            .peak_guest_isolated_mirror_channel_count,
        guest_isolated_mirror_channel_targets
    );
    assert_eq!(
        runtime.telemetry_summary.peak_archive_target_count,
        archive_targets
    );
    assert_eq!(
        runtime.telemetry_summary.peak_active_target_count,
        active_targets
    );
    assert_eq!(
        runtime.telemetry_summary.peak_degraded_target_count,
        degraded_targets
    );
    assert_eq!(
        runtime.telemetry_summary.peak_armed_target_count,
        armed_targets
    );
    assert_eq!(
        runtime.telemetry_summary.peak_pending_source_target_count,
        pending_source_targets
    );
    assert_eq!(
        runtime.telemetry_summary.last_runtime_target_count,
        Some(runtime.active_runtime_targets.len() as i64)
    );
    assert_eq!(
        runtime.telemetry_summary.last_playback_target_count,
        Some(playback_enabled_targets)
    );
    assert_eq!(
        runtime.telemetry_summary.last_recording_target_count,
        Some(recording_enabled_targets)
    );
    assert_eq!(
        runtime.telemetry_summary.last_variant_target_count,
        Some(variant_targets)
    );
    assert_eq!(
        runtime.telemetry_summary.last_collaboration_target_count,
        Some(collaboration_targets)
    );
    assert_eq!(
        runtime.telemetry_summary.last_host_channel_count,
        Some(host_channel_targets)
    );
    assert_eq!(
        runtime.telemetry_summary.last_mirror_channel_count,
        Some(mirror_channel_targets)
    );
    assert_eq!(
        runtime
            .telemetry_summary
            .last_shared_program_mirror_channel_count,
        Some(shared_program_mirror_channel_targets)
    );
    assert_eq!(
        runtime
            .telemetry_summary
            .last_guest_isolated_mirror_channel_count,
        Some(guest_isolated_mirror_channel_targets)
    );
    assert_eq!(
        runtime.telemetry_summary.last_archive_target_count,
        Some(archive_targets)
    );
    assert_eq!(
        runtime.telemetry_summary.last_active_target_count,
        Some(active_targets)
    );
    assert_eq!(
        runtime.telemetry_summary.last_degraded_target_count,
        Some(degraded_targets)
    );
    assert_eq!(
        runtime.telemetry_summary.last_armed_target_count,
        Some(armed_targets)
    );
    assert_eq!(
        runtime.telemetry_summary.last_pending_source_target_count,
        Some(pending_source_targets)
    );
    assert!(runtime.recent_telemetry.iter().any(|sample| {
        sample.sample_kind == "runtime_report"
            && sample.detail["collaboration"]["present"] == true
            && sample.detail["collaboration"]["sessionId"] == collaboration_session.id
            && sample.detail["outputs"]["activeRouteCount"] == 1
            && sample.detail["delivery"]["runtimeClass"] == "standard_hls"
            && sample.detail["delivery"]["segmentFormat"] == "mpegts"
            && sample.detail["delivery"]["ladderPolicy"] == "probe_high_motion_1080p"
            && sample.detail["targets"]["count"] == runtime.active_runtime_targets.len() as i64
            && sample.detail["targets"]["variantCount"] == variant_targets
            && sample.detail["targets"]["playbackEnabledCount"] == playback_enabled_targets
            && sample.detail["targets"]["recordingEnabledCount"] == recording_enabled_targets
            && sample.detail["targets"]["collaborationCount"] == collaboration_targets
            && sample.detail["targets"]["hostChannelCount"] == host_channel_targets
            && sample.detail["targets"]["mirrorChannelCount"] == mirror_channel_targets
            && sample.detail["targets"]["sharedProgramMirrorChannelCount"]
                == shared_program_mirror_channel_targets
            && sample.detail["targets"]["guestIsolatedMirrorChannelCount"]
                == guest_isolated_mirror_channel_targets
            && sample.detail["targets"]["archiveCount"] == archive_targets
            && sample.detail["targets"]["activeCount"] == active_targets
            && sample.detail["targets"]["degradedCount"] == degraded_targets
            && sample.detail["targets"]["armedCount"] == armed_targets
            && sample.detail["targets"]["pendingSourceCount"] == pending_source_targets
    }));
    let expected_variant_playlist = format!(
        "live/{}/{}/{}/1080p/playlist.m3u8",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );
    let expected_mirror_playlist = format!(
        "live/{}/{}/col-out-mirror-{}/master.m3u8",
        collaboration_participant
            .creator_id
            .as_deref()
            .expect("guest creator id"),
        collaboration_session.source_broadcast_id,
        collaboration_participant.id
    );
    let expected_host_route_archive = format!(
        "archive/{}/{}/col-out-archive-host-{}/final.mp4",
        creator.id, collaboration_session.source_broadcast_id, collaboration_session.id
    );
    assert_eq!(
        ready_spec["archive"]["outputRelativePath"],
        expected_host_route_archive.clone()
    );
    assert_eq!(
        ready_spec["archive"]["outputRelativePaths"],
        json!([expected_host_route_archive.clone()])
    );
    assert!(runtime.active_runtime_targets.iter().any(|target| {
        target.target_kind == "variant"
            && target.target_key == "1080p"
            && target.relative_path.as_deref() == Some(expected_variant_playlist.as_str())
    }));
    assert!(runtime.active_runtime_targets.iter().any(|target| {
        target.target_kind == "host_channel"
            && target.route_state == "active"
            && target.relative_path.as_deref() == Some(manifest_relative_path.as_str())
            && target.target_broadcast_id.as_deref() == Some(connected.session.broadcast_id.as_str())
    }));
    assert!(runtime.active_runtime_targets.iter().any(|target| {
        target.target_kind == "mirror_channel"
            && target.target_key == format!("col-out-mirror-{}", collaboration_participant.id)
            && target.target_creator_id.as_deref()
                == collaboration_participant.creator_id.as_deref()
            && target.relative_path.as_deref() == Some(expected_mirror_playlist.as_str())
            && target.mix_minus_required
    }));
    assert!(runtime.active_runtime_targets.iter().any(|target| {
        target.target_kind == "archive"
            && target.target_key == format!("col-out-archive-host-{}", collaboration_session.id)
            && target.relative_path.as_deref() == Some(expected_host_route_archive.as_str())
    }));
    assert!(!runtime.active_runtime_targets.iter().any(|target| {
        target.target_kind == "archive" && target.target_key == "primary"
    }));
    assert!(
        tokio::fs::metadata(media_path_for_relative(&state, &expected_mirror_playlist))
            .await
            .map_err(AppError::Io)?
            .len()
            > 0
    );
    assert!(
        tokio::fs::metadata(media_path_for_relative(&state, &expected_host_route_archive))
            .await
            .map_err(AppError::Io)?
            .len()
            > 0
    );
    let record =
        fetch_creator_live_ingest_session_record(&state.pool, &creator.id, &connected.session.id)
            .await?;
    assert!(
        record
            .recent_events
            .iter()
            .any(|event| event.event_type == "runtime_targets_synced")
    );
    assert_eq!(
        ready_spec["packaging"]["variants"][4]["outputRelativeDir"],
        format!(
            "live/{}/{}/{}/1080p",
            connected.session.creator_id, connected.session.broadcast_id, connected.session.id
        )
    );
    assert_eq!(
        ready_spec["packaging"]["variants"][4]["relativePlaylistPath"],
        format!(
            "live/{}/{}/{}/1080p/playlist.m3u8",
            connected.session.creator_id, connected.session.broadcast_id, connected.session.id
        )
    );
    assert_eq!(
        ready_spec["packaging"]["variants"][4]["segmentRelativePattern"],
        format!(
            "live/{}/{}/{}/1080p/segment_%03d.ts",
            connected.session.creator_id, connected.session.broadcast_id, connected.session.id
        )
    );
    assert_eq!(
        ready_spec["archive"]["stagingRelativePath"],
        archive_staging_relative_path
    );
    assert_eq!(ready_spec["archive"]["status"], "not_started");
    assert!(
        tokio::fs::try_exists(
            media_path_for_relative(
                &state,
                ready_spec["packaging"]["variants"][4]["relativePlaylistPath"]
                    .as_str()
                    .expect("playlist path in spec"),
            )
            .parent()
            .expect("variant playlist parent")
        )
        .await?
    );

    let _ = terminate_creator_live_ingest(
        State(state.clone()),
        auth_headers(&auth_token),
        Path(connected.session.id.clone()),
        Json(TerminateLiveIngestRequest {
            reason: Some("runtime spec sync validation".to_string()),
        }),
    )
    .await?;

    let terminal_spec: Value = serde_json::from_str(
        &tokio::fs::read_to_string(&spec_full_path)
            .await
            .map_err(AppError::Io)?,
    )?;
    assert_eq!(terminal_spec["session"]["status"], "terminated");
    assert_eq!(terminal_spec["runtime"]["state"], "disconnected");
    assert_eq!(terminal_spec["runtime"]["packagingStatus"], "ready");
    assert_eq!(terminal_spec["archive"]["status"], "not_started");
    assert_eq!(
        terminal_spec["expectedPaths"]["archiveRelativePath"],
        archive_relative_path
    );

    Ok(())
}

#[tokio::test]
async fn ll_hls_delivery_profile_persists_into_runtime_output_spec_and_telemetry() -> AppResult<()>
{
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    sqlx::query("UPDATE creator_live_settings SET delivery_class = 'll_hls' WHERE creator_id = ?")
        .bind(&creator.id)
        .execute(&state.pool)
        .await?;

    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-ll-hls".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;

    let spec_relative_path = runtime_spec_path(&connected.session);
    let spec_full_path = media_path_for_relative(&state, &spec_relative_path);
    let manifest_relative_path = runtime_manifest_path(&connected.session);
    let variant_playlist_relative_path = format!(
        "live/{}/{}/{}/720p/playlist.m3u8",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );
    let init_relative_path = format!(
        "live/{}/{}/{}/720p/init.mp4",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );
    let part_relative_path = format!(
        "live/{}/{}/{}/720p/part_000_000.m4s",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );
    let segment_relative_path = format!(
        "live/{}/{}/{}/720p/segment_000.m4s",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );
    let mut ingest_headers = HeaderMap::new();
    ingest_headers.insert(
        "x-ingest-token",
        HeaderValue::from_str(&connected.ingest_token).unwrap(),
    );
    let _ = heartbeat_live_ingest(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers.clone(),
        Json(IngestHeartbeatRequest {
            bitrate_kbps: 5400,
            viewers: 21,
            dropped_frames: 0,
            cpu_percent: Some(12),
            free_disk_gb: Some(320.0),
            ingest_latency_ms: Some(220),
            source_probe: Some(crate::models::LiveSourceProbeInput {
                container_format: Some("mpegts".to_string()),
                video_codec: Some("h264".to_string()),
                audio_codec: Some("aac".to_string()),
                width: Some(1280),
                height: Some(720),
                frame_rate: Some(30.0),
                audio_sample_rate_hz: Some(48_000),
                audio_channels: Some(2),
            }),
        }),
    )
    .await?;

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

    assert_eq!(output.runtime_class, "ll_hls");
    assert_eq!(output.latency_profile, "low");
    assert_eq!(output.segment_format, "fmp4");
    assert!(output.partial_segments_enabled);
    assert!(output.blocking_reload_enabled);
    assert_eq!(output.target_segment_duration_sec, 2);
    assert_eq!(output.hold_back_segments, 2);
    assert_eq!(output.ladder_policy, "probe_general_hd");
    assert_eq!(output.content_class, "general_hd");
    let manifest = tokio::fs::read_to_string(media_path_for_relative(&state, &manifest_relative_path))
        .await
        .map_err(AppError::Io)?;
    let variant_playlist = tokio::fs::read_to_string(media_path_for_relative(
        &state,
        &variant_playlist_relative_path,
    ))
    .await
    .map_err(AppError::Io)?;
    assert!(manifest.contains("#EXTM3U"));
    assert!(manifest.contains("720p/playlist.m3u8"));
    assert!(variant_playlist.contains("#EXT-X-SERVER-CONTROL:CAN-BLOCK-RELOAD=YES"));
    assert!(variant_playlist.contains("#EXT-X-PART:"));
    assert!(variant_playlist.contains("segment_000.m4s"));
    assert!(
        tokio::fs::metadata(media_path_for_relative(&state, &init_relative_path))
            .await
            .map_err(AppError::Io)?
            .len()
            > 0
    );
    assert!(
        tokio::fs::metadata(media_path_for_relative(&state, &part_relative_path))
            .await
            .map_err(AppError::Io)?
            .len()
            > 0
    );
    assert!(
        tokio::fs::metadata(media_path_for_relative(&state, &segment_relative_path))
            .await
            .map_err(AppError::Io)?
            .len()
            > 0
    );

    let spec: Value = serde_json::from_str(
        &tokio::fs::read_to_string(&spec_full_path)
            .await
            .map_err(AppError::Io)?,
    )?;
    assert_eq!(spec["runtime"]["runtimeClass"], "ll_hls");
    assert_eq!(spec["runtime"]["latencyProfile"], "low");
    assert_eq!(spec["runtime"]["segmentFormat"], "fmp4");
    assert_eq!(spec["runtime"]["partialSegmentsEnabled"], true);
    assert_eq!(spec["runtime"]["blockingReloadEnabled"], true);
    assert_eq!(spec["runtime"]["targetSegmentDurationSec"], 2);
    assert_eq!(spec["runtime"]["holdBackSegments"], 2);
    assert_eq!(spec["packaging"]["runtimeClass"], "ll_hls");
    assert_eq!(spec["packaging"]["latencyProfile"], "low");
    assert_eq!(spec["packaging"]["segmentFormat"], "fmp4");
    assert_eq!(spec["packaging"]["partialSegmentsEnabled"], true);
    assert_eq!(spec["packaging"]["blockingReloadEnabled"], true);
    assert_eq!(spec["packaging"]["targetLatencyMs"], 4000);
    assert_eq!(spec["packaging"]["ladderPolicy"], "probe_general_hd");
    assert_eq!(spec["packaging"]["contentClass"], "general_hd");
    assert_eq!(
        spec["packaging"]["variants"][3]["segmentRelativePattern"],
        format!(
            "live/{}/{}/{}/720p/segment_%03d.m4s",
            connected.session.creator_id, connected.session.broadcast_id, connected.session.id
        )
    );

    let runtime = fetch_creator_live_runtime_response(&state.pool, &creator.id).await?;
    assert!(runtime.telemetry_summary.ll_hls_samples >= 1);
    assert_eq!(runtime.telemetry_summary.peak_discontinuity_sequence, 0);
    assert_eq!(
        runtime.telemetry_summary.last_runtime_class.as_deref(),
        Some("ll_hls")
    );
    assert_eq!(
        runtime.telemetry_summary.last_latency_profile.as_deref(),
        Some("low")
    );
    assert_eq!(
        runtime.telemetry_summary.last_ladder_policy.as_deref(),
        Some("probe_general_hd")
    );
    assert_eq!(
        runtime.telemetry_summary.last_content_class.as_deref(),
        Some("general_hd")
    );
    assert!(runtime.recent_telemetry.iter().any(|sample| {
        sample.sample_kind == "runtime_report"
            && sample.detail["delivery"]["runtimeClass"] == "ll_hls"
            && sample.detail["delivery"]["segmentFormat"] == "fmp4"
            && sample.detail["delivery"]["partialSegmentsEnabled"] == true
            && sample.detail["delivery"]["ladderPolicy"] == "probe_general_hd"
    }));

    Ok(())
}

#[tokio::test]
async fn live_media_playlist_rewrites_tokenized_ll_hls_uris_and_honors_blocking_reload()
-> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    sqlx::query("UPDATE creator_live_settings SET delivery_class = 'll_hls' WHERE creator_id = ?")
        .bind(&creator.id)
        .execute(&state.pool)
        .await?;

    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-live-playlist-blocking".to_string(),
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
    let variant_playlist_relative_path = format!(
        "live/{}/{}/{}/720p/playlist.m3u8",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );
    let variant_playlist_path = media_path_for_relative(&state, &variant_playlist_relative_path);
    let stream_id = format!("lv-{}-live", creator.handle);

    let _ = heartbeat_live_ingest(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers.clone(),
        Json(IngestHeartbeatRequest {
            bitrate_kbps: 5400,
            viewers: 42,
            dropped_frames: 0,
            cpu_percent: Some(11),
            free_disk_gb: Some(280.0),
            ingest_latency_ms: Some(240),
            source_probe: Some(crate::models::LiveSourceProbeInput {
                container_format: Some("mpegts".to_string()),
                video_codec: Some("h264".to_string()),
                audio_codec: Some("aac".to_string()),
                width: Some(1280),
                height: Some(720),
                frame_rate: Some(30.0),
                audio_sample_rate_hz: Some(48_000),
                audio_channels: Some(2),
            }),
        }),
    )
    .await?;
    let _ = report_live_runtime(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers,
        Json(UpdateLiveRuntimeStateRequest {
            runtime_state: "healthy".to_string(),
            packaging_status: "ready".to_string(),
            archive_status: "not_started".to_string(),
            manifest_relative_path: Some(manifest_relative_path),
            archive_relative_path: None,
            last_error: None,
        }),
    )
    .await?;

    let playback = create_live_playback_session(
        State(state.clone()),
        HeaderMap::new(),
        Path(stream_id),
    )
    .await?
    .0;

    let first_response = serve_media_file(
        State(state.clone()),
        Path(variant_playlist_relative_path.clone()),
        HeaderMap::new(),
        Query(PlaybackAccessQuery {
            playback_token: Some(playback.playback_token.clone()),
            hls_msn: Some(0),
            hls_part: Some(0),
        }),
    )
    .await?;
    let first_body = axum::body::to_bytes(first_response.into_body(), usize::MAX)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let first_text = String::from_utf8(first_body.to_vec())
        .map_err(|error| AppError::Internal(error.to_string()))?;
    assert!(first_text.contains(&format!(
        "init.mp4?playbackToken={}",
        playback.playback_token
    )));
    assert!(first_text.contains(&format!(
        "part_000_000.m4s?playbackToken={}",
        playback.playback_token
    )));
    assert!(first_text.contains(&format!(
        "segment_000.m4s?playbackToken={}",
        playback.playback_token
    )));

    let state_for_update = state.clone();
    let playlist_for_update = variant_playlist_path.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let body = concat!(
            "#EXTM3U\n",
            "#EXT-X-VERSION:9\n",
            "#EXT-X-TARGETDURATION:2\n",
            "#EXT-X-PLAYLIST-TYPE:EVENT\n",
            "#EXT-X-MEDIA-SEQUENCE:1\n",
            "#EXT-X-DISCONTINUITY-SEQUENCE:0\n",
            "#EXT-X-INDEPENDENT-SEGMENTS\n",
            "#EXT-X-MAP:URI=\"init.mp4\"\n",
            "#EXT-X-SERVER-CONTROL:CAN-BLOCK-RELOAD=YES,PART-HOLD-BACK=2.000\n",
            "#EXT-X-PART-INF:PART-TARGET=1.000\n",
            "#EXT-X-PART:DURATION=1.000,URI=\"part_001_000.m4s\",INDEPENDENT=YES\n",
            "#EXTINF:2.000,\n",
            "segment_001.m4s\n"
        );
        let _ = tokio::fs::write(&playlist_for_update, body).await;
        let _ = tokio::fs::write(
            media_path_for_relative(
                &state_for_update,
                &format!(
                    "live/{}/{}/{}/720p/part_001_000.m4s",
                    connected.session.creator_id, connected.session.broadcast_id, connected.session.id
                ),
            ),
            b"updated-part",
        )
        .await;
        let _ = tokio::fs::write(
            media_path_for_relative(
                &state_for_update,
                &format!(
                    "live/{}/{}/{}/720p/segment_001.m4s",
                    connected.session.creator_id, connected.session.broadcast_id, connected.session.id
                ),
            ),
            b"updated-segment",
        )
        .await;
    });

    let started = tokio::time::Instant::now();
    let blocked_response = serve_media_file(
        State(state.clone()),
        Path(variant_playlist_relative_path),
        HeaderMap::new(),
        Query(PlaybackAccessQuery {
            playback_token: Some(playback.playback_token.clone()),
            hls_msn: Some(1),
            hls_part: Some(0),
        }),
    )
    .await?;
    assert!(started.elapsed() >= Duration::from_millis(200));
    let blocked_body = axum::body::to_bytes(blocked_response.into_body(), usize::MAX)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let blocked_text = String::from_utf8(blocked_body.to_vec())
        .map_err(|error| AppError::Internal(error.to_string()))?;
    assert!(blocked_text.contains("#EXT-X-MEDIA-SEQUENCE:1"));
    assert!(blocked_text.contains(&format!(
        "part_001_000.m4s?playbackToken={}",
        playback.playback_token
    )));
    assert!(blocked_text.contains(&format!(
        "segment_001.m4s?playbackToken={}",
        playback.playback_token
    )));

    Ok(())
}

#[tokio::test]
async fn runtime_report_emits_backend_owned_hls_and_archive_artifacts() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-runtime-owned-artifacts".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;

    let manifest_relative_path = runtime_manifest_path(&connected.session);
    let archive_relative_path = runtime_archive_path(&connected.session);
    let staging_relative_path = runtime_archive_staging_path(&connected.session);
    let variant_playlist_relative_path = format!(
        "live/{}/{}/{}/1080p/playlist.m3u8",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );
    let segment_relative_path = format!(
        "live/{}/{}/{}/1080p/segment_000.ts",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );
    let mut ingest_headers = HeaderMap::new();
    ingest_headers.insert(
        "x-ingest-token",
        HeaderValue::from_str(&connected.ingest_token).unwrap(),
    );

    let _ = heartbeat_live_ingest(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers.clone(),
        Json(IngestHeartbeatRequest {
            bitrate_kbps: 7100,
            viewers: 144,
            dropped_frames: 1,
            cpu_percent: Some(14),
            free_disk_gb: Some(480.0),
            ingest_latency_ms: Some(310),
            source_probe: Some(crate::models::LiveSourceProbeInput {
                container_format: Some("mpegts".to_string()),
                video_codec: Some("h264".to_string()),
                audio_codec: Some("aac".to_string()),
                width: Some(1920),
                height: Some(1080),
                frame_rate: Some(60.0),
                audio_sample_rate_hz: Some(48_000),
                audio_channels: Some(2),
            }),
        }),
    )
    .await?;

    let output = report_live_runtime(
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
    .await?
    .0;
    assert_eq!(output.packaging_status, "ready");

    let manifest = tokio::fs::read_to_string(media_path_for_relative(&state, &manifest_relative_path))
        .await
        .map_err(AppError::Io)?;
    let variant_playlist = tokio::fs::read_to_string(media_path_for_relative(
        &state,
        &variant_playlist_relative_path,
    ))
    .await
    .map_err(AppError::Io)?;
    assert!(manifest.contains("1080p/playlist.m3u8"));
    assert!(variant_playlist.contains("#EXT-X-DISCONTINUITY-SEQUENCE:0"));
    assert!(variant_playlist.contains("segment_000.ts"));
    assert!(
        tokio::fs::metadata(media_path_for_relative(&state, &segment_relative_path))
            .await
            .map_err(AppError::Io)?
            .len()
            > 0
    );

    let archive_output = report_live_runtime(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers,
        Json(UpdateLiveRuntimeStateRequest {
            runtime_state: "archive_finalizing".to_string(),
            packaging_status: "ready".to_string(),
            archive_status: "finalizing".to_string(),
            manifest_relative_path: Some(manifest_relative_path.clone()),
            archive_relative_path: Some(archive_relative_path.clone()),
            last_error: None,
        }),
    )
    .await?
    .0;

    assert_eq!(archive_output.runtime_state, "archive_complete");
    assert_eq!(archive_output.archive_status, "complete");
    assert_eq!(
        archive_output.archive_relative_path.as_deref(),
        Some(archive_relative_path.as_str())
    );
    assert!(
        tokio::fs::metadata(media_path_for_relative(&state, &archive_relative_path))
            .await
            .map_err(AppError::Io)?
            .len()
            > 0
    );
    assert!(
        tokio::fs::metadata(media_path_for_relative(&state, &staging_relative_path))
            .await
            .map_err(AppError::Io)?
            .len()
            > 0
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

    assert_eq!(runtime_output.runtime_state, "packaging_degraded");
    assert_eq!(runtime_output.packaging_status, "degraded");
    assert!(
        runtime_output
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
        record.telemetry_summary.runtime_artifact_reconciliation_samples,
        1
    );
    assert_eq!(record.telemetry_summary.artifact_attention_samples, 1);
    assert_eq!(record.telemetry_summary.manifest_path_missing_samples, 1);
    assert_eq!(
        record.telemetry_summary.last_manifest_artifact_state.as_deref(),
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
        record.telemetry_summary.last_manifest_artifact_state.as_deref(),
        Some("declared")
    );
    assert_eq!(
        record.telemetry_summary.last_archive_artifact_state.as_deref(),
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
                && sample.detail["eventType"]
                    == Value::String("runtime_terminated".to_string()))
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
        record.telemetry_summary.runtime_artifact_reconciliation_samples,
        1
    );

    Ok(())
}

#[tokio::test]
async fn creator_runtime_repair_updates_output_and_records_audit_trail() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(&state.pool, &creator).await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-repair-creator".to_string(),
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
    let _ = report_live_runtime(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers,
        Json(UpdateLiveRuntimeStateRequest {
            runtime_state: "degraded".to_string(),
            packaging_status: "degraded".to_string(),
            archive_status: "failed".to_string(),
            manifest_relative_path: Some(manifest_relative_path.clone()),
            archive_relative_path: Some(archive_relative_path.clone()),
            last_error: Some("segment upload stalled".to_string()),
        }),
    )
    .await?;

    let report = repair_creator_live_runtime_output(
        State(state.clone()),
        auth_headers(&token),
        Path(connected.session.id.clone()),
        Json(RepairLiveRuntimeOutputRequest {
            reason: "operator repaired packaging drift".to_string(),
            runtime_state: Some("healthy".to_string()),
            packaging_status: Some("ready".to_string()),
            archive_status: Some("not_started".to_string()),
            manifest_relative_path: None,
            archive_relative_path: None,
            last_error: None,
            clear_manifest_relative_path: false,
            clear_archive_relative_path: true,
            clear_last_error: true,
        }),
    )
    .await?
    .0;

    assert_eq!(report.actor_scope, "creator");
    assert_eq!(report.session_id, connected.session.id);
    assert!(
        report
            .actions
            .iter()
            .any(|action| action.field == "lastError"
                && action.previous_value.as_deref() == Some("segment upload stalled")
                && action.next_value.is_none())
    );
    assert!(
        report
            .actions
            .iter()
            .any(|action| action.field == "archiveRelativePath"
                && action.previous_value.as_deref() == Some(archive_relative_path.as_str())
                && action.next_value.is_none())
    );
    assert_eq!(
        report
            .record
            .runtime_output
            .as_ref()
            .map(|item| item.runtime_state.as_str()),
        Some("healthy")
    );
    assert_eq!(
        report
            .record
            .runtime_output
            .as_ref()
            .map(|item| item.packaging_status.as_str()),
        Some("ready")
    );
    assert_eq!(
        report
            .record
            .runtime_output
            .as_ref()
            .and_then(|item| item.archive_relative_path.as_deref()),
        None
    );
    assert_eq!(
        report.record.recent_telemetry[0].sample_kind,
        "runtime_repair"
    );
    assert!(
        report
            .record
            .recent_events
            .iter()
            .any(|event| event.event_type == "runtime_repaired")
    );

    Ok(())
}

#[tokio::test]
async fn admin_runtime_repair_recovers_missing_runtime_output_row() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(&state.pool, &creator).await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-repair-admin".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;

    sqlx::query("DELETE FROM live_runtime_outputs WHERE session_id = ?")
        .bind(&connected.session.id)
        .execute(&state.pool)
        .await?;
    let manifest_relative_path = runtime_manifest_path(&connected.session);
    write_test_media_file(
        &state,
        &manifest_relative_path,
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:2\n",
    )
    .await?;

    let report = repair_admin_live_runtime_output(
        State(state.clone()),
        auth_headers(&token),
        Path(connected.session.id.clone()),
        Json(RepairLiveRuntimeOutputRequest {
            reason: "rebuild missing runtime output".to_string(),
            runtime_state: Some("healthy".to_string()),
            packaging_status: Some("ready".to_string()),
            archive_status: Some("not_started".to_string()),
            manifest_relative_path: Some(manifest_relative_path.clone()),
            archive_relative_path: None,
            last_error: None,
            clear_manifest_relative_path: false,
            clear_archive_relative_path: false,
            clear_last_error: true,
        }),
    )
    .await?
    .0;

    assert_eq!(report.actor_scope, "admin");
    assert_eq!(
        report
            .record
            .runtime_output
            .as_ref()
            .map(|item| item.runtime_state.as_str()),
        Some("healthy")
    );
    assert_eq!(
        report
            .record
            .runtime_output
            .as_ref()
            .and_then(|item| item.manifest_relative_path.as_deref()),
        Some(manifest_relative_path.as_str())
    );
    assert!(
        report
            .actions
            .iter()
            .any(|action| action.field == "runtimeState")
    );
    assert_eq!(
        report.record.recent_telemetry[0].sample_kind,
        "runtime_repair"
    );
    assert!(
        report
            .record
            .recent_events
            .iter()
            .any(|event| event.event_type == "runtime_repaired")
    );

    Ok(())
}

#[tokio::test]
async fn admin_live_ingest_overview_aggregates_latency_and_creator_breakdown() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(&state.pool, &creator).await?;
    let baseline = get_admin_live_ingest_overview(
        State(state.clone()),
        auth_headers(&token),
        Query(AdminLiveIngestOverviewQuery {
            creator_id: Some(creator.id.clone()),
        }),
    )
    .await?
    .0;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-overview".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;

    let connected_at = (Utc::now() - chrono::Duration::seconds(45)).to_rfc3339();
    let ready_at = (Utc::now() - chrono::Duration::seconds(15)).to_rfc3339();
    sqlx::query(
        "UPDATE live_ingest_sessions SET connected_at = ?, last_heartbeat_at = ? WHERE id = ?",
    )
    .bind(&connected_at)
    .bind(&ready_at)
    .bind(&connected.session.id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE live_runtime_outputs SET runtime_state = 'healthy', packaging_status = 'ready', archive_status = 'finalizing', updated_at = ?, last_runtime_event_at = ? WHERE session_id = ?",
    )
    .bind(&ready_at)
    .bind(&ready_at)
    .bind(&connected.session.id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "INSERT INTO live_runtime_telemetry (id, session_id, creator_id, broadcast_id, sample_kind, runtime_state, packaging_status, archive_status, bitrate_kbps, viewers, dropped_frames, cpu_percent, free_disk_gb, detail_json, collected_at) VALUES (?, ?, ?, ?, 'runtime_report', 'healthy', 'ready', 'not_started', 6400, 120, 0, 31, 500.0, '{}', ?)",
    )
    .bind(format!("lrt-test-{}", Uuid::new_v4().simple()))
    .bind(&connected.session.id)
    .bind(&creator.id)
    .bind(&broadcast.id)
    .bind(&ready_at)
    .execute(&state.pool)
    .await?;

    let overview = get_admin_live_ingest_overview(
        State(state.clone()),
        auth_headers(&token),
        Query(AdminLiveIngestOverviewQuery {
            creator_id: Some(creator.id.clone()),
        }),
    )
    .await?
    .0;

    assert_eq!(overview.active_sessions, 1);
    assert!(overview.ready_outputs >= baseline.ready_outputs + 1);
    assert!(overview.archive_finalizing_outputs >= baseline.archive_finalizing_outputs + 1);
    assert!(overview.artifact_attention_outputs >= baseline.artifact_attention_outputs + 1);
    assert!(
        overview.manifest_path_missing_outputs >= baseline.manifest_path_missing_outputs + 1
    );
    assert!(overview.archive_path_missing_outputs >= baseline.archive_path_missing_outputs + 1);
    assert_eq!(overview.unique_creators, 1);
    assert!(overview.total_samples >= baseline.total_samples + 1);
    assert!(overview.avg_ready_latency_seconds.is_some());
    assert_eq!(overview.creator_breakdown.len(), 1);
    assert_eq!(overview.creator_breakdown[0].creator_id, creator.id);
    assert_eq!(overview.creator_breakdown[0].handle, creator.handle);
    assert_eq!(overview.creator_breakdown[0].active_sessions, 1);
    assert!(overview.creator_breakdown[0].ready_outputs >= baseline.ready_outputs + 1);
    assert!(
        overview.creator_breakdown[0].artifact_attention_outputs
            >= baseline.artifact_attention_outputs + 1
    );
    assert!(
        overview.creator_breakdown[0].manifest_path_missing_outputs
            >= baseline.manifest_path_missing_outputs + 1
    );
    assert!(
        overview.creator_breakdown[0].archive_path_missing_outputs
            >= baseline.archive_path_missing_outputs + 1
    );
    assert_eq!(
        overview.creator_breakdown[0]
            .last_packaging_status
            .as_deref(),
        Some("ready")
    );
    assert_eq!(
        overview.creator_breakdown[0]
            .last_manifest_artifact_state
            .as_deref(),
        Some("missing")
    );
    assert_eq!(
        overview.creator_breakdown[0]
            .last_archive_artifact_state
            .as_deref(),
        Some("missing")
    );

    Ok(())
}
