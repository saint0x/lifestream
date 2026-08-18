use super::*;
use crate::models::LiveRuntimeTarget;

pub(super) fn build_target_detail(targets: &[LiveRuntimeTarget]) -> Value {
    json!({
        "count": targets.len(),
        "playbackEnabledCount": targets.iter().filter(|target| target.playback_enabled).count(),
        "recordingEnabledCount": targets.iter().filter(|target| target.recording_enabled).count(),
        "variantCount": targets.iter().filter(|target| target.target_kind == "variant").count(),
        "programCount": targets.iter().filter(|target| target.target_kind == "program").count(),
        "audioCount": targets.iter().filter(|target| target.target_kind == "audio").count(),
        "returnCount": targets.iter().filter(|target| target.target_kind == "return").count(),
        "engineCount": targets.iter().filter(|target| target.target_kind == "engine").count(),
        "bundleCount": targets.iter().filter(|target| target.target_kind == "bundle").count(),
        "mediaCount": targets.iter().filter(|target| target.target_kind == "media").count(),
        "launchCount": targets.iter().filter(|target| target.target_kind == "launch").count(),
        "hostChannelCount": targets.iter().filter(|target| target.target_kind == "host_channel").count(),
        "mirrorChannelCount": targets.iter().filter(|target| target.target_kind == "mirror_channel").count(),
        "sharedProgramMirrorChannelCount": targets
            .iter()
            .filter(|target| {
                target.target_kind == "mirror_channel"
                    && target.mix_minus_required
                    && target.source_participant_ids.len() > 1
            })
            .count(),
        "guestIsolatedMirrorChannelCount": targets
            .iter()
            .filter(|target| {
                target.target_kind == "mirror_channel"
                    && !(target.mix_minus_required && target.source_participant_ids.len() > 1)
            })
            .count(),
        "archiveCount": targets.iter().filter(|target| target.target_kind == "archive").count(),
        "collaborationCount": targets
            .iter()
            .filter(|target| matches!(target.target_kind.as_str(), "host_channel" | "mirror_channel" | "archive" | "program" | "audio" | "return" | "engine" | "bundle" | "media" | "launch"))
            .count(),
        "activeCount": targets.iter().filter(|target| target.route_state == "active").count(),
        "degradedCount": targets.iter().filter(|target| target.route_state == "degraded").count(),
        "armedCount": targets.iter().filter(|target| target.route_state == "armed").count(),
        "pendingSourceCount": targets.iter().filter(|target| target.route_state == "pending_source").count(),
        "kinds": targets.iter().map(|target| target.target_kind.clone()).collect::<Vec<_>>(),
        "states": targets.iter().map(|target| target.route_state.clone()).collect::<Vec<_>>(),
        "routes": targets
            .iter()
            .filter(|target| matches!(target.target_kind.as_str(), "host_channel" | "mirror_channel" | "archive" | "program" | "audio" | "return" | "engine" | "bundle" | "media" | "launch"))
            .map(|target| {
                json!({
                    "kind": target.target_kind,
                    "key": target.target_key,
                    "state": target.route_state,
                    "playbackEnabled": target.playback_enabled,
                    "recordingEnabled": target.recording_enabled,
                    "mixMinusRequired": target.mix_minus_required,
                    "targetCreatorId": target.target_creator_id,
                    "targetBroadcastId": target.target_broadcast_id,
                    "relativePath": target.relative_path,
                    "sourceParticipantIds": target.source_participant_ids,
                })
            })
            .collect::<Vec<_>>(),
    })
}
