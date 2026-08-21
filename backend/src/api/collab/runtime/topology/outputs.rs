use super::*;
use crate::models::{CollaborationOutputRoute, CollaborationParticipant};

pub(super) fn mirror_output_is_authorized(
    active_grant: Option<&CollaborationMirrorGrant>,
    issued_grant: Option<&CollaborationMirrorGrant>,
    active_pickup: Option<&CollaborationMirrorPickup>,
) -> bool {
    active_pickup.is_some() || active_grant.is_some() || issued_grant.is_some()
}

pub(super) fn build_topology_outputs(
    session: &CollaborationSessionView,
    grants: &[CollaborationMirrorGrant],
    pickups: &[CollaborationMirrorPickup],
    host_source_session: &Option<LiveIngestSession>,
    host_output_participant_ids: &[String],
    mix_minus_required: bool,
) -> Vec<CollaborationOutputRoute> {
    let host_output_id = format!("col-out-host-{}", session.id);
    let host_archive_output_id = format!("col-out-archive-host-{}", session.id);
    let host_output_state = match host_source_session
        .as_ref()
        .map(|session| session.contribution_state.as_str())
    {
        Some("healthy" | "attached") => "active",
        Some("degraded" | "stale") => "degraded",
        Some("disconnected") => "inactive",
        Some(_) => "pending_source",
        None => "pending_source",
    }
    .to_string();

    let mut outputs = vec![CollaborationOutputRoute {
        id: host_output_id,
        output_kind: "host_channel".to_string(),
        route_state: host_output_state.clone(),
        target_creator_id: Some(session.host_creator_id.clone()),
        target_broadcast_id: Some(session.source_broadcast_id.clone()),
        source_participant_ids: host_output_participant_ids.to_vec(),
        playback_enabled: matches!(host_output_state.as_str(), "active" | "degraded"),
        recording_enabled: false,
        mix_minus_required,
    }];

    if session.recording_policy == "host_archive" || session.recording_policy == "split_archive" {
        outputs.push(CollaborationOutputRoute {
            id: host_archive_output_id,
            output_kind: "archive".to_string(),
            route_state: if matches!(host_output_state.as_str(), "active" | "degraded") {
                "armed".to_string()
            } else {
                "pending_source".to_string()
            },
            target_creator_id: Some(session.host_creator_id.clone()),
            target_broadcast_id: Some(session.source_broadcast_id.clone()),
            source_participant_ids: host_output_participant_ids.to_vec(),
            playback_enabled: false,
            recording_enabled: true,
            mix_minus_required,
        });
    }

    for participant in &session.participants {
        if participant.role == "host" || participant.creator_id.is_none() {
            continue;
        }
        let active_grant = grants
            .iter()
            .find(|grant| grant.participant_id == participant.id && grant.state == "active");
        let issued_grant = grants
            .iter()
            .find(|grant| grant.participant_id == participant.id && grant.state == "issued");
        let active_pickup = pickups
            .iter()
            .find(|pickup| pickup.participant_id == participant.id && pickup.state == "active");
        if !participant.mirror_to_guest_channel
            || !mirror_output_is_authorized(active_grant, issued_grant, active_pickup)
        {
            continue;
        }

        let route_state = if let Some(_pickup) = active_pickup {
            match host_output_state.as_str() {
                "active" => "active".to_string(),
                "degraded" => "degraded".to_string(),
                _ => "pending_source".to_string(),
            }
        } else if let Some(grant) = active_grant {
            grant.state.clone()
        } else if let Some(grant) = issued_grant {
            grant.state.clone()
        } else {
            "inactive".to_string()
        };
        let source_participant_ids = if matches!(host_output_state.as_str(), "active" | "degraded")
        {
            if participant.publish_to_host {
                host_output_participant_ids.to_vec()
            } else {
                vec![participant.id.clone()]
            }
        } else {
            Vec::new()
        };
        outputs.push(CollaborationOutputRoute {
            id: format!("col-out-mirror-{}", participant.id),
            output_kind: "mirror_channel".to_string(),
            route_state: route_state.clone(),
            target_creator_id: participant.creator_id.clone(),
            target_broadcast_id: active_pickup.map(|pickup| pickup.guest_broadcast_id.clone()),
            source_participant_ids: source_participant_ids.clone(),
            playback_enabled: matches!(route_state.as_str(), "active" | "degraded"),
            recording_enabled: false,
            mix_minus_required: participant.publish_to_host && participant.state == "live",
        });
        if session.recording_policy == "split_archive" && participant.mirror_to_guest_channel {
            outputs.push(CollaborationOutputRoute {
                id: format!("col-out-archive-{}", participant.id),
                output_kind: "archive".to_string(),
                route_state: if matches!(route_state.as_str(), "active" | "degraded" | "issued") {
                    "armed".to_string()
                } else {
                    route_state.clone()
                },
                target_creator_id: participant.creator_id.clone(),
                target_broadcast_id: active_pickup.map(|pickup| pickup.guest_broadcast_id.clone()),
                source_participant_ids: source_participant_ids,
                playback_enabled: false,
                recording_enabled: true,
                mix_minus_required: participant.publish_to_host && participant.state == "live",
            });
        }
    }

    outputs
}

pub(super) fn planned_output_ids_for_participant(
    session: &CollaborationSessionView,
    participant: &CollaborationParticipant,
    outputs: &[CollaborationOutputRoute],
    host_output_participant_ids: &[String],
) -> Vec<String> {
    let mirror_output_id = format!("col-out-mirror-{}", participant.id);
    let archive_output_id = format!("col-out-archive-{}", participant.id);

    outputs
        .iter()
        .filter(|output| {
            if participant.role == "host" || participant.publish_to_host {
                output.id == format!("col-out-host-{}", session.id)
                    || output.id == format!("col-out-archive-host-{}", session.id)
                    || output.id == mirror_output_id
                    || output.id == archive_output_id
                    || output.source_participant_ids == host_output_participant_ids
            } else {
                output.id == mirror_output_id || output.id == archive_output_id
            }
        })
        .map(|output| output.id.clone())
        .collect()
}
