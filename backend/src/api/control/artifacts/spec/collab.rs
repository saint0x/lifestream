use super::doc::LiveRuntimeCollaborationSpec;
use super::*;
use crate::api::collab::{
    build_collaboration_runtime_response_for_host, fetch_active_collaboration_session_for_broadcast,
};
use crate::api::mirror::sync_active_collaboration_mirror_pickups_for_session_and_publish;
use crate::models::{
    CollaborationAudioRoute, CollaborationOutputRoute, CollaborationProgramRoute,
    CollaborationRuntimeAttachment, CollaborationRuntimeBundle, CollaborationRuntimeFanout,
    CollaborationRuntimeMixer, CollaborationRuntimeReturn,
};

pub(super) async fn sync_runtime_target_dependents(
    state: &SharedState,
    session: &LiveIngestSession,
) -> AppResult<()> {
    let Some(collaboration_session) =
        fetch_active_collaboration_session_for_broadcast(&state.pool, &session.broadcast_id)
            .await?
    else {
        return Ok(());
    };

    sync_active_collaboration_mirror_pickups_for_session_and_publish(
        state,
        &collaboration_session.id,
    )
    .await
}

pub(super) async fn build_live_runtime_collaboration_spec(
    state: &SharedState,
    session: &LiveIngestSession,
) -> AppResult<Option<LiveRuntimeCollaborationSpec>> {
    let Some(collaboration_session) =
        fetch_active_collaboration_session_for_broadcast(&state.pool, &session.broadcast_id)
            .await?
    else {
        return Ok(None);
    };
    let runtime =
        build_collaboration_runtime_response_for_host(&state.pool, collaboration_session).await?;
    let topology = runtime.topology;
    let bundle = build_collaboration_runtime_bundle(session, &topology)?;

    Ok(Some(LiveRuntimeCollaborationSpec {
        session_id: runtime.session.id,
        status: runtime.session.status,
        source_broadcast_id: runtime.session.source_broadcast_id,
        chat_mode: runtime.session.chat_mode,
        recording_policy: runtime.session.recording_policy,
        shared_chat: topology.shared_chat,
        mix_minus_required: topology.mix_minus_required,
        audio_mix_mode: if topology.mix_minus_required {
            "mix_minus".to_string()
        } else {
            "program_only".to_string()
        },
        connected_participants: topology.connected_participants,
        recording_owner_creator_id: topology.recording_owner_creator_id,
        host_output_participant_ids: topology.host_output_participant_ids,
        mirrored_creator_ids: topology.mirrored_creator_ids,
        contributions: topology.contributions,
        outputs: topology.outputs,
        programs: topology.programs,
        audio: topology.audio,
        engine: topology.engine,
        bundle,
        members: topology.members,
    }))
}

pub(in crate::api::control::artifacts) fn build_collaboration_runtime_bundle(
    session: &LiveIngestSession,
    topology: &crate::models::CollaborationRuntimeTopology,
) -> AppResult<CollaborationRuntimeBundle> {
    let attachments = topology
        .contributions
        .iter()
        .flat_map(|contribution| {
            topology
                .programs
                .iter()
                .filter(move |program| {
                    program
                        .source_participant_ids
                        .contains(&contribution.participant_id)
                })
                .map(move |program| CollaborationRuntimeAttachment {
                    participant_id: contribution.participant_id.clone(),
                    transport_class: contribution.transport_class.clone(),
                    contribution_bus_id: format!("bus-contrib-{}", contribution.participant_id),
                    program_bus_id: format!("bus-program-{}", program.id),
                    route_state: program.route_state.clone(),
                    mix_minus_required: contribution.mix_minus_required || program.mix_minus_required,
                })
        })
        .collect::<Vec<_>>();

    let mixers = topology
        .programs
        .iter()
        .map(|program| CollaborationRuntimeMixer {
            program_id: program.id.clone(),
            program_kind: program.program_kind.clone(),
            input_bus_ids: program
                .source_participant_ids
                .iter()
                .map(|participant_id| format!("bus-contrib-{participant_id}"))
                .collect(),
            output_bus_id: format!("bus-program-{}", program.id),
            route_state: program.route_state.clone(),
            mix_minus_required: program.mix_minus_required,
        })
        .collect::<Vec<_>>();

    let fanouts = topology
        .programs
        .iter()
        .flat_map(|program| {
            topology
                .outputs
                .iter()
                .filter(move |output| program.output_ids.contains(&output.id))
                .map(move |output| CollaborationRuntimeFanout {
                    output_id: output.id.clone(),
                    output_kind: output.output_kind.clone(),
                    input_bus_id: format!("bus-program-{}", program.id),
                    output_bus_id: format!("bus-output-{}", output.id),
                    relative_path: collaboration_route_relative_path(session, output),
                    target_creator_id: output.target_creator_id.clone(),
                    target_broadcast_id: output.target_broadcast_id.clone(),
                    route_state: output.route_state.clone(),
                    playback_enabled: output.playback_enabled,
                    recording_enabled: output.recording_enabled,
                    mix_minus_required: output.mix_minus_required,
                })
        })
        .collect::<Vec<_>>();

    let returns = topology
        .audio
        .iter()
        .filter(|audio| audio.receive_program_audio)
        .filter_map(|audio| {
            topology
                .programs
                .iter()
                .find(|program| program.program_kind == "host_program")
                .map(|program| CollaborationRuntimeReturn {
                    participant_id: audio.participant_id.clone(),
                    input_bus_id: format!("bus-program-{}", program.id),
                    output_bus_id: format!("bus-audio-{}", audio.participant_id),
                    excluded_participant_ids: audio.excluded_participant_ids.clone(),
                    attached_output_ids: audio.attached_output_ids.clone(),
                    route_state: audio.route_state.clone(),
                    mix_minus_required: audio.mix_minus_required,
                })
        })
        .collect::<Vec<_>>();

    validate_runtime_bundle(topology, &attachments, &mixers, &fanouts, &returns)?;

    Ok(CollaborationRuntimeBundle {
        bundle_mode: "media_runtime_v1".to_string(),
        engine_execution_mode: topology.engine.execution_mode.clone(),
        fanout_mode: topology.engine.fanout_mode.clone(),
        audio_mode: topology.engine.audio_mode.clone(),
        attachments,
        mixers,
        fanouts,
        returns,
    })
}

