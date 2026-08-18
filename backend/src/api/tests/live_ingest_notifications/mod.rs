use super::*;
use crate::api::control::canonical_live_runtime_archive_staging_relative_path;
use crate::api::control::canonical_live_runtime_spec_relative_path;
use crate::api::control::reconcile_live_runtime_output_artifacts_background;

mod notify;
mod runtime;
mod stale;

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
    assert_eq!(bootstrap_payload["creator"]["currentBroadcast"], Value::Null);
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
