use super::*;

pub(crate) fn validate_collaboration_participant_access(
    participant: &CollaborationParticipant,
) -> AppResult<()> {
    if matches!(participant.state.as_str(), "left" | "removed") {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

pub(crate) fn collaboration_session_view_for_host(
    session: CollaborationSession,
    host: CollaborationHostSummary,
) -> AppResult<CollaborationSessionView> {
    let participant = session
        .participants
        .iter()
        .find(|participant| participant.role == "host")
        .cloned()
        .ok_or_else(|| {
            AppError::Internal("collaboration session is missing a host participant".to_string())
        })?;
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

pub(crate) async fn fetch_collaboration_participants_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<Vec<CollaborationParticipant>> {
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, invite_id, user_id, creator_id, role, state, publish_to_host,
               mirror_to_guest_channel, can_speak_in_chat, joined_at, left_at, created_at, updated_at
        FROM collaboration_participants
        WHERE session_id = ?
        ORDER BY created_at ASC
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| CollaborationParticipant {
            id: row.get("id"),
            session_id: row.get("session_id"),
            invite_id: row.get("invite_id"),
            user_id: row.get("user_id"),
            creator_id: row.get("creator_id"),
            role: row.get("role"),
            state: row.get("state"),
            publish_to_host: row.get::<i64, _>("publish_to_host") == 1,
            mirror_to_guest_channel: row.get::<i64, _>("mirror_to_guest_channel") == 1,
            can_speak_in_chat: row.get::<i64, _>("can_speak_in_chat") == 1,
            joined_at: row.get("joined_at"),
            left_at: row.get("left_at"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .collect())
}

pub(crate) async fn fetch_collaboration_participant_by_id(
    pool: &SqlitePool,
    participant_id: &str,
) -> AppResult<CollaborationParticipant> {
    let row = sqlx::query(
        r#"
        SELECT id, session_id, invite_id, user_id, creator_id, role, state, publish_to_host,
               mirror_to_guest_channel, can_speak_in_chat, joined_at, left_at, created_at, updated_at
        FROM collaboration_participants
        WHERE id = ?
        "#,
    )
    .bind(participant_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(CollaborationParticipant {
        id: row.get("id"),
        session_id: row.get("session_id"),
        invite_id: row.get("invite_id"),
        user_id: row.get("user_id"),
        creator_id: row.get("creator_id"),
        role: row.get("role"),
        state: row.get("state"),
        publish_to_host: row.get::<i64, _>("publish_to_host") == 1,
        mirror_to_guest_channel: row.get::<i64, _>("mirror_to_guest_channel") == 1,
        can_speak_in_chat: row.get::<i64, _>("can_speak_in_chat") == 1,
        joined_at: row.get("joined_at"),
        left_at: row.get("left_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(crate) async fn fetch_collaboration_participant_for_user(
    pool: &SqlitePool,
    session_id: &str,
    user_id: &str,
) -> AppResult<CollaborationParticipant> {
    let row = sqlx::query(
        r#"
        SELECT id
        FROM collaboration_participants
        WHERE session_id = ? AND user_id = ?
        "#,
    )
    .bind(session_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let participant_id: String = row.get("id");
    fetch_collaboration_participant_by_id(pool, &participant_id).await
}

pub(crate) async fn fetch_collaboration_host_summary(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CollaborationHostSummary> {
    let profile = fetch_creator_profile(pool, creator_id).await?;
    Ok(CollaborationHostSummary {
        creator_id: profile.id,
        user_id: profile.user_id,
        handle: profile.handle,
        display_name: profile.display_name,
        avatar: profile.avatar,
        partner_status: profile.partner_status,
        live_status: contract_live_status(&profile.live_status),
        current_broadcast_id: profile.current_broadcast_id,
    })
}
