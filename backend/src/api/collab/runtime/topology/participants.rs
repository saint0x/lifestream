use super::outputs::mirror_output_is_authorized;
use super::*;
use crate::models::{CollaborationContributionAttachment, CollaborationTopologyMember};

pub(super) struct TopologyParticipantState {
    pub(super) host_output_participant_ids: Vec<String>,
    pub(super) backstage_participant_ids: Vec<String>,
    pub(super) live_participant_ids: Vec<String>,
    pub(super) mirrored_creator_ids: Vec<String>,
    pub(super) contributions: Vec<CollaborationContributionAttachment>,
    pub(super) members: Vec<CollaborationTopologyMember>,
    pub(super) mix_minus_required: bool,
}

pub(super) fn build_topology_participants(
    session: &CollaborationSessionView,
    grants: &[CollaborationMirrorGrant],
    pickups: &[CollaborationMirrorPickup],
    socket_sessions: &[CollaborationSocketPresence],
    host_source_session: Option<&LiveIngestSession>,
) -> TopologyParticipantState {
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
        let participant_mix_minus_required = participant.role != "host"
            && participant.publish_to_host
            && participant.state == "live";
        mix_minus_required |= participant_mix_minus_required;
        let authorized_mirror_output = participant.mirror_to_guest_channel
            && mirror_output_is_authorized(active_grant, issued_grant, active_pickup);

        let mirror_output_id = participant
            .creator_id
            .as_ref()
            .map(|_| format!("col-out-mirror-{}", participant.id));
        let mirror_archive_output_id = participant
            .creator_id
            .as_ref()
            .map(|_| format!("col-out-archive-{}", participant.id));

        let mut attached_output_ids = Vec::new();
        if participant.role == "host" || participant.publish_to_host {
            attached_output_ids.push(format!("col-out-host-{}", session.id));
            if matches!(
                session.recording_policy.as_str(),
                "host_archive" | "split_archive"
            ) {
                attached_output_ids.push(format!("col-out-archive-host-{}", session.id));
            }
        }
        if authorized_mirror_output {
            if let Some(mirror_output_id) = mirror_output_id {
                attached_output_ids.push(mirror_output_id);
            }
            if session.recording_policy == "split_archive" {
                if let Some(mirror_archive_output_id) = mirror_archive_output_id {
                    attached_output_ids.push(mirror_archive_output_id);
                }
            }
        }

        let transport_class = if participant.role == "host" {
            host_source_session
                .map(|source| source.contribution_class.clone())
                .unwrap_or_else(|| "missing_source".to_string())
        } else {
            "collaboration_socket".to_string()
        };
        let contribution_state = if participant.role == "host" {
            host_source_session
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
        let host_output_state = participant_host_output_state(participant);
        let mirror_pickup_state = participant_mirror_pickup_state(
            participant,
            active_grant,
            issued_grant,
            active_pickup,
            &mut mirrored_creator_ids,
        );

        contributions.push(CollaborationContributionAttachment {
            participant_id: participant.id.clone(),
            user_id: participant.user_id.clone(),
            creator_id: participant.creator_id.clone(),
            transport_class,
            media_transport: participant.media_transport.clone(),
            contribution_endpoint_url: participant.contribution_endpoint_url.clone(),
            return_endpoint_url: participant.return_endpoint_url.clone(),
            source_broadcast_id: if participant.role == "host" {
                Some(session.source_broadcast_id.clone())
            } else {
                active_pickup
                    .map(|pickup| pickup.source_broadcast_id.clone())
                    .or_else(|| Some(session.source_broadcast_id.clone()))
            },
            ingest_session_id: if participant.role == "host" {
                host_source_session.map(|source| source.id.clone())
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

    TopologyParticipantState {
        host_output_participant_ids,
        backstage_participant_ids,
        live_participant_ids,
        mirrored_creator_ids,
        contributions,
        members,
        mix_minus_required,
    }
}

fn participant_host_output_state(participant: &CollaborationParticipant) -> String {
    if participant.role == "host" {
        "host".to_string()
    } else if !participant.publish_to_host {
        "disabled".to_string()
    } else {
        match participant.state.as_str() {
            "live" => "live".to_string(),
            "backstage" => "backstage".to_string(),
            _ => "inactive".to_string(),
        }
    }
}

fn participant_mirror_pickup_state(
    participant: &CollaborationParticipant,
    active_grant: Option<&CollaborationMirrorGrant>,
    issued_grant: Option<&CollaborationMirrorGrant>,
    active_pickup: Option<&CollaborationMirrorPickup>,
    mirrored_creator_ids: &mut Vec<String>,
) -> String {
    if participant.role == "host" {
        return "host".to_string();
    }
    if let Some(pickup) = active_pickup {
        maybe_push_mirrored_creator_id(mirrored_creator_ids, participant.creator_id.as_ref());
        return pickup.state.clone();
    }
    if participant.creator_id.is_none() {
        return "unavailable".to_string();
    }
    if !participant.mirror_to_guest_channel {
        return "disabled".to_string();
    }
    if let Some(grant) = active_grant {
        maybe_push_mirrored_creator_id(mirrored_creator_ids, participant.creator_id.as_ref());
        return grant.state.clone();
    }
    if let Some(grant) = issued_grant {
        maybe_push_mirrored_creator_id(mirrored_creator_ids, participant.creator_id.as_ref());
        return grant.state.clone();
    }
    match participant.state.as_str() {
        "live" | "backstage" => "eligible".to_string(),
        _ => "inactive".to_string(),
    }
}

fn maybe_push_mirrored_creator_id(ids: &mut Vec<String>, creator_id: Option<&String>) {
    if let Some(creator_id) = creator_id {
        if !ids.contains(creator_id) {
            ids.push(creator_id.clone());
        }
    }
}
