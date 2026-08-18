use super::*;

pub(super) async fn resolve_collaboration_broadcast(
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

pub(super) async fn fetch_collaboration_sessions_for_host(
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

pub(super) async fn fetch_collaboration_sessions_for_participant(
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

pub(super) async fn fetch_collaboration_session_by_id(
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

pub(super) async fn fetch_active_collaboration_session_for_broadcast(
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

pub(super) async fn end_collaboration_session_internal_raw(
    state: &SharedState,
    session: &CollaborationSession,
    actor_user_id: Option<String>,
    payload: Value,
) -> AppResult<CollaborationSession> {
    if session.status == "ended" {
        return fetch_collaboration_session_by_id(&state.pool, &session.id).await;
    }

    let now = Utc::now().to_rfc3339();
    let revoked_invites =
        fetch_pending_collaboration_invites_for_session(&state.pool, &session.id).await?;
    sqlx::query(
        "UPDATE collaboration_sessions SET status = 'ended', ended_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(&session.id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE collaboration_participants SET state = CASE WHEN state IN ('left', 'removed') THEN state ELSE 'left' END, left_at = COALESCE(left_at, ?), updated_at = ? WHERE session_id = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(&session.id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE collaboration_invites SET state = CASE WHEN state = 'pending' THEN 'revoked' ELSE state END, responded_at = COALESCE(responded_at, ?) WHERE session_id = ?",
    )
    .bind(&now)
    .bind(&session.id)
    .execute(&state.pool)
    .await?;
    publish_collaboration_invite_revoked_events_raw(
        state,
        &session.id,
        actor_user_id.clone(),
        &revoked_invites,
        &now,
        "session_ended",
    )
    .await?;
    revoke_collaboration_mirror_grants_for_session_raw(
        state,
        &session.id,
        actor_user_id.clone(),
        &now,
        "session_ended",
    )
    .await?;
    publish_collaboration_reconciliation_event(
        state,
        &session.id,
        actor_user_id,
        None,
        "session_ended",
        json!({
            "hostCreatorId": session.host_creator_id,
            "sourceBroadcastId": session.source_broadcast_id,
            "endedAt": now,
            "details": payload,
        }),
    )
    .await?;

    fetch_collaboration_session_by_id(&state.pool, &session.id).await
}

pub(super) async fn end_collaboration_session_internal(
    state: &SharedState,
    session: &CollaborationSession,
    actor_user_id: Option<String>,
    payload: Value,
) -> AppResult<CollaborationSession> {
    if session.status == "ended" {
        return fetch_collaboration_session_by_id(&state.pool, &session.id).await;
    }

    let now = Utc::now().to_rfc3339();
    let revoked_invites =
        fetch_pending_collaboration_invites_for_session(&state.pool, &session.id).await?;
    sqlx::query(
        "UPDATE collaboration_sessions SET status = 'ended', ended_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(&session.id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE collaboration_participants SET state = CASE WHEN state IN ('left', 'removed') THEN state ELSE 'left' END, left_at = COALESCE(left_at, ?), updated_at = ? WHERE session_id = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(&session.id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE collaboration_invites SET state = CASE WHEN state = 'pending' THEN 'revoked' ELSE state END, responded_at = COALESCE(responded_at, ?) WHERE session_id = ?",
    )
    .bind(&now)
    .bind(&session.id)
    .execute(&state.pool)
    .await?;
    publish_collaboration_invite_revoked_events(
        state,
        &session.id,
        actor_user_id.clone(),
        &revoked_invites,
        &now,
        "session_ended",
    )
    .await?;
    revoke_collaboration_mirror_grants_for_session(
        state,
        &session.id,
        actor_user_id.clone(),
        &now,
        "session_ended",
    )
    .await?;
    publish_collaboration_event(
        state,
        &session.id,
        actor_user_id,
        None,
        "session_ended",
        json!({
            "hostCreatorId": session.host_creator_id,
            "sourceBroadcastId": session.source_broadcast_id,
            "endedAt": now,
            "details": payload,
        }),
    )
    .await?;

    fetch_collaboration_session_by_id(&state.pool, &session.id).await
}

pub(super) async fn fetch_collaboration_session_for_host(
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

pub(super) async fn fetch_collaboration_session_for_participant(
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

pub(super) fn validate_collaboration_participant_access(
    participant: &CollaborationParticipant,
) -> AppResult<()> {
    if matches!(participant.state.as_str(), "left" | "removed") {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

pub(super) fn collaboration_session_view_for_host(
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

pub(super) async fn fetch_collaboration_invites_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<Vec<CollaborationInvite>> {
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, host_creator_id, invitee_user_id, invitee_creator_id, role, state,
               mirror_to_guest_channel, message, created_at, responded_at, expires_at
        FROM collaboration_invites
        WHERE session_id = ?
        ORDER BY created_at ASC
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| CollaborationInvite {
            id: row.get("id"),
            session_id: row.get("session_id"),
            host_creator_id: row.get("host_creator_id"),
            invitee_user_id: row.get("invitee_user_id"),
            invitee_creator_id: row.get("invitee_creator_id"),
            role: row.get("role"),
            state: row.get("state"),
            mirror_to_guest_channel: row.get::<i64, _>("mirror_to_guest_channel") == 1,
            message: row.get("message"),
            created_at: row.get("created_at"),
            responded_at: row.get("responded_at"),
            expires_at: row.get("expires_at"),
        })
        .collect())
}

pub(super) async fn fetch_pending_collaboration_invites_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<Vec<CollaborationInvite>> {
    Ok(fetch_collaboration_invites_for_session(pool, session_id)
        .await?
        .into_iter()
        .filter(|invite| invite.state == "pending")
        .collect())
}

pub(super) async fn fetch_collaboration_participants_for_session(
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

pub(super) async fn fetch_collaboration_invite_by_id(
    pool: &SqlitePool,
    invite_id: &str,
) -> AppResult<CollaborationInvite> {
    let row = sqlx::query(
        r#"
        SELECT id, session_id, host_creator_id, invitee_user_id, invitee_creator_id, role, state,
               mirror_to_guest_channel, message, created_at, responded_at, expires_at
        FROM collaboration_invites
        WHERE id = ?
        "#,
    )
    .bind(invite_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(CollaborationInvite {
        id: row.get("id"),
        session_id: row.get("session_id"),
        host_creator_id: row.get("host_creator_id"),
        invitee_user_id: row.get("invitee_user_id"),
        invitee_creator_id: row.get("invitee_creator_id"),
        role: row.get("role"),
        state: row.get("state"),
        mirror_to_guest_channel: row.get::<i64, _>("mirror_to_guest_channel") == 1,
        message: row.get("message"),
        created_at: row.get("created_at"),
        responded_at: row.get("responded_at"),
        expires_at: row.get("expires_at"),
    })
}

pub(super) async fn fetch_collaboration_participant_by_id(
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

pub(super) async fn fetch_collaboration_participant_for_user(
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

pub(super) async fn fetch_collaboration_invites_for_user(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<Vec<CollaborationInvite>> {
    let rows = sqlx::query(
        r#"
        SELECT id
        FROM collaboration_invites
        WHERE invitee_user_id = ?
          AND state = 'pending'
        ORDER BY created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    let mut invites = Vec::with_capacity(rows.len());
    for row in rows {
        let invite_id: String = row.get("id");
        invites.push(fetch_collaboration_invite_by_id(pool, &invite_id).await?);
    }
    Ok(invites)
}

pub(super) async fn fetch_collaboration_host_summary(
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

pub(super) async fn has_pending_collaboration_invite_for_user(
    pool: &SqlitePool,
    session_id: &str,
    user_id: &str,
) -> AppResult<bool> {
    let row = sqlx::query(
        r#"
        SELECT 1
        FROM collaboration_invites
        WHERE session_id = ? AND invitee_user_id = ? AND state = 'pending'
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some())
}

pub(super) async fn fetch_collaboration_events(
    pool: &SqlitePool,
    session_id: &str,
    after_seq: i64,
    limit: i64,
) -> AppResult<Vec<CollaborationEvent>> {
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, sequence, actor_user_id, participant_id, event_type, payload_json, created_at
        FROM collaboration_events
        WHERE session_id = ? AND sequence > ?
        ORDER BY sequence ASC
        LIMIT ?
        "#,
    )
    .bind(session_id)
    .bind(after_seq.max(0))
    .bind(limit.clamp(1, 500))
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| CollaborationEvent {
            id: row.get("id"),
            session_id: row.get("session_id"),
            sequence: row.get("sequence"),
            actor_user_id: row.get("actor_user_id"),
            participant_id: row.get("participant_id"),
            event_type: row.get("event_type"),
            payload: serde_json::from_str(&row.get::<String, _>("payload_json"))
                .unwrap_or(Value::Null),
            created_at: row.get("created_at"),
        })
        .collect())
}

pub(super) async fn load_collaboration_socket_event_bootstrap(
    pool: &SqlitePool,
    session_id: &str,
    after_seq: i64,
) -> AppResult<(Vec<CollaborationEvent>, Vec<CollaborationEvent>)> {
    if after_seq > 0 {
        let replay_events =
            fetch_collaboration_events(pool, session_id, after_seq.max(0), 100).await?;
        Ok((Vec::new(), replay_events))
    } else {
        let snapshot_events = fetch_collaboration_events(pool, session_id, 0, 100).await?;
        Ok((snapshot_events, Vec::new()))
    }
}

pub(super) fn collaboration_event_is_visible_to_session(
    session: &CollaborationSessionView,
    event: &CollaborationEvent,
) -> bool {
    if session.participant.role == "host" {
        return true;
    }

    match event.event_type.as_str() {
        "invite_created" | "invite_revoked" | "invite_expired" => {
            event.payload["inviteeUserId"] == Value::String(session.participant.user_id.clone())
        }
        "mirror_grant_issued"
        | "mirror_grant_redeemed"
        | "mirror_grant_revoked"
        | "mirror_grant_expired"
        | "participant_state_requested" => {
            event.participant_id.as_deref() == Some(session.participant.id.as_str())
                || event.payload["participantId"] == Value::String(session.participant.id.clone())
        }
        _ => true,
    }
}

pub(super) fn filter_visible_collaboration_events_for_session(
    session: &CollaborationSessionView,
    events: Vec<CollaborationEvent>,
) -> Vec<CollaborationEvent> {
    events
        .into_iter()
        .filter(|event| collaboration_event_is_visible_to_session(session, event))
        .collect()
}
