use super::*;
use crate::api::control::{
    apply_collaboration_transport_gap, build_live_runtime_advisory,
    collaboration_transport_gap_from_topology, describe_declared_live_runtime_artifact_health,
    fetch_live_ingest_events_for_session, fetch_live_runtime_output_for_session,
    fetch_live_runtime_targets_for_session, fetch_live_runtime_telemetry_for_session,
    fetch_live_runtime_telemetry_summary, fetch_live_runtime_telemetry_summary_for_session,
    fetch_recent_live_runtime_targets, fetch_recent_live_runtime_telemetry,
};
use crate::api::presence::{
    reconcile_stale_creator_live_socket_sessions_for_read,
    reconcile_stale_creator_live_socket_sessions_for_read_coalesced,
};
const CREATOR_LIVE_CONTROL_RESPONSE_CACHE_TTL: Duration = Duration::from_millis(1_000);
const CREATOR_LIVE_RUNTIME_RESPONSE_CACHE_TTL: Duration = Duration::from_millis(2_000);
const CREATOR_LIVE_RECENT_SESSION_LIMIT: i64 = 1;
const CREATOR_LIVE_RECENT_RUNTIME_OUTPUT_LIMIT: i64 = 1;
const CREATOR_LIVE_RECENT_RUNTIME_TARGET_LIMIT: i64 = 1;
const CREATOR_LIVE_RECENT_TELEMETRY_LIMIT: i64 = 4;
const CREATOR_LIVE_RECENT_EVENT_LIMIT: i64 = 4;

fn trim_runtime_collaboration_embed_for_creator_runtime(
    collaboration: &mut CreatorLiveCollaborationSummary,
) {
    let Some(active_control) = collaboration.active_control.as_mut() else {
        return;
    };

    active_control.runtime.recent_events.clear();
    active_control.socket_sessions.clear();

    let topology = &mut active_control.runtime.topology;
    topology.contributions.clear();
    topology.outputs.clear();
    topology.programs.clear();
    topology.audio.clear();
    topology.engine.nodes.clear();
    topology.engine.edges.clear();
    topology.engine.buses.clear();
    topology.engine.operations.clear();
}

