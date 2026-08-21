use super::*;
use crate::api::control::{
    apply_collaboration_transport_gap, build_live_runtime_advisory,
    collaboration_transport_gap_from_topology, describe_declared_live_runtime_artifact_health,
    describe_live_runtime_artifact_health, fetch_live_runtime_output_for_session,
    fetch_live_runtime_targets_for_session, fetch_live_runtime_telemetry_for_session,
    fetch_live_runtime_telemetry_summary, fetch_live_runtime_telemetry_summary_for_session,
    fetch_recent_live_runtime_targets, fetch_recent_live_runtime_telemetry,
    reconcile_live_runtime_output_artifacts,
};

const CREATOR_LIVE_RECENT_SESSION_LIMIT: i64 = 3;
const CREATOR_LIVE_RECENT_RUNTIME_OUTPUT_LIMIT: i64 = 3;
const CREATOR_LIVE_RECENT_RUNTIME_TARGET_LIMIT: i64 = 6;
const CREATOR_LIVE_RECENT_TELEMETRY_LIMIT: i64 = 6;
const CREATOR_LIVE_RECENT_EVENT_LIMIT: i64 = 6;

pub(crate) async fn fetch_creator_live_control_response(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorLiveControlResponse> {
    reconcile_stale_creator_live_socket_sessions_for_read(pool, Some(creator_id), None).await?;
    let snapshot = build_creator_live_snapshot(pool, creator_id).await?;
    let settings = fetch_creator_live_settings(pool, creator_id).await?;
    let health = fetch_creator_live_health(pool, creator_id).await?;
    let collaboration =
        fetch_creator_live_collaboration_summary(pool, creator_id, &snapshot).await?;
    let subscriber_tiers = fetch_creator_subscriber_tiers(pool, creator_id).await?;
    let viewer_history = health.samples.iter().map(|sample| sample.viewers).collect();
    let bitrate_history = health
        .samples
        .iter()
        .map(|sample| sample.bitrate_kbps)
        .collect();
    let current_viewers = if let Some(session) = snapshot.ingest_session.as_ref() {
        session.viewers
    } else if snapshot.current_broadcast.is_some() {
        if let Some(viewers) = health.samples.last().map(|sample| sample.viewers) {
            viewers
        } else {
            fetch_live_stream_by_id(pool, &format!("lv-{}-live", snapshot.profile.handle))
                .await
                .map(|stream| stream.viewers)
                .unwrap_or(0)
        }
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
    let (snapshot, health, recent_sessions, recent_runtime_outputs, recent_runtime_targets, recent_events) =
        tokio::try_join!(
            build_creator_live_snapshot(pool, creator_id),
            fetch_creator_live_health(pool, creator_id),
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
            fetch_live_ingest_events_for_creator(pool, creator_id, CREATOR_LIVE_RECENT_EVENT_LIMIT),
        )?;
    let collaboration =
        fetch_creator_live_collaboration_summary(pool, creator_id, &snapshot).await?;
    let active_session = snapshot.ingest_session.clone();
    let (active_runtime_output, active_runtime_targets, telemetry_summary, recent_telemetry) =
        if let Some(session) = active_session.as_ref() {
            let session_id = session.id.as_str();
            let (output, targets, telemetry_summary, recent_telemetry) = tokio::try_join!(
                fetch_live_runtime_output_for_session(pool, session_id),
                fetch_live_runtime_targets_for_session(pool, session_id),
                fetch_live_runtime_telemetry_summary_for_session(pool, session_id),
                fetch_live_runtime_telemetry_for_session(
                    pool,
                    session_id,
                    CREATOR_LIVE_RECENT_TELEMETRY_LIMIT,
                ),
            )?;
            (output, targets, telemetry_summary, recent_telemetry)
        } else {
            let telemetry_summary = fetch_live_runtime_telemetry_summary(pool, creator_id).await?;
            let recent_telemetry =
                fetch_recent_live_runtime_telemetry(pool, creator_id, CREATOR_LIVE_RECENT_TELEMETRY_LIMIT)
                    .await?;
            (None, Vec::new(), telemetry_summary, recent_telemetry)
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

    Ok(CreatorLiveRuntimeResponse {
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
    })
}

pub(crate) async fn fetch_authoritative_creator_live_control_response(
    state: &SharedState,
    creator_id: &str,
) -> AppResult<CreatorLiveControlResponse> {
    reconcile_collaboration_expiry_for_host_read(state, creator_id).await?;
    fetch_creator_live_control_response(&state.pool, creator_id).await
}

pub(crate) async fn fetch_authoritative_creator_live_runtime_response(
    state: &SharedState,
    creator_id: &str,
) -> AppResult<CreatorLiveRuntimeResponse> {
    reconcile_collaboration_expiry_for_host_read(state, creator_id).await?;
    let mut response = fetch_creator_live_runtime_response(&state.pool, creator_id).await?;
    if let Some(session) = response.active_session.clone() {
        if let Some(reconciled_output) =
            reconcile_live_runtime_output_artifacts(state, &session).await?
        {
            response.active_runtime_output = Some(reconciled_output.clone());
            response.artifact_health = Some(
                describe_live_runtime_artifact_health(state, &session, &reconciled_output).await?,
            );
        }
    }
    Ok(response)
}
