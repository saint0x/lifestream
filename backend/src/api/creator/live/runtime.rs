use super::*;
use crate::api::ingestctl::{
    build_live_runtime_advisory, describe_declared_live_runtime_artifact_health,
    describe_live_runtime_artifact_health,
    fetch_current_live_runtime_targets,
    fetch_live_runtime_telemetry_summary,
    fetch_recent_live_runtime_targets, fetch_recent_live_runtime_telemetry,
    reconcile_live_runtime_output_artifacts,
};

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
    let snapshot = build_creator_live_snapshot(pool, creator_id).await?;
    let health = fetch_creator_live_health(pool, creator_id).await?;
    let collaboration =
        fetch_creator_live_collaboration_summary(pool, creator_id, &snapshot).await?;
    let active_session = fetch_active_live_ingest_session(pool, creator_id).await?;
    let active_runtime_output = fetch_current_live_runtime_output(pool, creator_id).await?;
    let active_runtime_targets = fetch_current_live_runtime_targets(pool, creator_id).await?;
    let telemetry_summary = fetch_live_runtime_telemetry_summary(pool, creator_id).await?;
    let runtime_advisory = build_live_runtime_advisory(
        active_session.as_ref(),
        active_runtime_output.as_ref(),
        Some(&telemetry_summary),
    );
    let artifact_health = match (active_session.as_ref(), active_runtime_output.as_ref()) {
        (Some(session), Some(output)) => {
            Some(describe_declared_live_runtime_artifact_health(session, output))
        }
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
        recent_sessions: fetch_recent_live_ingest_sessions(pool, creator_id, 10).await?,
        recent_runtime_outputs: fetch_recent_live_runtime_outputs(pool, creator_id, 10).await?,
        recent_runtime_targets: fetch_recent_live_runtime_targets(pool, creator_id, 25).await?,
        recent_telemetry: fetch_recent_live_runtime_telemetry(pool, creator_id, 25).await?,
        recent_events: fetch_live_ingest_events_for_creator(pool, creator_id, 25).await?,
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
    if let Some(session) = fetch_active_live_ingest_session(&state.pool, creator_id).await? {
        let _ = reconcile_live_runtime_output_artifacts(state, &session).await?;
    }
    let mut response = fetch_creator_live_runtime_response(&state.pool, creator_id).await?;
    if let (Some(session), Some(output)) = (
        response.active_session.as_ref(),
        response.active_runtime_output.as_ref(),
    ) {
        response.artifact_health =
            Some(describe_live_runtime_artifact_health(state, session, output).await?);
    }
    Ok(response)
}
