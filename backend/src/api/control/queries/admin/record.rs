use super::*;

pub(crate) async fn fetch_admin_live_ingest_sessions(
    pool: &SqlitePool,
    creator_filter: Option<&str>,
    status_filter: Option<&str>,
    limit: i64,
) -> AppResult<Vec<AdminLiveIngestSessionRecord>> {
    reconcile_stale_live_ingest_sessions_for_read(pool, creator_filter, None).await?;
    let limit = limit.clamp(1, 250);
    let rows = match (creator_filter, status_filter) {
        (Some(creator_id), Some(status)) => {
            sqlx::query(
                "SELECT id FROM live_ingest_sessions WHERE creator_id = ? AND status = ? ORDER BY connected_at DESC LIMIT ?",
            )
            .bind(creator_id)
            .bind(status)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        (Some(creator_id), None) => {
            sqlx::query(
                "SELECT id FROM live_ingest_sessions WHERE creator_id = ? ORDER BY connected_at DESC LIMIT ?",
            )
            .bind(creator_id)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        (None, Some(status)) => {
            sqlx::query(
                "SELECT id FROM live_ingest_sessions WHERE status = ? ORDER BY connected_at DESC LIMIT ?",
            )
            .bind(status)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        (None, None) => {
            sqlx::query("SELECT id FROM live_ingest_sessions ORDER BY connected_at DESC LIMIT ?")
                .bind(limit)
                .fetch_all(pool)
                .await?
        }
    };

    let mut records = Vec::with_capacity(rows.len());
    for row in rows {
        let session_id: String = row.get("id");
        records.push(fetch_admin_live_ingest_session_record(pool, &session_id).await?);
    }
    Ok(records)
}

pub(crate) async fn fetch_admin_live_ingest_session_record(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<AdminLiveIngestSessionRecord> {
    let session = fetch_live_ingest_session_by_id_global(pool, session_id).await?;
    let runtime_output = fetch_live_runtime_output_for_session(pool, session_id).await?;
    let runtime_targets = fetch_live_runtime_targets_for_session(pool, session_id).await?;
    let telemetry_summary =
        fetch_live_runtime_telemetry_summary_for_session(pool, session_id).await?;
    let recent_events = fetch_live_ingest_events_for_session(pool, session_id, 20).await?;
    let runtime_advisory = build_live_runtime_advisory(
        Some(&session),
        runtime_output.as_ref(),
        Some(&telemetry_summary),
    );
    let runtime_advisory = if let Some(collaboration_session) =
        fetch_active_collaboration_session_for_broadcast(pool, &session.broadcast_id).await?
    {
        let runtime =
            build_collaboration_runtime_response_for_host(pool, collaboration_session).await?;
        apply_collaboration_transport_gap(
            &session,
            runtime_advisory,
            collaboration_transport_gap_from_topology(&runtime.topology),
        )
    } else {
        runtime_advisory
    };
    Ok(AdminLiveIngestSessionRecord {
        stale_connection: is_live_ingest_session_stale(&session),
        runtime_advisory,
        artifact_health: runtime_output
            .as_ref()
            .map(|output| describe_declared_live_runtime_artifact_health(&session, output)),
        session,
        runtime_output,
        runtime_targets,
        telemetry_summary,
        recent_telemetry: fetch_live_runtime_telemetry_for_session(pool, session_id, 20).await?,
        recent_events,
    })
}

pub(crate) async fn fetch_creator_live_ingest_session_record(
    pool: &SqlitePool,
    creator_id: &str,
    session_id: &str,
) -> AppResult<AdminLiveIngestSessionRecord> {
    let session = fetch_live_ingest_session_by_id(pool, creator_id, session_id).await?;
    let runtime_output = fetch_live_runtime_output_for_session(pool, session_id).await?;
    let runtime_targets = fetch_live_runtime_targets_for_session(pool, session_id).await?;
    let telemetry_summary =
        fetch_live_runtime_telemetry_summary_for_session(pool, session_id).await?;
    let recent_events = fetch_live_ingest_events_for_session(pool, session_id, 20).await?;
    let runtime_advisory = build_live_runtime_advisory(
        Some(&session),
        runtime_output.as_ref(),
        Some(&telemetry_summary),
    );
    let runtime_advisory = if let Some(collaboration_session) =
        fetch_active_collaboration_session_for_broadcast(pool, &session.broadcast_id).await?
    {
        let runtime =
            build_collaboration_runtime_response_for_host(pool, collaboration_session).await?;
        apply_collaboration_transport_gap(
            &session,
            runtime_advisory,
            collaboration_transport_gap_from_topology(&runtime.topology),
        )
    } else {
        runtime_advisory
    };
    Ok(AdminLiveIngestSessionRecord {
        stale_connection: is_live_ingest_session_stale(&session),
        runtime_advisory,
        artifact_health: runtime_output
            .as_ref()
            .map(|output| describe_declared_live_runtime_artifact_health(&session, output)),
        session,
        runtime_output,
        runtime_targets,
        telemetry_summary,
        recent_telemetry: fetch_live_runtime_telemetry_for_session(pool, session_id, 20).await?,
        recent_events,
    })
}