fn validate_runtime_bundle(
    topology: &crate::models::CollaborationRuntimeTopology,
    attachments: &[CollaborationRuntimeAttachment],
    mixers: &[CollaborationRuntimeMixer],
    fanouts: &[CollaborationRuntimeFanout],
    returns: &[CollaborationRuntimeReturn],
) -> AppResult<()> {
    if !topology.engine.operations.is_empty() && attachments.is_empty() && !mixers.is_empty() {
        return Err(AppError::Internal(
            "compiled collaboration runtime bundle missing contribution attachments".to_string(),
        ));
    }
    if topology
        .programs
        .iter()
        .any(|program| !mixers.iter().any(|mixer| mixer.program_id == program.id))
    {
        return Err(AppError::Internal(
            "compiled collaboration runtime bundle missing program mixer".to_string(),
        ));
    }
    if topology
        .outputs
        .iter()
        .any(|output| !fanouts.iter().any(|fanout| fanout.output_id == output.id))
    {
        return Err(AppError::Internal(
            "compiled collaboration runtime bundle missing output fanout".to_string(),
        ));
    }
    if topology.mix_minus_required
        && topology
            .audio
            .iter()
            .any(|audio| audio.receive_program_audio)
        && returns.is_empty()
    {
        return Err(AppError::Internal(
            "compiled collaboration runtime bundle missing audio returns".to_string(),
        ));
    }
    Ok(())
}

pub(in crate::api::control::artifacts) fn collaboration_route_relative_path(
    session: &LiveIngestSession,
    route: &CollaborationOutputRoute,
) -> Option<String> {
    match route.output_kind.as_str() {
        "host_channel" => Some(canonical_live_runtime_manifest_relative_path(session)),
        "mirror_channel" => route
            .target_creator_id
            .as_ref()
            .zip(route.target_broadcast_id.as_ref())
            .map(|(creator_id, broadcast_id)| {
                format!("live/{creator_id}/{broadcast_id}/{}/master.m3u8", route.id)
            }),
        "archive" => route
            .target_creator_id
            .as_ref()
            .zip(route.target_broadcast_id.as_ref())
            .map(|(creator_id, broadcast_id)| {
                format!("archive/{creator_id}/{broadcast_id}/{}/final.mp4", route.id)
            }),
        _ => route
            .target_creator_id
            .as_ref()
            .zip(route.target_broadcast_id.as_ref())
            .map(|(creator_id, broadcast_id)| {
                format!("runtime/{creator_id}/{broadcast_id}/{}/target", route.id)
            }),
    }
}

pub(in crate::api::control::artifacts) fn collaboration_program_relative_path(
    session: &LiveIngestSession,
    program: &CollaborationProgramRoute,
) -> String {
    format!(
        "runtime/{}/{}/{}/collaboration/programs/{}.json",
        session.creator_id, session.broadcast_id, session.id, program.id
    )
}

pub(in crate::api::control::artifacts) fn collaboration_audio_relative_path(
    session: &LiveIngestSession,
    route: &CollaborationAudioRoute,
) -> String {
    format!(
        "runtime/{}/{}/{}/collaboration/audio/{}.json",
        session.creator_id, session.broadcast_id, session.id, route.participant_id
    )
}

pub(in crate::api::control::artifacts) fn collaboration_engine_relative_path(
    session: &LiveIngestSession,
) -> String {
    format!(
        "runtime/{}/{}/{}/collaboration/engine.json",
        session.creator_id, session.broadcast_id, session.id
    )
}

pub(in crate::api::control::artifacts) fn collaboration_bundle_relative_path(
    session: &LiveIngestSession,
) -> String {
    format!(
        "runtime/{}/{}/{}/collaboration/runtime.json",
        session.creator_id, session.broadcast_id, session.id
    )
}