pub(crate) async fn fetch_creator_live_control_response(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorLiveControlResponse> {
    reconcile_stale_creator_live_socket_sessions_for_read(pool, Some(creator_id), None).await?;
    let (snapshot, settings, health, subscriber_tiers) = tokio::try_join!(
        build_creator_live_snapshot(pool, creator_id),
        fetch_creator_live_settings(pool, creator_id),
        fetch_creator_live_health(pool, creator_id),
        fetch_creator_subscriber_tiers(pool, creator_id),
    )?;
    let collaboration =
        fetch_creator_live_collaboration_summary(pool, creator_id, &snapshot).await?;
    let viewer_history = health.samples.iter().map(|sample| sample.viewers).collect();
    let bitrate_history = health
        .samples
        .iter()
        .map(|sample| sample.bitrate_kbps)
        .collect();
    let current_viewers = if let Some(session) = snapshot.ingest_session.as_ref() {
        session.viewers
    } else if snapshot.current_broadcast.is_some() {
        fetch_live_stream_by_id(pool, &format!("lv-{}-live", snapshot.profile.handle))
            .await
            .map(|stream| stream.viewers)
            .or_else(|_| {
                health
                    .samples
                    .last()
                    .map(|sample| sample.viewers)
                    .ok_or(AppError::NotFound)
            })
            .unwrap_or(0)
    } else {
        0
    };

    Ok(CreatorLiveControlResponse {
        is_live: snapshot.current_broadcast.is_some(),
        current_viewers,
        snapshot,
        settings,
        health,
        collaboration,
        subscriber_tiers,
        viewer_history,
        bitrate_history,
    })
}

pub(crate) async fn fetch_creator_live_runtime_response(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorLiveRuntimeResponse> {
    reconcile_stale_creator_live_socket_sessions_for_read(pool, Some(creator_id), None).await?;
    let (snapshot, health) = tokio::try_join!(
        build_creator_live_snapshot(pool, creator_id),
        fetch_creator_live_health(pool, creator_id),
    )?;
    let collaboration =
        fetch_creator_live_collaboration_summary(pool, creator_id, &snapshot).await?;
    let active_session = snapshot.ingest_session.clone();
    let (
        active_runtime_output,
        active_runtime_targets,
        telemetry_summary,
        recent_telemetry,
        recent_events,
        recent_sessions,
        recent_runtime_outputs,
        recent_runtime_targets,
    ) = if let Some(session) = active_session.as_ref() {
        let session_id = session.id.as_str();
        let (output, targets, recent_telemetry, recent_events) = tokio::try_join!(
            fetch_live_runtime_output_for_session(pool, session_id),
            fetch_live_runtime_targets_for_session(pool, session_id),
            fetch_live_runtime_telemetry_for_session(
                pool,
                session_id,
                CREATOR_LIVE_RECENT_TELEMETRY_LIMIT,
            ),
            fetch_live_ingest_events_for_session(pool, session_id, CREATOR_LIVE_RECENT_EVENT_LIMIT),
        )?;
        let telemetry_summary =
            fetch_live_runtime_telemetry_summary_for_session(pool, session_id).await?;
        let recent_runtime_targets = targets
            .iter()
            .max_by(|left, right| {
                left.updated_at
                    .cmp(&right.updated_at)
                    .then_with(|| left.target_kind.cmp(&right.target_kind))
                    .then_with(|| left.target_key.cmp(&right.target_key))
            })
            .cloned()
            .into_iter()
            .collect();
        let recent_runtime_outputs = output.clone().into_iter().collect();
        (
            output,
            targets,
            telemetry_summary,
            recent_telemetry,
            recent_events,
            vec![session.clone()],
            recent_runtime_outputs,
            recent_runtime_targets,
        )
    } else {
        let (
            recent_sessions,
            recent_runtime_outputs,
            recent_runtime_targets,
            telemetry_summary,
            recent_telemetry,
            recent_events,
        ) = tokio::try_join!(
            fetch_recent_live_ingest_sessions(pool, creator_id, CREATOR_LIVE_RECENT_SESSION_LIMIT),
            fetch_recent_live_runtime_outputs(
                pool,
                creator_id,
                CREATOR_LIVE_RECENT_RUNTIME_OUTPUT_LIMIT,
            ),
            fetch_recent_live_runtime_targets(
                pool,
                creator_id,
                CREATOR_LIVE_RECENT_RUNTIME_TARGET_LIMIT,
            ),
            fetch_live_runtime_telemetry_summary(pool, creator_id),
            fetch_recent_live_runtime_telemetry(
                pool,
                creator_id,
                CREATOR_LIVE_RECENT_TELEMETRY_LIMIT,
            ),
            fetch_live_ingest_events_for_creator(pool, creator_id, CREATOR_LIVE_RECENT_EVENT_LIMIT),
        )?;
        (
            None,
            Vec::new(),
            telemetry_summary,
            recent_telemetry,
            recent_events,
            recent_sessions,
            recent_runtime_outputs,
            recent_runtime_targets,
        )
    };
    let runtime_advisory = build_live_runtime_advisory(
        active_session.as_ref(),
        active_runtime_output.as_ref(),
        Some(&telemetry_summary),
    );
    let runtime_advisory = if let (Some(session), Some(active_control)) = (
        active_session.as_ref(),
        collaboration.active_control.as_ref(),
    ) {
        apply_collaboration_transport_gap(
            session,
            runtime_advisory,
            collaboration_transport_gap_from_topology(&active_control.runtime.topology),
        )
    } else {
        runtime_advisory
    };
    let artifact_health = match (active_session.as_ref(), active_runtime_output.as_ref()) {
        (Some(session), Some(output)) => Some(describe_declared_live_runtime_artifact_health(
            session, output,
        )),
        _ => None,
    };

    let mut response = CreatorLiveRuntimeResponse {
        snapshot,
        health,
        collaboration,
        active_session,
        active_runtime_output,
        active_runtime_targets,
        telemetry_summary,
        runtime_advisory,
        artifact_health,
        recent_sessions,
        recent_runtime_outputs,
        recent_runtime_targets,
        recent_telemetry,
        recent_events,
    };
    trim_runtime_collaboration_embed_for_creator_runtime(&mut response.collaboration);
    Ok(response)
}

pub(crate) async fn fetch_authoritative_creator_live_control_response(
    state: &SharedState,
    creator_id: &str,
) -> AppResult<CreatorLiveControlResponse> {
    if let Some(cached) = state
        .live_response_cache
        .get_control(creator_id, CREATOR_LIVE_CONTROL_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok(cached);
    }
    let _coalesced = state
        .request_coalescer
        .acquire(&format!("creator-live-control:{creator_id}"))
        .await;
    if let Some(cached) = state
        .live_response_cache
        .get_control(creator_id, CREATOR_LIVE_CONTROL_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok(cached);
    }
    reconcile_stale_creator_live_socket_sessions_for_read_coalesced(state, Some(creator_id), None)
        .await?;
    reconcile_collaboration_expiry_for_host_read(state, creator_id).await?;
    let response =
        fetch_creator_live_control_response(state.db.try_sqlite_adapter()?, creator_id).await?;
    state
        .live_response_cache
        .put_control(creator_id, response.clone())
        .await;
    Ok(response)
}

pub(crate) async fn fetch_authoritative_creator_live_runtime_response(
    state: &SharedState,
    creator_id: &str,
) -> AppResult<CreatorLiveRuntimeResponse> {
    if let Some(cached) = state
        .live_response_cache
        .get_runtime(creator_id, CREATOR_LIVE_RUNTIME_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok(cached);
    }
    let _coalesced = state
        .request_coalescer
        .acquire(&format!("creator-live-runtime:{creator_id}"))
        .await;
    if let Some(cached) = state
        .live_response_cache
        .get_runtime(creator_id, CREATOR_LIVE_RUNTIME_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok(cached);
    }
    reconcile_stale_creator_live_socket_sessions_for_read_coalesced(state, Some(creator_id), None)
        .await?;
    reconcile_collaboration_expiry_for_host_read(state, creator_id).await?;
    let response =
        fetch_creator_live_runtime_response(state.db.try_sqlite_adapter()?, creator_id).await?;
    state
        .live_response_cache
        .put_runtime(creator_id, response.clone())
        .await;
    Ok(response)
}
