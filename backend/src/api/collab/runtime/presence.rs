use super::*;

pub(crate) fn filter_visible_collaboration_mirror_pickups_for_session_view(
    session: &CollaborationSessionView,
    pickups: &[CollaborationMirrorPickup],
) -> Vec<CollaborationMirrorPickup> {
    if session.participant.role == "host" {
        pickups.to_vec()
    } else {
        pickups
            .iter()
            .filter(|pickup| pickup.participant_id == session.participant.id)
            .cloned()
            .collect()
    }
}

pub(crate) fn filter_visible_collaboration_mirror_grants_for_session_view(
    session: &CollaborationSessionView,
    grants: &[CollaborationMirrorGrant],
) -> Vec<CollaborationMirrorGrant> {
    if session.participant.role == "host" {
        grants.to_vec()
    } else {
        grants
            .iter()
            .filter(|grant| grant.participant_id == session.participant.id)
            .cloned()
            .collect()
    }
}

pub(crate) async fn fetch_collaboration_socket_presence_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<Vec<CollaborationSocketPresence>> {
    let cutoff = active_presence_cutoff();
    let rows = sqlx::query(
        r#"
        SELECT id, collaboration_session_id, user_id, creator_id, participant_id,
               connected_at, last_seen_at, disconnected_at
        FROM collaboration_socket_sessions
        WHERE collaboration_session_id = ?
        ORDER BY connected_at DESC
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let last_seen_at: String = row.get("last_seen_at");
            let disconnected_at: Option<String> = row.get("disconnected_at");
            CollaborationSocketPresence {
                id: row.get("id"),
                session_id: row.get("collaboration_session_id"),
                user_id: row.get("user_id"),
                creator_id: row.get("creator_id"),
                participant_id: row.get("participant_id"),
                connected_at: row.get("connected_at"),
                last_seen_at: last_seen_at.clone(),
                disconnected_at,
                is_stale: last_seen_at < cutoff,
            }
        })
        .collect())
}

pub(crate) async fn fetch_collaboration_socket_presence_by_id_raw(
    pool: &SqlitePool,
    session_id: &str,
    socket_id: &str,
) -> AppResult<CollaborationSocketPresence> {
    let cutoff = active_presence_cutoff();
    let row = sqlx::query(
        r#"
        SELECT id, collaboration_session_id, user_id, creator_id, participant_id,
               connected_at, last_seen_at, disconnected_at
        FROM collaboration_socket_sessions
        WHERE collaboration_session_id = ? AND id = ?
        "#,
    )
    .bind(session_id)
    .bind(socket_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let last_seen_at: String = row.get("last_seen_at");
    let disconnected_at: Option<String> = row.get("disconnected_at");
    Ok(CollaborationSocketPresence {
        id: row.get("id"),
        session_id: row.get("collaboration_session_id"),
        user_id: row.get("user_id"),
        creator_id: row.get("creator_id"),
        participant_id: row.get("participant_id"),
        connected_at: row.get("connected_at"),
        last_seen_at: last_seen_at.clone(),
        disconnected_at,
        is_stale: last_seen_at < cutoff,
    })
}
