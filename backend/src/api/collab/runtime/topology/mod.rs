use super::presence::{
    fetch_collaboration_socket_presence_for_session,
    fetch_visible_collaboration_mirror_grants_for_session_view,
    fetch_visible_collaboration_mirror_pickups_for_session_view,
};
use super::*;
use crate::api::collab::fetch_collaboration_invites_for_session;
use crate::models::{
    CollaborationAudioRoute, CollaborationContributionAttachment, CollaborationExecutionEdge,
    CollaborationExecutionNode, CollaborationExecutionPlan, CollaborationOutputRoute,
    CollaborationProgramRoute,
};

mod outputs;
mod resolve;

use outputs::{build_topology_outputs, mirror_output_is_authorized};
use resolve::resolve_host_source_ingest_session;

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
    let mut host_output_participant_ids = Vec::new();
    let mut backstage_participant_ids = Vec::new();
    let mut live_participant_ids = Vec::new();
    let mut mirrored_creator_ids = Vec::new();
    let mut contributions = Vec::with_capacity(session.participants.len());
    let mut members = Vec::with_capacity(session.participants.len());
    let mut mix_minus_required = false;

    for participant in &session.participants {
        let active_grant = grants
            .iter()
            .find(|grant| grant.participant_id == participant.id && grant.state == "active");
        let issued_grant = grants
            .iter()
            .find(|grant| grant.participant_id == participant.id && grant.state == "issued");
        let active_pickup = pickups
            .iter()
            .find(|pickup| pickup.participant_id == participant.id && pickup.state == "active");

        if participant.role == "host" || participant.publish_to_host {
            host_output_participant_ids.push(participant.id.clone());
        }
        if participant.state == "backstage" {
            backstage_participant_ids.push(participant.id.clone());
        }
        if participant.state == "live" {
            live_participant_ids.push(participant.id.clone());
        }
        let has_socket = socket_sessions.iter().any(|socket| {
            socket.participant_id.as_deref() == Some(participant.id.as_str())
                && socket.disconnected_at.is_none()
                && !socket.is_stale
        });

        let host_output_state = if participant.role == "host" {
            "host".to_string()
        } else if !participant.publish_to_host {
            "disabled".to_string()
        } else {
            match participant.state.as_str() {
                "live" => "live".to_string(),
                "backstage" => "backstage".to_string(),
                _ => "inactive".to_string(),
            }
        };
        let mirror_output_id = participant
            .creator_id
            .as_ref()
            .map(|_| format!("col-out-mirror-{}", participant.id));
        let mirror_archive_output_id = participant
            .creator_id
            .as_ref()
            .map(|_| format!("col-out-archive-{}", participant.id));

        let mirror_pickup_state = if participant.role == "host" {
            "host".to_string()
        } else if let Some(pickup) = active_pickup {
            if let Some(creator_id) = participant.creator_id.clone() {
                if !mirrored_creator_ids.contains(&creator_id) {
                    mirrored_creator_ids.push(creator_id);
                }
            }
            pickup.state.clone()
        } else if participant.creator_id.is_none() {
            "unavailable".to_string()
        } else if !participant.mirror_to_guest_channel {
            "disabled".to_string()
        } else if let Some(grant) = active_grant {
            if let Some(creator_id) = participant.creator_id.clone() {
                if !mirrored_creator_ids.contains(&creator_id) {
                    mirrored_creator_ids.push(creator_id);
                }
            }
            grant.state.clone()
        } else if let Some(grant) = issued_grant {
            if let Some(creator_id) = participant.creator_id.clone() {
                if !mirrored_creator_ids.contains(&creator_id) {
                    mirrored_creator_ids.push(creator_id);
                }
            }
            grant.state.clone()
        } else {
            match participant.state.as_str() {
                "live" | "backstage" => "eligible".to_string(),
                _ => "inactive".to_string(),
            }
        };
        let participant_mix_minus_required = participant.role != "host"
            && participant.publish_to_host
            && participant.state == "live";
        mix_minus_required |= participant_mix_minus_required;
        let authorized_mirror_output = participant.mirror_to_guest_channel
            && mirror_output_is_authorized(active_grant, issued_grant, active_pickup);

        let mut attached_output_ids = Vec::new();
        if participant.role == "host" || participant.publish_to_host {
            attached_output_ids.push(format!("col-out-host-{}", session.id));
            if session.recording_policy == "host_archive"
                || session.recording_policy == "split_archive"
            {
                attached_output_ids.push(format!("col-out-archive-host-{}", session.id));
            }
        }
        if let Some(mirror_output_id) = mirror_output_id.clone() {
            if authorized_mirror_output {
                attached_output_ids.push(mirror_output_id);
            }
        }
        if session.recording_policy == "split_archive" {
            if let Some(mirror_archive_output_id) = mirror_archive_output_id.clone() {
                if authorized_mirror_output {
                    attached_output_ids.push(mirror_archive_output_id);
                }
            }
        }

        let transport_class = if participant.role == "host" {
            host_source_session
                .as_ref()
                .map(|source| source.contribution_class.clone())
                .unwrap_or_else(|| "missing_source".to_string())
        } else {
            "collaboration_socket".to_string()
        };
        let contribution_state = if participant.role == "host" {
            host_source_session
                .as_ref()
                .map(|source| source.contribution_state.clone())
                .unwrap_or_else(|| "missing_source".to_string())
        } else {
            match participant.state.as_str() {
                "live" if has_socket => "attached".to_string(),
                "live" => "awaiting_socket".to_string(),
                "backstage" if has_socket => "standby".to_string(),
                "backstage" => "awaiting_socket".to_string(),
                _ => "inactive".to_string(),
            }
        };

        contributions.push(CollaborationContributionAttachment {
            participant_id: participant.id.clone(),
            user_id: participant.user_id.clone(),
            creator_id: participant.creator_id.clone(),
            transport_class,
            source_broadcast_id: if participant.role == "host" {
                Some(session.source_broadcast_id.clone())
            } else {
                active_pickup
                    .map(|pickup| pickup.source_broadcast_id.clone())
                    .or_else(|| Some(session.source_broadcast_id.clone()))
            },
            ingest_session_id: if participant.role == "host" {
                host_source_session.as_ref().map(|source| source.id.clone())
            } else {
                None
            },
            contribution_state,
            attached_output_ids,
            mix_minus_required: participant_mix_minus_required,
        });

        members.push(CollaborationTopologyMember {
            participant_id: participant.id.clone(),
            user_id: participant.user_id.clone(),
            creator_id: participant.creator_id.clone(),
            role: participant.role.clone(),
            state: participant.state.clone(),
            publish_to_host: participant.publish_to_host,
            mirror_to_guest_channel: participant.mirror_to_guest_channel,
            can_speak_in_chat: participant.can_speak_in_chat,
            host_output_state,
            mirror_pickup_state,
            mirror_pickup_broadcast_id: active_pickup
                .map(|pickup| pickup.guest_broadcast_id.clone()),
            mirror_pickup_activated_at: active_pickup.map(|pickup| pickup.activated_at.clone()),
        });
    }

    let outputs = build_topology_outputs(
        session,
        grants,
        pickups,
        &host_source_session,
        &host_output_participant_ids,
        mix_minus_required,
    );
    let programs = build_topology_programs(
        session,
        &outputs,
        &host_output_participant_ids,
        &live_participant_ids,
    );
    let audio = build_topology_audio(
        session,
        &host_output_participant_ids,
        &live_participant_ids,
        &backstage_participant_ids,
        &contributions,
    );
    let engine = build_topology_engine(
        &programs,
        &audio,
        &outputs,
        &contributions,
        &mirrored_creator_ids,
        mix_minus_required,
    );

    Ok(CollaborationRuntimeTopology {
        session_id: session.id.clone(),
        source_broadcast_id: session.source_broadcast_id.clone(),
        chat_mode: session.chat_mode.clone(),
        recording_policy: session.recording_policy.clone(),
        shared_chat,
        mix_minus_required,
        recording_owner_creator_id,
        connected_participants,
        host_output_participant_ids,
        backstage_participant_ids,
        live_participant_ids,
        mirrored_creator_ids,
        contributions,
        outputs,
        programs,
        audio,
        engine,
        members,
    })
}

