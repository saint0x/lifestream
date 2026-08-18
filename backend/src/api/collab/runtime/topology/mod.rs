use super::presence::{
    fetch_collaboration_socket_presence_for_session,
    fetch_visible_collaboration_mirror_grants_for_session_view,
    fetch_visible_collaboration_mirror_pickups_for_session_view,
};
use super::*;
use crate::models::CollaborationRuntimeTopology;

mod audio;
mod engine;
mod outputs;
mod participants;
mod programs;
mod resolve;
mod responses;

use audio::build_topology_audio;
use engine::build_topology_engine;
use outputs::build_topology_outputs;
use participants::build_topology_participants;
use programs::build_topology_programs;
use resolve::resolve_host_source_ingest_session;
pub(crate) use responses::{
    build_collaboration_runtime_response_for_host,
    build_collaboration_runtime_response_for_participant,
    build_creator_collaboration_control_response_for_host,
    fetch_creator_live_collaboration_summary,
};

pub(crate) async fn build_collaboration_runtime_topology(
    pool: &SqlitePool,
    session: &CollaborationSessionView,
    grants: &[CollaborationMirrorGrant],
    pickups: &[CollaborationMirrorPickup],
    connected_participants: i64,
) -> AppResult<CollaborationRuntimeTopology> {
    let shared_chat = session.chat_mode == "shared";
    let recording_owner_creator_id = match session.recording_policy.as_str() {
        "host_archive" => Some(session.host_creator_id.clone()),
        _ => None,
    };
    let socket_sessions =
        fetch_collaboration_socket_presence_for_session(pool, &session.id).await?;
    let host_source_session = resolve_host_source_ingest_session(pool, session).await?;
    let participant_state = build_topology_participants(
        session,
        grants,
        pickups,
        &socket_sessions,
        host_source_session.as_ref(),
    );

    let outputs = build_topology_outputs(
        session,
        grants,
        pickups,
        &host_source_session,
        &participant_state.host_output_participant_ids,
        participant_state.mix_minus_required,
    );
    let programs = build_topology_programs(
        session,
        &outputs,
        &participant_state.host_output_participant_ids,
        &participant_state.live_participant_ids,
    );
    let audio = build_topology_audio(
        session,
        &participant_state.host_output_participant_ids,
        &participant_state.live_participant_ids,
        &participant_state.backstage_participant_ids,
        &participant_state.contributions,
    );
    let engine = build_topology_engine(
        &programs,
        &audio,
        &outputs,
        &participant_state.contributions,
        &participant_state.mirrored_creator_ids,
        participant_state.mix_minus_required,
    );

    Ok(CollaborationRuntimeTopology {
        session_id: session.id.clone(),
        source_broadcast_id: session.source_broadcast_id.clone(),
        chat_mode: session.chat_mode.clone(),
        recording_policy: session.recording_policy.clone(),
        shared_chat,
        mix_minus_required: participant_state.mix_minus_required,
        recording_owner_creator_id,
        connected_participants,
        host_output_participant_ids: participant_state.host_output_participant_ids,
        backstage_participant_ids: participant_state.backstage_participant_ids,
        live_participant_ids: participant_state.live_participant_ids,
        mirrored_creator_ids: participant_state.mirrored_creator_ids,
        contributions: participant_state.contributions,
        outputs,
        programs,
        audio,
        engine,
        members: participant_state.members,
    })
}

fn push_unique(ids: &mut Vec<String>, value: String) {
    if !ids.contains(&value) {
        ids.push(value);
    }
}
