use super::*;

pub(super) async fn publish_collaboration_mirror_grant_revoked_events(
    state: &SharedState,
    session_id: &str,
    actor_user_id: Option<String>,
    grants: &[CollaborationMirrorGrant],
    revoked_at: &str,
    reason: &str,
) -> AppResult<()> {
    for grant in grants {
        publish_collaboration_event(
            state,
            session_id,
            actor_user_id.clone(),
            Some(grant.participant_id.clone()),
            "mirror_grant_revoked",
            json!({
                "grantId": grant.id,
                "participantId": grant.participant_id,
                "guestCreatorId": grant.guest_creator_id,
                "scope": grant.scope,
                "revokedAt": revoked_at,
                "reason": reason,
            }),
        )
        .await?;
    }
    Ok(())
}

pub(super) async fn publish_collaboration_mirror_grant_revoked_events_raw(
    state: &SharedState,
    session_id: &str,
    actor_user_id: Option<String>,
    grants: &[CollaborationMirrorGrant],
    revoked_at: &str,
    reason: &str,
) -> AppResult<()> {
    for grant in grants {
        publish_collaboration_reconciliation_event(
            state,
            session_id,
            actor_user_id.clone(),
            Some(grant.participant_id.clone()),
            "mirror_grant_revoked",
            json!({
                "grantId": grant.id,
                "participantId": grant.participant_id,
                "guestCreatorId": grant.guest_creator_id,
                "scope": grant.scope,
                "revokedAt": revoked_at,
                "reason": reason,
            }),
        )
        .await?;
    }
    Ok(())
}

pub(super) async fn publish_collaboration_invite_revoked_events(
    state: &SharedState,
    session_id: &str,
    actor_user_id: Option<String>,
    invites: &[CollaborationInvite],
    revoked_at: &str,
    reason: &str,
) -> AppResult<()> {
    for invite in invites {
        publish_collaboration_event(
            state,
            session_id,
            actor_user_id.clone(),
            None,
            "invite_revoked",
            json!({
                "inviteId": invite.id,
                "inviteeUserId": invite.invitee_user_id,
                "inviteeCreatorId": invite.invitee_creator_id,
                "role": invite.role,
                "mirrorToGuestChannel": invite.mirror_to_guest_channel,
                "revokedAt": revoked_at,
                "reason": reason,
            }),
        )
        .await?;
    }
    Ok(())
}

pub(super) async fn publish_collaboration_invite_revoked_events_raw(
    state: &SharedState,
    session_id: &str,
    actor_user_id: Option<String>,
    invites: &[CollaborationInvite],
    revoked_at: &str,
    reason: &str,
) -> AppResult<()> {
    for invite in invites {
        publish_collaboration_reconciliation_event(
            state,
            session_id,
            actor_user_id.clone(),
            None,
            "invite_revoked",
            json!({
                "inviteId": invite.id,
                "inviteeUserId": invite.invitee_user_id,
                "inviteeCreatorId": invite.invitee_creator_id,
                "role": invite.role,
                "mirrorToGuestChannel": invite.mirror_to_guest_channel,
                "revokedAt": revoked_at,
                "reason": reason,
            }),
        )
        .await?;
    }
    Ok(())
}

pub(super) async fn publish_collaboration_event_raw(
    state: &SharedState,
    session_id: &str,
    actor_user_id: Option<String>,
    participant_id: Option<String>,
    event_type: &str,
    payload: Value,
) -> AppResult<CollaborationEvent> {
    let created_at = Utc::now().to_rfc3339();
    let row = sqlx::query(
        "UPDATE collaboration_sessions SET last_event_seq = last_event_seq + 1, updated_at = ? WHERE id = ? RETURNING last_event_seq",
    )
    .bind(&created_at)
    .bind(session_id)
    .fetch_one(&state.pool)
    .await?;
    let sequence: i64 = row.get("last_event_seq");
    let event = CollaborationEvent {
        id: format!("cole-{}", Uuid::new_v4().simple()),
        session_id: session_id.to_string(),
        sequence,
        actor_user_id,
        participant_id,
        event_type: event_type.to_string(),
        payload,
        created_at,
    };
    sqlx::query(
        r#"
        INSERT INTO collaboration_events (
            id, session_id, sequence, actor_user_id, participant_id, event_type, payload_json, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&event.id)
    .bind(&event.session_id)
    .bind(event.sequence)
    .bind(&event.actor_user_id)
    .bind(&event.participant_id)
    .bind(&event.event_type)
    .bind(to_json(&event.payload)?)
    .bind(&event.created_at)
    .execute(&state.pool)
    .await?;
    state
        .realtime
        .publish(
            &collaboration_channel_id(session_id),
            WsEvent::CollaborationEvent {
                event: event.clone(),
            },
        )
        .await;
    let _ = publish_collaboration_topology(state, session_id).await;
    Ok(event)
}

pub(super) async fn publish_collaboration_event(
    state: &SharedState,
    session_id: &str,
    actor_user_id: Option<String>,
    participant_id: Option<String>,
    event_type: &str,
    payload: Value,
) -> AppResult<CollaborationEvent> {
    let event = publish_collaboration_event_raw(
        state,
        session_id,
        actor_user_id.clone(),
        participant_id,
        event_type,
        payload,
    )
    .await?;
    let session = fetch_collaboration_session_by_id(&state.pool, session_id).await?;
    let _ = publish_current_creator_live_state(state, &session.host_creator_id).await;
    Ok(event)
}

pub(super) async fn publish_collaboration_reconciliation_event(
    state: &SharedState,
    session_id: &str,
    actor_user_id: Option<String>,
    participant_id: Option<String>,
    event_type: &str,
    payload: Value,
) -> AppResult<CollaborationEvent> {
    let event = publish_collaboration_event_raw(
        state,
        session_id,
        actor_user_id,
        participant_id,
        event_type,
        payload,
    )
    .await?;
    let session = fetch_collaboration_session_by_id(&state.pool, session_id).await?;
    let _ = publish_current_creator_live_state(state, &session.host_creator_id).await;
    Ok(event)
}

pub(super) fn collaboration_channel_id(session_id: &str) -> String {
    format!("collab:{session_id}")
}
