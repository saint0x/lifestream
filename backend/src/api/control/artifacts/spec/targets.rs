use super::*;

pub(super) fn build_live_runtime_targets(
    session: &LiveIngestSession,
    spec: &LiveRuntimeSpecDocument,
    output: &LiveRuntimeOutput,
) -> Vec<LiveRuntimeTarget> {
    let now = Utc::now().to_rfc3339();
    let mut targets = build_variant_targets(session, spec, output, &now);

    if spec.collaboration.is_none() {
        targets.push(build_primary_archive_target(session, spec, output, &now));
    }

    if let Some(collaboration) = spec.collaboration.as_ref() {
        targets.extend(build_collaboration_output_targets(
            session,
            collaboration,
            &now,
        ));
        targets.extend(build_collaboration_program_targets(
            session,
            collaboration,
            &now,
        ));
        targets.extend(build_collaboration_audio_targets(
            session,
            collaboration,
            &now,
        ));
        targets.extend(build_collaboration_return_targets(
            session,
            collaboration,
            &now,
        ));
        targets.push(build_collaboration_engine_target(
            session,
            collaboration,
            &now,
        ));
        targets.push(build_collaboration_bundle_target(
            session,
            collaboration,
            &now,
        ));
        targets.push(build_collaboration_media_target(
            session,
            collaboration,
            &now,
        ));
        targets.push(build_collaboration_launch_target(
            session,
            collaboration,
            &now,
        ));
    }

    targets
}

fn build_variant_targets(
    session: &LiveIngestSession,
    spec: &LiveRuntimeSpecDocument,
    output: &LiveRuntimeOutput,
    now: &str,
) -> Vec<LiveRuntimeTarget> {
    spec.packaging
        .variants
        .iter()
        .map(|variant| LiveRuntimeTarget {
            id: format!("lrt-variant-{}-{}", session.id, variant.label),
            session_id: session.id.clone(),
            creator_id: session.creator_id.clone(),
            broadcast_id: session.broadcast_id.clone(),
            target_kind: "variant".to_string(),
            target_key: variant.label.clone(),
            target_label: variant.label.clone(),
            route_state: output.packaging_status.clone(),
            target_creator_id: Some(session.creator_id.clone()),
            target_broadcast_id: Some(session.broadcast_id.clone()),
            playback_enabled: matches!(output.packaging_status.as_str(), "ready" | "complete"),
            recording_enabled: false,
            mix_minus_required: false,
            relative_path: Some(variant.relative_playlist_path.clone()),
            source_participant_ids: Vec::new(),
            created_at: now.to_string(),
            updated_at: now.to_string(),
        })
        .collect()
}

fn build_primary_archive_target(
    session: &LiveIngestSession,
    spec: &LiveRuntimeSpecDocument,
    output: &LiveRuntimeOutput,
    now: &str,
) -> LiveRuntimeTarget {
    LiveRuntimeTarget {
        id: format!("lrt-archive-{}", session.id),
        session_id: session.id.clone(),
        creator_id: session.creator_id.clone(),
        broadcast_id: session.broadcast_id.clone(),
        target_kind: "archive".to_string(),
        target_key: "primary".to_string(),
        target_label: "primary archive".to_string(),
        route_state: output.archive_status.clone(),
        target_creator_id: Some(session.creator_id.clone()),
        target_broadcast_id: Some(session.broadcast_id.clone()),
        playback_enabled: false,
        recording_enabled: spec.archive.enabled,
        mix_minus_required: false,
        relative_path: Some(spec.archive.output_relative_path.clone()),
        source_participant_ids: Vec::new(),
        created_at: now.to_string(),
        updated_at: now.to_string(),
    }
}

