use super::presence::{
    fetch_collaboration_socket_presence_for_session,
    fetch_visible_collaboration_mirror_grants_for_session_view,
    fetch_visible_collaboration_mirror_pickups_for_session_view,
};
use super::*;

pub(crate) fn build_collaboration_runtime_topology(
    session: &CollaborationSessionView,
    grants: &[CollaborationMirrorGrant],
    pickups: &[CollaborationMirrorPickup],
    connected_participants: i64,
) -> CollaborationRuntimeTopology {
    let shared_chat = session.chat_mode == "shared";
    let recording_owner_creator_id = match session.recording_policy.as_str() {
        "host_archive" => Some(session.host_creator_id.clone()),
        _ => None,
    };
    let mut host_output_participant_ids = Vec::new();
    let mut backstage_participant_ids = Vec::new();
    let mut live_participant_ids = Vec::new();
    let mut mirrored_creator_ids = Vec::new();
    let mut members = Vec::with_capacity(session.participants.len());

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

    CollaborationRuntimeTopology {
        session_id: session.id.clone(),
        source_broadcast_id: session.source_broadcast_id.clone(),
        chat_mode: session.chat_mode.clone(),
        recording_policy: session.recording_policy.clone(),
        shared_chat,
        recording_owner_creator_id,
        connected_participants,
        host_output_participant_ids,
        backstage_participant_ids,
        live_participant_ids,
        mirrored_creator_ids,
        members,
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
        &session,
        &session_grants,
        &session_pickups,
        connected_participants,
    );
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