fn build_topology_programs(
    session: &CollaborationSessionView,
    outputs: &[CollaborationOutputRoute],
    host_output_participant_ids: &[String],
    live_participant_ids: &[String],
) -> Vec<CollaborationProgramRoute> {
    let mut programs = Vec::new();
    let host_participant_ids = host_output_participant_ids.to_vec();
    let host_output_ids = session
        .participants
        .iter()
        .filter(|participant| participant.role == "host" || participant.publish_to_host)
        .flat_map(|participant| {
            planned_output_ids_for_participant(session, participant, outputs, host_output_participant_ids)
        })
        .fold(Vec::new(), |mut ids, output_id| {
            push_unique(&mut ids, output_id);
            ids
        });
    let host_route_state = program_route_state(outputs, &host_output_ids);
    programs.push(CollaborationProgramRoute {
        id: format!("col-program-host-{}", session.id),
        program_kind: "host_program".to_string(),
        route_state: host_route_state,
        source_participant_ids: host_participant_ids.clone(),
        output_ids: host_output_ids,
        target_creator_id: Some(session.host_creator_id.clone()),
        target_broadcast_id: Some(session.source_broadcast_id.clone()),
        playback_enabled: outputs.iter().any(|output| {
            output.output_kind == "host_channel"
                && matches!(output.route_state.as_str(), "active" | "degraded" | "issued")
        }),
        recording_enabled: outputs.iter().any(|output| {
            output.recording_enabled
                && host_participant_ids
                    .iter()
                    .all(|participant_id| output.source_participant_ids.contains(participant_id))
        }),
        mix_minus_required: session.participants.iter().any(|participant| {
            participant.role != "host" && participant.publish_to_host && participant.state == "live"
        }),
    });

    for participant in &session.participants {
        if participant.role == "host" || participant.publish_to_host {
            continue;
        }
        let output_ids =
            planned_output_ids_for_participant(session, participant, outputs, host_output_participant_ids);
        if output_ids.is_empty() {
            continue;
        }
        let source_participant_ids = if live_participant_ids.contains(&participant.id) {
            vec![participant.id.clone()]
        } else {
            Vec::new()
        };
        programs.push(CollaborationProgramRoute {
            id: format!("col-program-{}", participant.id),
            program_kind: "guest_program".to_string(),
            route_state: program_route_state(outputs, &output_ids),
            source_participant_ids,
            output_ids,
            target_creator_id: participant.creator_id.clone(),
            target_broadcast_id: outputs
                .iter()
                .find(|output| output.id == format!("col-out-mirror-{}", participant.id))
                .and_then(|output| output.target_broadcast_id.clone()),
            playback_enabled: outputs.iter().any(|output| {
                output.id == format!("col-out-mirror-{}", participant.id)
                    && matches!(output.route_state.as_str(), "active" | "degraded" | "issued")
            }),
            recording_enabled: outputs.iter().any(|output| {
                output.id == format!("col-out-archive-{}", participant.id) && output.recording_enabled
            }),
            mix_minus_required: false,
        });
    }

    programs
}

