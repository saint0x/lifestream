use super::presence::{
    fetch_collaboration_socket_presence_for_session,
    filter_visible_collaboration_mirror_grants_for_session_view,
    filter_visible_collaboration_mirror_pickups_for_session_view,
};
use super::*;

const CREATOR_LIVE_RECENT_COLLABORATION_SESSIONS_LIMIT: i64 = 3;

struct CollaborationRuntimeBuild {
    runtime: CollaborationRuntimeResponse,
    socket_sessions: Vec<CollaborationSocketPresence>,
}

#[derive(sqlx::FromRow)]
struct CreatorCollaborationCountsRow {
    total_sessions: i64,
    active_session_count: i64,
    pending_invite_count: i64,
    active_grant_count: i64,
    issued_grant_count: i64,
}

async fn build_collaboration_runtime_for_session_view(
    pool: &SqlitePool,
    session: CollaborationSessionView,
) -> AppResult<CollaborationRuntimeBuild> {
    let (session_grants, session_pickups, recent_events, socket_sessions) = tokio::try_join!(
        fetch_collaboration_mirror_grants_for_session(pool, &session.id),
        fetch_collaboration_mirror_pickups_for_session(pool, &session.id),
        fetch_collaboration_events(pool, &session.id, 0, 100),
        fetch_collaboration_socket_presence_for_session(pool, &session.id),
    )?;
    let visible_grants =
        filter_visible_collaboration_mirror_grants_for_session_view(&session, &session_grants);
    let visible_pickups =
        filter_visible_collaboration_mirror_pickups_for_session_view(&session, &session_pickups);
    let topology = build_collaboration_runtime_topology(
        pool,
        &session,
        &session_grants,
        &session_pickups,
        &socket_sessions,
    )
    .await?;

    Ok(CollaborationRuntimeBuild {
        runtime: CollaborationRuntimeResponse {
            session: session.clone(),
            topology,
            grants: visible_grants,
            pickups: visible_pickups,
            recent_events: filter_visible_collaboration_events_for_session(&session, recent_events),
        },
        socket_sessions,
    })
}

async fn fetch_creator_collaboration_counts(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorCollaborationCountsRow> {
    sqlx::query_as(
        r#"
        SELECT
            (SELECT COUNT(*)
             FROM collaboration_sessions
             WHERE host_creator_id = ?) AS total_sessions,
            (SELECT COUNT(*)
             FROM collaboration_sessions
             WHERE host_creator_id = ? AND status IN ('active', 'pending')) AS active_session_count,
            (SELECT COUNT(*)
             FROM collaboration_invites invites
             JOIN collaboration_sessions sessions
               ON sessions.id = invites.session_id
             WHERE sessions.host_creator_id = ?
               AND invites.state = 'pending') AS pending_invite_count,
            (SELECT COUNT(*)
             FROM collaboration_mirror_grants grants
             JOIN collaboration_sessions sessions
               ON sessions.id = grants.session_id
             WHERE sessions.host_creator_id = ?
               AND grants.state = 'active') AS active_grant_count,
            (SELECT COUNT(*)
             FROM collaboration_mirror_grants grants
             JOIN collaboration_sessions sessions
               ON sessions.id = grants.session_id
             WHERE sessions.host_creator_id = ?
               AND grants.state = 'issued') AS issued_grant_count
        "#,
    )
    .bind(creator_id)
    .bind(creator_id)
    .bind(creator_id)
    .bind(creator_id)
    .bind(creator_id)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

pub(crate) async fn build_collaboration_runtime_response_for_participant(
    pool: &SqlitePool,
    session: CollaborationSessionView,
) -> AppResult<CollaborationRuntimeResponse> {
    Ok(build_collaboration_runtime_for_session_view(pool, session)
        .await?
        .runtime)
}

pub(crate) async fn build_collaboration_runtime_response_for_host(
    pool: &SqlitePool,
    session: CollaborationSession,
) -> AppResult<CollaborationRuntimeResponse> {
    let host = fetch_collaboration_host_summary(pool, &session.host_creator_id).await?;
    let view = collaboration_session_view_for_host(session, host)?;
    Ok(build_collaboration_runtime_for_session_view(pool, view)
        .await?
        .runtime)
}

pub(crate) async fn build_creator_collaboration_control_response_for_host(
    pool: &SqlitePool,
    session: CollaborationSession,
) -> AppResult<CreatorCollaborationControlResponse> {
    let pending_invite_count = session
        .invites
        .iter()
        .filter(|invite| invite.state == "pending")
        .count() as i64;
    let host = fetch_collaboration_host_summary(pool, &session.host_creator_id).await?;
    let view = collaboration_session_view_for_host(session, host)?;
    let CollaborationRuntimeBuild {
        runtime,
        socket_sessions,
    } = build_collaboration_runtime_for_session_view(pool, view).await?;
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

    let (active_control, recent_sessions, counts) = tokio::try_join!(
        async {
            match active_session.clone() {
                Some(session) => build_creator_collaboration_control_response_for_host(pool, session)
                    .await
                    .map(Some),
                None => Ok(None),
            }
        },
        fetch_recent_collaboration_sessions_for_host(
            pool,
            creator_id,
            CREATOR_LIVE_RECENT_COLLABORATION_SESSIONS_LIMIT,
        ),
        fetch_creator_collaboration_counts(pool, creator_id),
    )?;

    Ok(CreatorLiveCollaborationSummary {
        active_session,
        active_control,
        recent_sessions,
        total_sessions: counts.total_sessions,
        active_session_count: counts.active_session_count,
        pending_invite_count: counts.pending_invite_count,
        active_grant_count: counts.active_grant_count,
        issued_grant_count: counts.issued_grant_count,
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
