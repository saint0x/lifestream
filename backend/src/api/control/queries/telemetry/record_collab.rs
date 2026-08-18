use super::*;
use crate::api::collab::{
    build_collaboration_runtime_response_for_host, fetch_active_collaboration_session_for_broadcast,
};
use crate::api::control::fetch_terminalizable_live_ingest_sessions_for_broadcast;
use crate::api::control::{
    build_collaboration_runtime_bundle, collaboration_transport_gap_from_topology,
};
use crate::api::media::build_collaboration_media_runtime;

#[derive(Clone, Debug)]
pub(super) struct LiveRuntimeTelemetryCollaboration {
    pub session_id: String,
    pub status: String,
    pub chat_mode: String,
    pub recording_policy: String,
    pub participant_count: i64,
    pub live_participant_count: i64,
    pub backstage_participant_count: i64,
    pub mirror_participant_count: i64,
    pub active_grant_count: i64,
    pub issued_grant_count: i64,
    pub active_pickup_count: i64,
    pub mix_minus_required: bool,
    pub transport_gap_present: bool,
    pub audio_mix_mode: &'static str,
    pub active_route_count: i64,
    pub armed_archive_route_count: i64,
    pub shared_program_mirror_route_count: i64,
    pub guest_isolated_mirror_route_count: i64,
    pub engine_node_count: i64,
    pub engine_edge_count: i64,
    pub mix_minus_edge_count: i64,
    pub mirror_fanout_edge_count: i64,
    pub bundle_attachment_count: i64,
    pub bundle_mixer_count: i64,
    pub bundle_fanout_count: i64,
    pub bundle_return_count: i64,
    pub media_stage_count: i64,
    pub media_output_target_count: i64,
    pub media_return_target_count: i64,
    pub media_input_participant_count: i64,
    pub media_mix_minus_participant_count: i64,
}

pub(super) async fn build_live_runtime_telemetry_collaboration(
    pool: &SqlitePool,
    broadcast_id: &str,
) -> AppResult<Option<LiveRuntimeTelemetryCollaboration>> {
    let Some(session) =
        fetch_active_collaboration_session_for_broadcast(pool, broadcast_id).await?
    else {
        return Ok(None);
    };
    let live_session = fetch_terminalizable_live_ingest_sessions_for_broadcast(
        pool,
        &session.host_creator_id,
        &session.source_broadcast_id,
    )
    .await?
    .into_iter()
    .next()
    .ok_or_else(|| {
        AppError::Internal(
            "active collaboration session missing corresponding live ingest session".to_string(),
        )
    })?;
    let runtime = build_collaboration_runtime_response_for_host(pool, session.clone()).await?;
    let topology = runtime.topology;
    let bundle = build_collaboration_runtime_bundle(&live_session, &topology)?;
    let media_runtime = build_collaboration_media_runtime(&bundle)?;

    let participant_count = runtime.session.participants.len() as i64;
    let live_participant_count = runtime
        .session
        .participants
        .iter()
        .filter(|participant| participant.state == "live")
        .count() as i64;
    let backstage_participant_count = runtime
        .session
        .participants
        .iter()
        .filter(|participant| participant.state == "backstage")
        .count() as i64;
    let mirror_participant_count = runtime
        .session
        .participants
        .iter()
        .filter(|participant| participant.role != "host" && participant.mirror_to_guest_channel)
        .count() as i64;
    let mix_minus_required = topology.mix_minus_required;
    let transport_gap_present = collaboration_transport_gap_from_topology(&topology);
    let active_grant_count = count_collaboration_rows(
        pool,
        "collaboration_mirror_grants",
        &runtime.session.id,
        "active",
    )
    .await?;
    let issued_grant_count = count_collaboration_rows(
        pool,
        "collaboration_mirror_grants",
        &runtime.session.id,
        "issued",
    )
    .await?;
    let active_pickup_count = count_collaboration_rows(
        pool,
        "collaboration_mirror_pickups",
        &runtime.session.id,
        "active",
    )
    .await?;
    let shared_program_mirror_route_count = topology
        .outputs
        .iter()
        .filter(|route| {
            route.output_kind == "mirror_channel"
                && route.mix_minus_required
                && route.source_participant_ids.len() > 1
        })
        .count() as i64;
    let guest_isolated_mirror_route_count = topology
        .outputs
        .iter()
        .filter(|route| {
            route.output_kind == "mirror_channel"
                && !(route.mix_minus_required && route.source_participant_ids.len() > 1)
        })
        .count() as i64;
    let active_route_count = topology
        .outputs
        .iter()
        .filter(|route| matches!(route.route_state.as_str(), "active" | "degraded"))
        .count() as i64;
    let armed_archive_route_count = topology
        .outputs
        .iter()
        .filter(|route| route.output_kind == "archive" && route.recording_enabled)
        .count() as i64;
    let engine_node_count = topology.engine.nodes.len() as i64;
    let engine_edge_count = topology.engine.edges.len() as i64;
    let mix_minus_edge_count = topology
        .engine
        .edges
        .iter()
        .filter(|edge| {
            edge.edge_kind == "program_to_audio_return" && !edge.excluded_participant_ids.is_empty()
        })
        .count() as i64;
    let mirror_fanout_edge_count = topology
        .engine
        .edges
        .iter()
        .filter(|edge| edge.edge_kind == "program_to_output")
        .count() as i64;
    let bundle_attachment_count = bundle.attachments.len() as i64;
    let bundle_mixer_count = bundle.mixers.len() as i64;
    let bundle_fanout_count = bundle.fanouts.len() as i64;
    let bundle_return_count = bundle.returns.len() as i64;
    let media_stage_count = media_runtime.stage_count;
    let media_output_target_count = media_runtime.output_targets.len() as i64;
    let media_return_target_count = media_runtime.return_targets.len() as i64;
    let media_input_participant_count = media_runtime.input_participant_ids.len() as i64;
    let media_mix_minus_participant_count = media_runtime.mix_minus_participant_ids.len() as i64;

    Ok(Some(LiveRuntimeTelemetryCollaboration {
        session_id: runtime.session.id,
        status: runtime.session.status,
        chat_mode: runtime.session.chat_mode,
        recording_policy: runtime.session.recording_policy,
        participant_count,
        live_participant_count,
        backstage_participant_count,
        mirror_participant_count,
        active_grant_count,
        issued_grant_count,
        active_pickup_count,
        mix_minus_required,
        transport_gap_present,
        audio_mix_mode: if mix_minus_required {
            "mix_minus"
        } else {
            "program_only"
        },
        active_route_count,
        armed_archive_route_count,
        shared_program_mirror_route_count,
        guest_isolated_mirror_route_count,
        engine_node_count,
        engine_edge_count,
        mix_minus_edge_count,
        mirror_fanout_edge_count,
        bundle_attachment_count,
        bundle_mixer_count,
        bundle_fanout_count,
        bundle_return_count,
        media_stage_count,
        media_output_target_count,
        media_return_target_count,
        media_input_participant_count,
        media_mix_minus_participant_count,
    }))
}

async fn count_collaboration_rows(
    pool: &SqlitePool,
    table: &str,
    session_id: &str,
    state: &str,
) -> AppResult<i64> {
    let query = format!("SELECT COUNT(*) FROM {table} WHERE session_id = ? AND state = ?");
    sqlx::query_scalar(&query)
        .bind(session_id)
        .bind(state)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
}