fn build_topology_audio(
    session: &CollaborationSessionView,
    host_output_participant_ids: &[String],
    live_participant_ids: &[String],
    backstage_participant_ids: &[String],
    contributions: &[CollaborationContributionAttachment],
) -> Vec<CollaborationAudioRoute> {
    session
        .participants
        .iter()
        .map(|participant| {
            let contribution = contributions
                .iter()
                .find(|item| item.participant_id == participant.id)
                .expect("collaboration contribution should exist for participant");
            let route_kind = if participant.role == "host" {
                "program_origin"
            } else if participant.publish_to_host {
                "mix_minus_return"
            } else if !contribution.attached_output_ids.is_empty() {
                "isolated_program"
            } else {
                "inactive"
            };
            let route_state = if live_participant_ids.contains(&participant.id) {
                "live"
            } else if backstage_participant_ids.contains(&participant.id) {
                "standby"
            } else {
                "inactive"
            };
            let receive_program_audio = participant.role != "host" && participant.publish_to_host;
            let mix_minus_required =
                participant.role != "host" && participant.publish_to_host && participant.state == "live";
            let upstream_participant_ids = if participant.role == "host" || !participant.publish_to_host {
                if contribution.attached_output_ids.is_empty() {
                    Vec::new()
                } else {
                    vec![participant.id.clone()]
                }
            } else {
                host_output_participant_ids.to_vec()
            };
            let excluded_participant_ids = if mix_minus_required {
                vec![participant.id.clone()]
            } else {
                Vec::new()
            };

            CollaborationAudioRoute {
                participant_id: participant.id.clone(),
                user_id: participant.user_id.clone(),
                creator_id: participant.creator_id.clone(),
                route_kind: route_kind.to_string(),
                route_state: route_state.to_string(),
                receive_program_audio,
                mix_minus_required,
                upstream_participant_ids,
                excluded_participant_ids,
                attached_output_ids: contribution.attached_output_ids.clone(),
            }
        })
        .collect()
}

