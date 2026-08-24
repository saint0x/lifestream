use super::*;

#[tokio::test]
async fn creator_runtime_repair_updates_output_and_records_audit_trail() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let broadcast = insert_ready_broadcast(state.db.sqlite_adapter(), &creator).await?;
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
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let broadcast = insert_ready_broadcast(state.db.sqlite_adapter(), &creator).await?;
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
        .execute(state.db.sqlite_adapter())
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