fn build_collaboration_output_targets(
    session: &LiveIngestSession,
    collaboration: &LiveRuntimeCollaborationSpec,
    now: &str,
) -> Vec<LiveRuntimeTarget> {
    collaboration
        .outputs
        .iter()
        .map(|route| LiveRuntimeTarget {
            id: format!("lrt-route-{}", route.id),
            session_id: session.id.clone(),
            creator_id: session.creator_id.clone(),
            broadcast_id: session.broadcast_id.clone(),
            target_kind: route.output_kind.clone(),
            target_key: route.id.clone(),
            target_label: route.output_kind.replace('_', " "),
            route_state: route.route_state.clone(),
            target_creator_id: route.target_creator_id.clone(),
            target_broadcast_id: route.target_broadcast_id.clone(),
            playback_enabled: route.playback_enabled,
            recording_enabled: route.recording_enabled,
            mix_minus_required: route.mix_minus_required,
            relative_path: collaboration_route_relative_path(session, route),
            source_participant_ids: route.source_participant_ids.clone(),
            created_at: now.to_string(),
            updated_at: now.to_string(),
        })
        .collect()
}

fn build_collaboration_program_targets(
    session: &LiveIngestSession,
    collaboration: &LiveRuntimeCollaborationSpec,
    now: &str,
) -> Vec<LiveRuntimeTarget> {
    collaboration
        .programs
        .iter()
        .map(|program| LiveRuntimeTarget {
            id: format!("lrt-program-{}", program.id),
            session_id: session.id.clone(),
            creator_id: session.creator_id.clone(),
            broadcast_id: session.broadcast_id.clone(),
            target_kind: "program".to_string(),
            target_key: program.id.clone(),
            target_label: program.program_kind.replace('_', " "),
            route_state: program.route_state.clone(),
            target_creator_id: program.target_creator_id.clone(),
            target_broadcast_id: program.target_broadcast_id.clone(),
            playback_enabled: program.playback_enabled,
            recording_enabled: program.recording_enabled,
            mix_minus_required: program.mix_minus_required,
            relative_path: Some(collaboration_program_relative_path(session, program)),
            source_participant_ids: program.source_participant_ids.clone(),
            created_at: now.to_string(),
            updated_at: now.to_string(),
        })
        .collect()
}

fn build_collaboration_audio_targets(
    session: &LiveIngestSession,
    collaboration: &LiveRuntimeCollaborationSpec,
    now: &str,
) -> Vec<LiveRuntimeTarget> {
    collaboration
        .audio
        .iter()
        .map(|audio| LiveRuntimeTarget {
            id: format!("lrt-audio-{}-{}", session.id, audio.participant_id),
            session_id: session.id.clone(),
            creator_id: session.creator_id.clone(),
            broadcast_id: session.broadcast_id.clone(),
            target_kind: "audio".to_string(),
            target_key: audio.participant_id.clone(),
            target_label: format!("audio {}", audio.route_kind.replace('_', " ")),
            route_state: audio.route_state.clone(),
            target_creator_id: audio.creator_id.clone(),
            target_broadcast_id: Some(session.broadcast_id.clone()),
            playback_enabled: audio.receive_program_audio,
            recording_enabled: false,
            mix_minus_required: audio.mix_minus_required,
            relative_path: Some(collaboration_audio_relative_path(session, audio)),
            source_participant_ids: audio.upstream_participant_ids.clone(),
            created_at: now.to_string(),
            updated_at: now.to_string(),
        })
        .collect()
}

