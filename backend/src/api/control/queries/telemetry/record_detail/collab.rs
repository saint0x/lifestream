use super::super::record_collab::LiveRuntimeTelemetryCollaboration;
use super::*;

pub(super) fn build_collaboration_detail(
    collaboration: Option<&LiveRuntimeTelemetryCollaboration>,
) -> Value {
    collaboration
        .map(|item| {
            json!({
                "present": true,
                "sessionId": item.session_id.clone(),
                "status": item.status.clone(),
                "chatMode": item.chat_mode.clone(),
                "recordingPolicy": item.recording_policy.clone(),
                "participantCount": item.participant_count,
                "liveParticipantCount": item.live_participant_count,
                "backstageParticipantCount": item.backstage_participant_count,
                "mirrorParticipantCount": item.mirror_participant_count,
                "activeGrantCount": item.active_grant_count,
                "issuedGrantCount": item.issued_grant_count,
                "activePickupCount": item.active_pickup_count,
                "mixMinusRequired": item.mix_minus_required,
                "transportGapPresent": item.transport_gap_present,
                "audioMixMode": item.audio_mix_mode,
                "sharedProgramMirrorRouteCount": item.shared_program_mirror_route_count,
                "guestIsolatedMirrorRouteCount": item.guest_isolated_mirror_route_count,
                "engineNodeCount": item.engine_node_count,
                "engineEdgeCount": item.engine_edge_count,
                "mixMinusEdgeCount": item.mix_minus_edge_count,
                "mirrorFanoutEdgeCount": item.mirror_fanout_edge_count,
                "bundleAttachmentCount": item.bundle_attachment_count,
                "bundleMixerCount": item.bundle_mixer_count,
                "bundleFanoutCount": item.bundle_fanout_count,
                "bundleReturnCount": item.bundle_return_count,
                "mediaStageCount": item.media_stage_count,
                "mediaOutputTargetCount": item.media_output_target_count,
                "mediaReturnTargetCount": item.media_return_target_count,
                "mediaInputParticipantCount": item.media_input_participant_count,
                "mediaMixMinusParticipantCount": item.media_mix_minus_participant_count,
            })
        })
        .unwrap_or_else(|| json!({ "present": false }))
}
