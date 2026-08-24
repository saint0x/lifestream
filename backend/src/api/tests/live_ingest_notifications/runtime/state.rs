use super::*;

#[tokio::test]
async fn runtime_reports_persist_output_state_for_creator_and_terminal_session() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let auth_token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let broadcast = insert_ready_broadcast(state.db.sqlite_adapter(), &creator).await?;
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

    let runtime =
        fetch_creator_live_runtime_response(state.db.sqlite_adapter(), &creator.id).await?;
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
    assert_eq!(
        runtime.telemetry_summary.last_playback_target_count,
        Some(0)
    );
    assert_eq!(
        runtime.telemetry_summary.last_recording_target_count,
        Some(1)
    );
    assert_eq!(runtime.telemetry_summary.last_variant_target_count, Some(0));
    assert_eq!(
        runtime.telemetry_summary.last_collaboration_target_count,
        Some(1)
    );
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
    assert_eq!(
        runtime.recent_telemetry[0].detail["targets"]["variantCount"],
        0
    );

    let record = fetch_creator_live_ingest_session_record(
        state.db.sqlite_adapter(),
        &creator.id,
        &connected.session.id,
    )
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

    let terminal_record = fetch_creator_live_ingest_session_record(
        state.db.sqlite_adapter(),
        &creator.id,
        &connected.session.id,
    )
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