fn build_collaboration_return_targets(
    session: &LiveIngestSession,
    collaboration: &LiveRuntimeCollaborationSpec,
    now: &str,
) -> Vec<LiveRuntimeTarget> {
    collaboration
        .media
        .return_targets
        .iter()
        .map(|route| {
            let audio = collaboration
                .audio
                .iter()
                .find(|audio| audio.participant_id == route.participant_id);
            LiveRuntimeTarget {
                id: format!("lrt-return-{}-{}", session.id, route.participant_id),
                session_id: session.id.clone(),
                creator_id: session.creator_id.clone(),
                broadcast_id: session.broadcast_id.clone(),
                target_kind: "return".to_string(),
                target_key: route.participant_id.clone(),
                target_label: "audio return".to_string(),
                route_state: route.route_state.clone(),
                target_creator_id: audio.and_then(|item| item.creator_id.clone()),
                target_broadcast_id: Some(session.broadcast_id.clone()),
                playback_enabled: false,
                recording_enabled: false,
                mix_minus_required: route.mix_minus_required,
                relative_path: Some(collaboration_return_relative_path(session, route)),
                source_participant_ids: audio
                    .map(|item| item.upstream_participant_ids.clone())
                    .unwrap_or_default(),
                created_at: now.to_string(),
                updated_at: now.to_string(),
            }
        })
        .collect()
}

fn build_collaboration_engine_target(
    session: &LiveIngestSession,
    collaboration: &LiveRuntimeCollaborationSpec,
    now: &str,
) -> LiveRuntimeTarget {
    LiveRuntimeTarget {
        id: format!("lrt-engine-{}", session.id),
        session_id: session.id.clone(),
        creator_id: session.creator_id.clone(),
        broadcast_id: session.broadcast_id.clone(),
        target_kind: "engine".to_string(),
        target_key: collaboration.engine.execution_mode.clone(),
        target_label: "collaboration engine".to_string(),
        route_state: collaboration_engine_state(collaboration),
        target_creator_id: Some(session.creator_id.clone()),
        target_broadcast_id: Some(session.broadcast_id.clone()),
        playback_enabled: false,
        recording_enabled: false,
        mix_minus_required: collaboration.mix_minus_required,
        relative_path: Some(collaboration_engine_relative_path(session)),
        source_participant_ids: collaboration
            .contributions
            .iter()
            .map(|item| item.participant_id.clone())
            .collect(),
        created_at: now.to_string(),
        updated_at: now.to_string(),
    }
}

fn build_collaboration_bundle_target(
    session: &LiveIngestSession,
    collaboration: &LiveRuntimeCollaborationSpec,
    now: &str,
) -> LiveRuntimeTarget {
    LiveRuntimeTarget {
        id: format!("lrt-bundle-{}", session.id),
        session_id: session.id.clone(),
        creator_id: session.creator_id.clone(),
        broadcast_id: session.broadcast_id.clone(),
        target_kind: "bundle".to_string(),
        target_key: collaboration.bundle.bundle_mode.clone(),
        target_label: "collaboration bundle".to_string(),
        route_state: bundle_state(collaboration),
        target_creator_id: Some(session.creator_id.clone()),
        target_broadcast_id: Some(session.broadcast_id.clone()),
        playback_enabled: false,
        recording_enabled: false,
        mix_minus_required: collaboration.mix_minus_required,
        relative_path: Some(collaboration_bundle_relative_path(session)),
        source_participant_ids: collaboration
            .contributions
            .iter()
            .map(|item| item.participant_id.clone())
            .collect(),
        created_at: now.to_string(),
        updated_at: now.to_string(),
    }
}

fn build_collaboration_media_target(
    session: &LiveIngestSession,
    collaboration: &LiveRuntimeCollaborationSpec,
    now: &str,
) -> LiveRuntimeTarget {
    LiveRuntimeTarget {
        id: format!("lrt-media-{}", session.id),
        session_id: session.id.clone(),
        creator_id: session.creator_id.clone(),
        broadcast_id: session.broadcast_id.clone(),
        target_kind: "media".to_string(),
        target_key: collaboration.media.runtime_mode.clone(),
        target_label: "collaboration media runtime".to_string(),
        route_state: media_runtime_state(collaboration),
        target_creator_id: Some(session.creator_id.clone()),
        target_broadcast_id: Some(session.broadcast_id.clone()),
        playback_enabled: false,
        recording_enabled: false,
        mix_minus_required: collaboration.mix_minus_required,
        relative_path: Some(collaboration_media_relative_path(session)),
        source_participant_ids: collaboration.media.input_participant_ids.clone(),
        created_at: now.to_string(),
        updated_at: now.to_string(),
    }
}

