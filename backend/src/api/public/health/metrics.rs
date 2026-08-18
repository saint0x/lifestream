use super::*;

pub(crate) async fn metrics(State(state): State<SharedState>) -> AppResult<Response> {
    let mut body = String::new();
    let status_counts = state.metrics.status_counts().await;
    let active_streams = state.realtime.active_streams().await;
    let active_collaboration_sessions = state.realtime.active_collaboration_sessions().await;
    reconcile_stale_creator_live_socket_sessions_for_read(&state.pool, None, None).await?;
    let active_viewer_presence = count_all_active_live_viewer_sessions(&state.pool).await?;
    let active_collaboration_presence =
        count_all_active_collaboration_socket_sessions(&state.pool).await?;
    let active_creator_live_presence =
        count_all_active_creator_live_socket_sessions(&state.pool).await?;
    let live_ingest_overview = fetch_admin_live_ingest_overview(&state.pool, None).await?;
    let worker = state.background_worker.snapshot().await;
    let _ = writeln!(body, "# TYPE lifestream_http_requests_total counter");
    let _ = writeln!(
        body,
        "lifestream_http_requests_total {}",
        state.metrics.total_requests()
    );
    let _ = writeln!(body, "# TYPE lifestream_http_requests_in_flight gauge");
    let _ = writeln!(
        body,
        "lifestream_http_requests_in_flight {}",
        state.metrics.in_flight_requests()
    );
    let _ = writeln!(body, "# TYPE lifestream_http_rate_limited_total counter");
    let _ = writeln!(
        body,
        "lifestream_http_rate_limited_total {}",
        state.metrics.total_rate_limits()
    );
    let _ = writeln!(body, "# TYPE lifestream_http_responses_total counter");
    for (status, count) in status_counts {
        let _ = writeln!(
            body,
            "lifestream_http_responses_total{{status=\"{status}\"}} {count}"
        );
    }
    let _ = writeln!(body, "# TYPE lifestream_uptime_seconds gauge");
    let _ = writeln!(body, "lifestream_uptime_seconds {}", state.uptime_seconds());
    let _ = writeln!(body, "# TYPE lifestream_background_worker_ready gauge");
    let _ = writeln!(
        body,
        "lifestream_background_worker_ready {}",
        if worker
            .last_success_age_seconds
            .is_some_and(|age| age <= BACKGROUND_WORKER_STALE_AFTER_SECONDS)
        {
            1
        } else {
            0
        }
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_background_worker_last_tick_age_seconds gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_background_worker_last_tick_age_seconds {}",
        worker.last_tick_age_seconds.unwrap_or(u64::MAX)
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_background_worker_last_success_age_seconds gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_background_worker_last_success_age_seconds {}",
        worker.last_success_age_seconds.unwrap_or(u64::MAX)
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_background_worker_consecutive_failures gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_background_worker_consecutive_failures {}",
        worker.consecutive_failures
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_realtime_collaboration_sessions gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_realtime_collaboration_sessions {}",
        active_collaboration_sessions
    );
    let _ = writeln!(body, "# TYPE lifestream_presence_live_viewers gauge");
    let _ = writeln!(
        body,
        "lifestream_presence_live_viewers {}",
        active_viewer_presence
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_presence_collaboration_participants gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_presence_collaboration_participants {}",
        active_collaboration_presence
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_presence_creator_live_sockets gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_presence_creator_live_sockets {}",
        active_creator_live_presence
    );
    let _ = writeln!(body, "# TYPE lifestream_db_pool_connections gauge");
    let _ = writeln!(body, "lifestream_db_pool_connections {}", state.pool.size());
    let _ = writeln!(body, "# TYPE lifestream_db_pool_idle_connections gauge");
    let _ = writeln!(
        body,
        "lifestream_db_pool_idle_connections {}",
        state.pool.num_idle()
    );
    let _ = writeln!(body, "# TYPE lifestream_ws_connections gauge");
    let _ = writeln!(
        body,
        "lifestream_ws_connections {}",
        state.realtime.total_connections()
    );
    let _ = writeln!(body, "# TYPE lifestream_ws_active_streams gauge");
    let _ = writeln!(body, "lifestream_ws_active_streams {active_streams}");
    let _ = writeln!(body, "# TYPE lifestream_live_ingest_active_sessions gauge");
    let _ = writeln!(
        body,
        "lifestream_live_ingest_active_sessions {}",
        live_ingest_overview.active_sessions
    );
    let _ = writeln!(body, "# TYPE lifestream_live_ingest_stale_sessions gauge");
    let _ = writeln!(
        body,
        "lifestream_live_ingest_stale_sessions {}",
        live_ingest_overview.stale_sessions
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_live_ingest_terminal_sessions gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_live_ingest_terminal_sessions {}",
        live_ingest_overview.terminal_sessions
    );
    let _ = writeln!(body, "# TYPE lifestream_live_ingest_ready_outputs gauge");
    let _ = writeln!(
        body,
        "lifestream_live_ingest_ready_outputs {}",
        live_ingest_overview.ready_outputs
    );
    let _ = writeln!(body, "# TYPE lifestream_live_ingest_degraded_outputs gauge");
    let _ = writeln!(
        body,
        "lifestream_live_ingest_degraded_outputs {}",
        live_ingest_overview.degraded_outputs
    );
    let _ = writeln!(body, "# TYPE lifestream_live_ingest_failed_outputs gauge");
    let _ = writeln!(
        body,
        "lifestream_live_ingest_failed_outputs {}",
        live_ingest_overview.failed_outputs
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_live_ingest_archive_finalizing_outputs gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_live_ingest_archive_finalizing_outputs {}",
        live_ingest_overview.archive_finalizing_outputs
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_live_ingest_archive_complete_outputs gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_live_ingest_archive_complete_outputs {}",
        live_ingest_overview.archive_complete_outputs
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_live_ingest_artifact_attention_outputs gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_live_ingest_artifact_attention_outputs {}",
        live_ingest_overview.artifact_attention_outputs
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_live_ingest_manifest_path_missing_outputs gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_live_ingest_manifest_path_missing_outputs {}",
        live_ingest_overview.manifest_path_missing_outputs
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_live_ingest_archive_path_missing_outputs gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_live_ingest_archive_path_missing_outputs {}",
        live_ingest_overview.archive_path_missing_outputs
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_live_ingest_telemetry_samples gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_live_ingest_telemetry_samples {}",
        live_ingest_overview.total_samples
    );
    let _ = writeln!(body, "# TYPE lifestream_live_ingest_degraded_samples gauge");
    let _ = writeln!(
        body,
        "lifestream_live_ingest_degraded_samples {}",
        live_ingest_overview.degraded_samples
    );
    let _ = writeln!(body, "# TYPE lifestream_live_ingest_failure_samples gauge");
    let _ = writeln!(
        body,
        "lifestream_live_ingest_failure_samples {}",
        live_ingest_overview.failure_samples
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_live_ingest_advisory_critical_samples gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_live_ingest_advisory_critical_samples {}",
        live_ingest_overview.advisory_critical_samples
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_live_ingest_advisory_repairable_samples gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_live_ingest_advisory_repairable_samples {}",
        live_ingest_overview.advisory_repairable_samples
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_live_ingest_runtime_artifact_reconciliation_samples gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_live_ingest_runtime_artifact_reconciliation_samples {}",
        live_ingest_overview.runtime_artifact_reconciliation_samples
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_live_ingest_runtime_archive_completion_samples gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_live_ingest_runtime_archive_completion_samples {}",
        live_ingest_overview.runtime_archive_completion_samples
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_live_ingest_peak_host_channel_targets gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_live_ingest_peak_host_channel_targets {}",
        live_ingest_overview.peak_host_channel_count
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_live_ingest_peak_mirror_channel_targets gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_live_ingest_peak_mirror_channel_targets {}",
        live_ingest_overview.peak_mirror_channel_count
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_live_ingest_peak_shared_program_mirror_channel_targets gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_live_ingest_peak_shared_program_mirror_channel_targets {}",
        live_ingest_overview.peak_shared_program_mirror_channel_count
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_live_ingest_peak_guest_isolated_mirror_channel_targets gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_live_ingest_peak_guest_isolated_mirror_channel_targets {}",
        live_ingest_overview.peak_guest_isolated_mirror_channel_count
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_live_ingest_peak_archive_targets gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_live_ingest_peak_archive_targets {}",
        live_ingest_overview.peak_archive_target_count
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_live_ingest_peak_active_targets gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_live_ingest_peak_active_targets {}",
        live_ingest_overview.peak_active_target_count
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_live_ingest_peak_degraded_targets gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_live_ingest_peak_degraded_targets {}",
        live_ingest_overview.peak_degraded_target_count
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_live_ingest_peak_armed_targets gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_live_ingest_peak_armed_targets {}",
        live_ingest_overview.peak_armed_target_count
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_live_ingest_peak_pending_source_targets gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_live_ingest_peak_pending_source_targets {}",
        live_ingest_overview.peak_pending_source_target_count
    );
    if let Some(value) = live_ingest_overview.last_host_channel_count {
        let _ = writeln!(
            body,
            "# TYPE lifestream_live_ingest_last_host_channel_targets gauge"
        );
        let _ = writeln!(
            body,
            "lifestream_live_ingest_last_host_channel_targets {}",
            value
        );
    }
    if let Some(value) = live_ingest_overview.last_mirror_channel_count {
        let _ = writeln!(
            body,
            "# TYPE lifestream_live_ingest_last_mirror_channel_targets gauge"
        );
        let _ = writeln!(
            body,
            "lifestream_live_ingest_last_mirror_channel_targets {}",
            value
        );
    }
    if let Some(value) = live_ingest_overview.last_shared_program_mirror_channel_count {
        let _ = writeln!(
            body,
            "# TYPE lifestream_live_ingest_last_shared_program_mirror_channel_targets gauge"
        );
        let _ = writeln!(
            body,
            "lifestream_live_ingest_last_shared_program_mirror_channel_targets {}",
            value
        );
    }
    if let Some(value) = live_ingest_overview.last_guest_isolated_mirror_channel_count {
        let _ = writeln!(
            body,
            "# TYPE lifestream_live_ingest_last_guest_isolated_mirror_channel_targets gauge"
        );
        let _ = writeln!(
            body,
            "lifestream_live_ingest_last_guest_isolated_mirror_channel_targets {}",
            value
        );
    }
    if let Some(value) = live_ingest_overview.last_archive_target_count {
        let _ = writeln!(
            body,
            "# TYPE lifestream_live_ingest_last_archive_targets gauge"
        );
        let _ = writeln!(
            body,
            "lifestream_live_ingest_last_archive_targets {}",
            value
        );
    }
    if let Some(value) = live_ingest_overview.last_active_target_count {
        let _ = writeln!(
            body,
            "# TYPE lifestream_live_ingest_last_active_targets gauge"
        );
        let _ = writeln!(body, "lifestream_live_ingest_last_active_targets {}", value);
    }
    if let Some(value) = live_ingest_overview.last_degraded_target_count {
        let _ = writeln!(
            body,
            "# TYPE lifestream_live_ingest_last_degraded_targets gauge"
        );
        let _ = writeln!(
            body,
            "lifestream_live_ingest_last_degraded_targets {}",
            value
        );
    }
    if let Some(value) = live_ingest_overview.last_armed_target_count {
        let _ = writeln!(
            body,
            "# TYPE lifestream_live_ingest_last_armed_targets gauge"
        );
        let _ = writeln!(body, "lifestream_live_ingest_last_armed_targets {}", value);
    }
    if let Some(value) = live_ingest_overview.last_pending_source_target_count {
        let _ = writeln!(
            body,
            "# TYPE lifestream_live_ingest_last_pending_source_targets gauge"
        );
        let _ = writeln!(
            body,
            "lifestream_live_ingest_last_pending_source_targets {}",
            value
        );
    }
    if let Some(value) = live_ingest_overview.avg_ready_latency_seconds {
        let _ = writeln!(
            body,
            "# TYPE lifestream_live_ingest_ready_latency_seconds gauge"
        );
        let _ = writeln!(
            body,
            "lifestream_live_ingest_ready_latency_seconds {}",
            value
        );
    }
    if let Some(value) = live_ingest_overview.avg_archive_completion_seconds {
        let _ = writeln!(
            body,
            "# TYPE lifestream_live_ingest_archive_completion_seconds gauge"
        );
        let _ = writeln!(
            body,
            "lifestream_live_ingest_archive_completion_seconds {}",
            value
        );
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )
        .body(Body::from(body))
        .map_err(|_| AppError::BadRequest("failed to build metrics response".to_string()))
}