fn planned_output_ids_for_participant(
    session: &CollaborationSessionView,
    participant: &CollaborationParticipant,
    outputs: &[CollaborationOutputRoute],
    host_output_participant_ids: &[String],
) -> Vec<String> {
    outputs
        .iter()
        .filter(|output| {
            if participant.role == "host" || participant.publish_to_host {
                output.id == format!("col-out-host-{}", session.id)
                    || output.id == format!("col-out-archive-host-{}", session.id)
                    || output.source_participant_ids == host_output_participant_ids
            } else {
                output.id == format!("col-out-mirror-{}", participant.id)
                    || output.id == format!("col-out-archive-{}", participant.id)
            }
        })
        .map(|output| output.id.clone())
        .collect()
}

fn program_route_state(outputs: &[CollaborationOutputRoute], output_ids: &[String]) -> String {
    let selected = outputs
        .iter()
        .filter(|output| output_ids.contains(&output.id))
        .collect::<Vec<_>>();
    if selected
        .iter()
        .any(|output| matches!(output.route_state.as_str(), "active" | "degraded"))
    {
        if selected
            .iter()
            .any(|output| output.route_state == "degraded")
        {
            "degraded".to_string()
        } else {
            "active".to_string()
        }
    } else if selected
        .iter()
        .any(|output| matches!(output.route_state.as_str(), "issued" | "armed"))
    {
        "armed".to_string()
    } else if selected
        .iter()
        .any(|output| output.route_state == "pending_source")
    {
        "pending_source".to_string()
    } else {
        "inactive".to_string()
    }
}

fn push_unique(ids: &mut Vec<String>, value: String) {
    if !ids.contains(&value) {
        ids.push(value);
    }
}

