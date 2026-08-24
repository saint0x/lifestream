use super::*;

#[tokio::test]
async fn admin_live_ingest_overview_aggregates_latency_and_creator_breakdown() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let baseline = get_admin_live_ingest_overview(
        State(state.clone()),
        auth_headers(&token),
        Query(AdminLiveIngestOverviewQuery {
            creator_id: Some(creator.id.clone()),
        }),
    )
    .await?
    .0;
    let broadcast = insert_ready_broadcast(state.db.sqlite_adapter(), &creator).await?;
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
    .execute(state.db.sqlite_adapter())
    .await?;
    sqlx::query(
        "UPDATE live_runtime_outputs SET runtime_state = 'healthy', packaging_status = 'ready', archive_status = 'finalizing', updated_at = ?, last_runtime_event_at = ? WHERE session_id = ?",
    )
    .bind(&ready_at)
    .bind(&ready_at)
    .bind(&connected.session.id)
    .execute(state.db.sqlite_adapter())
    .await?;
    sqlx::query(
        "INSERT INTO live_runtime_telemetry (id, session_id, creator_id, broadcast_id, sample_kind, runtime_state, packaging_status, archive_status, bitrate_kbps, viewers, dropped_frames, cpu_percent, free_disk_gb, detail_json, collected_at) VALUES (?, ?, ?, ?, 'runtime_report', 'healthy', 'ready', 'not_started', 6400, 120, 0, 31, 500.0, '{}', ?)",
    )
    .bind(format!("lrt-test-{}", Uuid::new_v4().simple()))
    .bind(&connected.session.id)
    .bind(&creator.id)
    .bind(&broadcast.id)
    .bind(&ready_at)
    .execute(state.db.sqlite_adapter())
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
    assert!(overview.manifest_path_missing_outputs >= baseline.manifest_path_missing_outputs + 1);
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
