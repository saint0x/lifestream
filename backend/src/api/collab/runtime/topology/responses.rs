use super::presence::fetch_collaboration_socket_presence_for_session;
use super::*;
use crate::api::collab::fetch_collaboration_invites_for_session;

const CREATOR_LIVE_RECENT_COLLABORATION_SESSIONS_LIMIT: i64 = 3;

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
    let active_session = if let Some(current_broadcast) = snapshot.current_broadcast.as_ref() {
        fetch_active_collaboration_session_for_broadcast(pool, &current_broadcast.id).await?
    } else if let Some(pending_broadcast) = snapshot.pending_broadcast.as_ref() {
        fetch_active_collaboration_session_for_broadcast(pool, &pending_broadcast.id).await?
    } else {
        fetch_latest_active_or_pending_collaboration_session_for_host(pool, creator_id).await?
    };

    let active_control = if let Some(session) = active_session.clone() {
        Some(build_creator_collaboration_control_response_for_host(pool, session).await?)
    } else {
        None
    };

    let recent_sessions = fetch_recent_collaboration_sessions_for_host(
        pool,
        creator_id,
        CREATOR_LIVE_RECENT_COLLABORATION_SESSIONS_LIMIT,
    )
    .await?;
    let total_sessions = count_collaboration_sessions_for_host(pool, creator_id).await?;
    let active_session_count =
        count_active_or_pending_collaboration_sessions_for_host(pool, creator_id).await?;
    let pending_invite_count = count_pending_collaboration_invites_for_host(pool, creator_id).await?;
    let active_grant_count = count_collaboration_grants_for_host_by_state(pool, creator_id, "active").await?;
    let issued_grant_count = count_collaboration_grants_for_host_by_state(pool, creator_id, "issued").await?;

    Ok(CreatorLiveCollaborationSummary {
        active_session,
        active_control,
        recent_sessions,
        total_sessions,
        active_session_count,
        pending_invite_count,
        active_grant_count,
        issued_grant_count,
    })
}

async fn fetch_latest_active_or_pending_collaboration_session_for_host(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Option<CollaborationSession>> {
    let row = sqlx::query(
        r#"
        SELECT id
        FROM collaboration_sessions
        WHERE host_creator_id = ? AND status IN ('active', 'pending')
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(creator_id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(row) => {
            let session_id: String = row.get("id");
            fetch_collaboration_session_for_host(pool, creator_id, &session_id)
                .await
                .map(Some)
        }
        None => Ok(None),
    }
}

async fn fetch_recent_collaboration_sessions_for_host(
    pool: &SqlitePool,
    creator_id: &str,
    limit: i64,
) -> AppResult<Vec<CollaborationSession>> {
    let rows = sqlx::query(
        r#"
        SELECT id
        FROM collaboration_sessions
        WHERE host_creator_id = ?
        ORDER BY created_at DESC
        LIMIT ?
        "#,
    )
    .bind(creator_id)
    .bind(limit.max(1))
    .fetch_all(pool)
    .await?;

    let mut sessions = Vec::with_capacity(rows.len());
    for row in rows {
        let session_id: String = row.get("id");
        sessions.push(fetch_collaboration_session_for_host(pool, creator_id, &session_id).await?);
    }
    Ok(sessions)
}

async fn count_collaboration_sessions_for_host(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<i64> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM collaboration_sessions
        WHERE host_creator_id = ?
        "#,
    )
    .bind(creator_id)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

async fn count_active_or_pending_collaboration_sessions_for_host(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<i64> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM collaboration_sessions
        WHERE host_creator_id = ? AND status IN ('active', 'pending')
        "#,
    )
    .bind(creator_id)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

async fn count_pending_collaboration_invites_for_host(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<i64> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM collaboration_invites invites
        JOIN collaboration_sessions sessions
          ON sessions.id = invites.session_id
        WHERE sessions.host_creator_id = ?
          AND invites.state = 'pending'
        "#,
    )
    .bind(creator_id)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

async fn count_collaboration_grants_for_host_by_state(
    pool: &SqlitePool,
    creator_id: &str,
    state: &str,
) -> AppResult<i64> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM collaboration_mirror_grants grants
        JOIN collaboration_sessions sessions
          ON sessions.id = grants.session_id
        WHERE sessions.host_creator_id = ?
          AND grants.state = ?
        "#,
    )
    .bind(creator_id)
    .bind(state)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}