fn build_topology_engine(
    programs: &[CollaborationProgramRoute],
    audio: &[CollaborationAudioRoute],
    outputs: &[CollaborationOutputRoute],
    contributions: &[CollaborationContributionAttachment],
    mirrored_creator_ids: &[String],
    mix_minus_required: bool,
) -> CollaborationExecutionPlan {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for contribution in contributions {
        nodes.push(CollaborationExecutionNode {
            id: format!("contrib-{}", contribution.participant_id),
            node_kind: "contribution".to_string(),
            route_state: contribution.contribution_state.clone(),
            participant_id: Some(contribution.participant_id.clone()),
            target_creator_id: contribution.creator_id.clone(),
            target_broadcast_id: contribution.source_broadcast_id.clone(),
            mix_minus_required: contribution.mix_minus_required,
        });
    }
    for program in programs {
        nodes.push(CollaborationExecutionNode {
            id: program.id.clone(),
            node_kind: program.program_kind.clone(),
            route_state: program.route_state.clone(),
            participant_id: None,
            target_creator_id: program.target_creator_id.clone(),
            target_broadcast_id: program.target_broadcast_id.clone(),
            mix_minus_required: program.mix_minus_required,
        });
    }
    for route in audio {
        nodes.push(CollaborationExecutionNode {
            id: format!("audio-{}", route.participant_id),
            node_kind: route.route_kind.clone(),
            route_state: route.route_state.clone(),
            participant_id: Some(route.participant_id.clone()),
            target_creator_id: route.creator_id.clone(),
            target_broadcast_id: None,
            mix_minus_required: route.mix_minus_required,
        });
    }
    for output in outputs {
        nodes.push(CollaborationExecutionNode {
            id: output.id.clone(),
            node_kind: output.output_kind.clone(),
            route_state: output.route_state.clone(),
            participant_id: None,
            target_creator_id: output.target_creator_id.clone(),
            target_broadcast_id: output.target_broadcast_id.clone(),
            mix_minus_required: output.mix_minus_required,
        });
    }

    for program in programs {
        for participant_id in &program.source_participant_ids {
            edges.push(CollaborationExecutionEdge {
                id: format!("edge-contrib-{participant_id}-{}", program.id),
                edge_kind: "contribution_to_program".to_string(),
                from_node_id: format!("contrib-{participant_id}"),
                to_node_id: program.id.clone(),
                route_state: program.route_state.clone(),
                excluded_participant_ids: Vec::new(),
            });
        }
        for output_id in &program.output_ids {
            edges.push(CollaborationExecutionEdge {
                id: format!("edge-program-{}-{output_id}", program.id),
                edge_kind: "program_to_output".to_string(),
                from_node_id: program.id.clone(),
                to_node_id: output_id.clone(),
                route_state: program.route_state.clone(),
                excluded_participant_ids: Vec::new(),
            });
        }
    }

    let host_program_id = programs
        .iter()
        .find(|program| program.program_kind == "host_program")
        .map(|program| program.id.clone());
    for route in audio {
        if route.receive_program_audio {
            if let Some(host_program_id) = host_program_id.as_ref() {
                edges.push(CollaborationExecutionEdge {
                    id: format!("edge-program-{host_program_id}-audio-{}", route.participant_id),
                    edge_kind: "program_to_audio_return".to_string(),
                    from_node_id: host_program_id.clone(),
                    to_node_id: format!("audio-{}", route.participant_id),
                    route_state: route.route_state.clone(),
                    excluded_participant_ids: route.excluded_participant_ids.clone(),
                });
            }
        }
    }

    CollaborationExecutionPlan {
        execution_mode: "topology_graph_v1".to_string(),
        fanout_mode: if mirrored_creator_ids.is_empty() {
            "host_only".to_string()
        } else {
            "mirrored_collaboration".to_string()
        },
        audio_mode: if mix_minus_required {
            "mix_minus".to_string()
        } else {
            "program_only".to_string()
        },
        nodes,
        edges,
    }
}

pub(crate) async fn build_collaboration_runtime_response_for_participant(
    pool: &SqlitePool,
    session: CollaborationSessionView,
) -> AppResult<CollaborationRuntimeResponse> {
    let session_grants = fetch_collaboration_mirror_grants_for_session(pool, &session.id).await?;
    let session_pickups = fetch_collaboration_mirror_pickups_for_session(pool, &session.id).await?;
    let visible_grants =
        fetch_visible_collaboration_mirror_grants_for_session_view(pool, &session).await?;
    let visible_pickups =
        fetch_visible_collaboration_mirror_pickups_for_session_view(pool, &session).await?;
    let recent_events = filter_visible_collaboration_events_for_session(
        &session,
        fetch_collaboration_events(pool, &session.id, 0, 100).await?,
    );
    let connected_participants =
        count_active_collaboration_socket_sessions(pool, &session.id).await?;
    let topology = build_collaboration_runtime_topology(
        pool,
        &session,
        &session_grants,
        &session_pickups,
        connected_participants,
    )
    .await?;
    Ok(CollaborationRuntimeResponse {
        session,
        topology,
        grants: visible_grants,
        pickups: visible_pickups,
        recent_events,
    })
}

