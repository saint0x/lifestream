use super::doc::LiveRuntimeCollaborationSpec;
use super::*;
use crate::api::collab::{
    build_collaboration_runtime_response_for_host, fetch_active_collaboration_session_for_broadcast,
};
use crate::api::mirror::sync_active_collaboration_mirror_pickups_for_session_and_publish;
use crate::models::CollaborationOutputRoute;

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
        members: topology.members,
    }))
}

pub(in crate::api::ingestctl::artifacts) fn collaboration_route_relative_path(
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