fn build_collaboration_launch_target(
    session: &LiveIngestSession,
    collaboration: &LiveRuntimeCollaborationSpec,
    now: &str,
) -> LiveRuntimeTarget {
    LiveRuntimeTarget {
        id: format!("lrt-launch-{}", session.id),
        session_id: session.id.clone(),
        creator_id: session.creator_id.clone(),
        broadcast_id: session.broadcast_id.clone(),
        target_kind: "launch".to_string(),
        target_key: collaboration.launch.launch_mode.clone(),
        target_label: "collaboration launch plan".to_string(),
        route_state: launch_runtime_state(collaboration),
        target_creator_id: Some(session.creator_id.clone()),
        target_broadcast_id: Some(session.broadcast_id.clone()),
        playback_enabled: false,
        recording_enabled: false,
        mix_minus_required: collaboration.mix_minus_required,
        relative_path: Some(collaboration_launch_relative_path(session)),
        source_participant_ids: collaboration
            .launch
            .inputs
            .iter()
            .map(|item| item.participant_id.clone())
            .collect(),
        created_at: now.to_string(),
        updated_at: now.to_string(),
    }
}

fn collaboration_engine_state(collaboration: &LiveRuntimeCollaborationSpec) -> String {
    if collaboration.engine.edges.is_empty() {
        "inactive".to_string()
    } else if collaboration
        .engine
        .nodes
        .iter()
        .any(|node| node.route_state == "degraded")
    {
        "degraded".to_string()
    } else if collaboration
        .engine
        .nodes
        .iter()
        .any(|node| matches!(node.route_state.as_str(), "active" | "live" | "attached"))
    {
        "active".to_string()
    } else {
        "armed".to_string()
    }
}

fn bundle_state(collaboration: &LiveRuntimeCollaborationSpec) -> String {
    route_state_from_iter(
        collaboration
            .bundle
            .fanouts
            .iter()
            .map(|item| item.route_state.as_str())
            .chain(
                collaboration
                    .bundle
                    .returns
                    .iter()
                    .map(|item| item.route_state.as_str()),
            ),
    )
}

fn media_runtime_state(collaboration: &LiveRuntimeCollaborationSpec) -> String {
    route_state_from_iter(
        collaboration
            .media
            .output_targets
            .iter()
            .map(|item| item.route_state.as_str())
            .chain(
                collaboration
                    .media
                    .return_targets
                    .iter()
                    .map(|item| item.route_state.as_str()),
            ),
    )
}

fn launch_runtime_state(collaboration: &LiveRuntimeCollaborationSpec) -> String {
    if collaboration.launch.ready && !collaboration.launch.steps.is_empty() {
        "active".to_string()
    } else if !collaboration.launch.unresolved_participant_ids.is_empty() {
        "pending_source".to_string()
    } else if !collaboration.launch.inputs.is_empty() {
        "armed".to_string()
    } else {
        "inactive".to_string()
    }
}

fn route_state_from_iter<'a>(states: impl Iterator<Item = &'a str>) -> String {
    let states = states.collect::<Vec<_>>();
    if states.is_empty() {
        return "inactive".to_string();
    }
    if states
        .iter()
        .any(|state| matches!(*state, "active" | "live" | "attached"))
    {
        return "active".to_string();
    }
    if states.iter().any(|state| *state == "degraded") {
        return "degraded".to_string();
    }
    if states.iter().any(|state| *state == "armed") {
        return "armed".to_string();
    }
    if states.iter().any(|state| *state == "pending_source") {
        return "pending_source".to_string();
    }
    if states.iter().any(|state| *state == "issued") {
        return "issued".to_string();
    }
    if states.iter().any(|state| *state == "standby") {
        return "standby".to_string();
    }
    states.into_iter().next().unwrap_or("inactive").to_string()
}
