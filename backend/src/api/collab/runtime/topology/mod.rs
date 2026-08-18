use super::presence::{
    fetch_collaboration_socket_presence_for_session,
    fetch_visible_collaboration_mirror_grants_for_session_view,
    fetch_visible_collaboration_mirror_pickups_for_session_view,
};
use super::*;
use crate::api::collab::fetch_collaboration_invites_for_session;
use crate::models::{CollaborationContributionAttachment, CollaborationRuntimeTopology};

mod audio;
mod engine;
mod outputs;
mod programs;
mod resolve;

use audio::build_topology_audio;
use engine::build_topology_engine;
use outputs::{build_topology_outputs, mirror_output_is_authorized};
use programs::build_topology_programs;
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

fn push_unique(ids: &mut Vec<String>, value: String) {
    if !ids.contains(&value) {
        ids.push(value);
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
