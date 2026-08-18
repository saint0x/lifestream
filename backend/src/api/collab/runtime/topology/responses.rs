use super::presence::fetch_collaboration_socket_presence_for_session;
use super::*;
use crate::api::collab::fetch_collaboration_invites_for_session;

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