pub(crate) async fn build_collaboration_runtime_response_for_host(
    pool: &SqlitePool,
    session: CollaborationSession,
) -> AppResult<CollaborationRuntimeResponse> {
    let host = fetch_collaboration_host_summary(pool, &session.host_creator_id).await?;
    let view = collaboration_session_view_for_host(session, host)?;
    build_collaboration_runtime_response_for_participant(pool, view).await
}

pub(crate) async fn build_creator_collaboration_control_response_for_host(
    pool: &SqlitePool,
    session: CollaborationSession,
) -> AppResult<CreatorCollaborationControlResponse> {
    let runtime = build_collaboration_runtime_response_for_host(pool, session).await?;
    let socket_sessions =
        fetch_collaboration_socket_presence_for_session(pool, &runtime.session.id).await?;
    let pending_invite_count = fetch_collaboration_invites_for_session(pool, &runtime.session.id)
        .await?
        .into_iter()
        .filter(|invite| invite.state == "pending")
        .count() as i64;
    let active_grant_count = runtime
        .grants
        .iter()
        .filter(|grant| grant.state == "active")
        .count() as i64;
    let issued_grant_count = runtime
        .grants
        .iter()
        .filter(|grant| grant.state == "issued")
        .count() as i64;
    let stale_socket_count = socket_sessions
        .iter()
        .filter(|socket| socket.is_stale && socket.disconnected_at.is_none())
        .count() as i64;
    Ok(CreatorCollaborationControlResponse {
        runtime,
        socket_sessions,
        pending_invite_count,
        active_grant_count,
        issued_grant_count,
        stale_socket_count,
    })
}

pub(crate) async fn fetch_creator_live_collaboration_summary(
    pool: &SqlitePool,
    creator_id: &str,
    snapshot: &crate::models::CreatorLiveSnapshot,
) -> AppResult<CreatorLiveCollaborationSummary> {
    let sessions = fetch_collaboration_sessions_for_host(pool, creator_id).await?;
    let active_session = if let Some(current_broadcast) = snapshot.current_broadcast.as_ref() {
        sessions
            .iter()
            .find(|session| {
                session.source_broadcast_id == current_broadcast.id
                    && matches!(session.status.as_str(), "active" | "pending")
            })
            .cloned()
    } else if let Some(pending_broadcast) = snapshot.pending_broadcast.as_ref() {
        sessions
            .iter()
            .find(|session| {
                session.source_broadcast_id == pending_broadcast.id
                    && matches!(session.status.as_str(), "active" | "pending")
            })
            .cloned()
    } else {
        sessions
            .iter()
            .find(|session| matches!(session.status.as_str(), "active" | "pending"))
            .cloned()
    };

    let active_control = if let Some(session) = active_session.clone() {
        Some(build_creator_collaboration_control_response_for_host(pool, session).await?)
    } else {
        None
    };

    let pending_invite_count = sessions
        .iter()
        .map(|session| {
            session
                .invites
                .iter()
                .filter(|invite| invite.state == "pending")
                .count() as i64
        })
        .sum();

    let mut active_grant_count = 0_i64;
    let mut issued_grant_count = 0_i64;
    for session in &sessions {
        let grants = fetch_collaboration_mirror_grants_for_session(pool, &session.id).await?;
        active_grant_count += grants
            .iter()
            .filter(|grant| grant.state == "active")
            .count() as i64;
        issued_grant_count += grants
            .iter()
            .filter(|grant| grant.state == "issued")
            .count() as i64;
    }

    Ok(CreatorLiveCollaborationSummary {
        active_session,
        active_control,
        recent_sessions: sessions.iter().take(10).cloned().collect(),
        total_sessions: sessions.len() as i64,
        active_session_count: sessions
            .iter()
            .filter(|session| matches!(session.status.as_str(), "active" | "pending"))
            .count() as i64,
        pending_invite_count,
        active_grant_count,
        issued_grant_count,
    })
}
