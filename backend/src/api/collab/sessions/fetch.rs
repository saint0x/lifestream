use super::*;

pub(crate) async fn resolve_collaboration_broadcast(
    pool: &SqlitePool,
    creator_id: &str,
    broadcast_id: Option<&str>,
) -> AppResult<Broadcast> {
    match broadcast_id {
        Some(id) => fetch_broadcast_by_id(pool, creator_id, id).await,
        None => fetch_broadcasts(pool, creator_id)
            .await?
            .into_iter()
            .find(|broadcast| broadcast.status == "live" || broadcast.status == "ready")
            .ok_or_else(|| {
                AppError::BadRequest(
                    "a live or ready broadcast is required to start collaboration".to_string(),
                )
            }),
    }
}

pub(crate) async fn fetch_collaboration_sessions_for_host(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<CollaborationSession>> {
    let rows = sqlx::query(
        r#"
        SELECT id
        FROM collaboration_sessions
        WHERE host_creator_id = ?
        ORDER BY created_at DESC
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    let mut sessions = Vec::with_capacity(rows.len());
    for row in rows {
        let session_id: String = row.get("id");
        sessions.push(fetch_collaboration_session_for_host(pool, creator_id, &session_id).await?);
    }
    Ok(sessions)
}

pub(crate) async fn fetch_collaboration_sessions_for_participant(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<Vec<CollaborationSessionView>> {
    let rows = sqlx::query(
        r#"
        SELECT session_id
        FROM collaboration_participants
        WHERE user_id = ?
        ORDER BY created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut sessions = Vec::with_capacity(rows.len());
    for row in rows {
        let session_id: String = row.get("session_id");
        match fetch_collaboration_session_for_participant(pool, user_id, &session_id).await {
            Ok(session) => sessions.push(session),
            Err(AppError::Forbidden) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(sessions)
}

pub(crate) async fn fetch_collaboration_session_by_id(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<CollaborationSession> {
    let row = sqlx::query(
        r#"
        SELECT id, host_creator_id, source_broadcast_id, title, status, chat_mode,
               recording_policy, last_event_seq, created_at, updated_at, activated_at, ended_at
        FROM collaboration_sessions
        WHERE id = ?
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(CollaborationSession {
        id: row.get("id"),
        host_creator_id: row.get("host_creator_id"),
        source_broadcast_id: row.get("source_broadcast_id"),
        title: row.get("title"),
        status: row.get("status"),
        chat_mode: row.get("chat_mode"),
        recording_policy: row.get("recording_policy"),
        last_event_seq: row.get("last_event_seq"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        activated_at: row.get("activated_at"),
        ended_at: row.get("ended_at"),
        invites: fetch_collaboration_invites_for_session(pool, session_id).await?,
        participants: fetch_collaboration_participants_for_session(pool, session_id).await?,
    })
}

pub(crate) async fn fetch_active_collaboration_session_for_broadcast(
    pool: &SqlitePool,
    broadcast_id: &str,
) -> AppResult<Option<CollaborationSession>> {
    let row = sqlx::query(
        r#"
        SELECT id, host_creator_id, source_broadcast_id, title, status, chat_mode, recording_policy,
               last_event_seq, created_at, updated_at, activated_at, ended_at
        FROM collaboration_sessions
        WHERE source_broadcast_id = ? AND status IN ('pending', 'active')
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(broadcast_id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(row) => {
            let session_id: String = row.get("id");
            fetch_collaboration_session_by_id(pool, &session_id)
                .await
                .map(Some)
        }
        None => Ok(None),
    }
}

pub(crate) async fn fetch_collaboration_session_for_host(
    pool: &SqlitePool,
    creator_id: &str,
    session_id: &str,
) -> AppResult<CollaborationSession> {
    let session = fetch_collaboration_session_by_id(pool, session_id).await?;
    if session.host_creator_id != creator_id {
        return Err(AppError::NotFound);
    }
    Ok(session)
}

pub(crate) async fn fetch_collaboration_session_for_participant(
    pool: &SqlitePool,
    user_id: &str,
    session_id: &str,
) -> AppResult<CollaborationSessionView> {
    let session = fetch_collaboration_session_by_id(pool, session_id).await?;
    let participant = fetch_collaboration_participant_for_user(pool, session_id, user_id).await?;
    validate_collaboration_participant_access(&participant)?;
    let host = fetch_collaboration_host_summary(pool, &session.host_creator_id).await?;
    Ok(CollaborationSessionView {
        id: session.id,
        host_creator_id: session.host_creator_id,
        source_broadcast_id: session.source_broadcast_id,
        title: session.title,
        status: session.status,
        chat_mode: session.chat_mode,
        recording_policy: session.recording_policy,
        last_event_seq: session.last_event_seq,
        created_at: session.created_at,
        updated_at: session.updated_at,
        activated_at: session.activated_at,
        ended_at: session.ended_at,
        host,
        participant,
        participants: session.participants,
    })
}
