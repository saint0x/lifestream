use super::topology::{
    build_collaboration_runtime_response_for_host,
    build_creator_collaboration_control_response_for_host,
};
use super::*;

pub(crate) async fn publish_collaboration_topology(
    state: &SharedState,
    session_id: &str,
) -> AppResult<()> {
    let session = fetch_collaboration_session_by_id(&state.pool, session_id).await?;
    let response = build_collaboration_runtime_response_for_host(&state.pool, session).await?;
    state
        .realtime
        .publish(
            &collaboration_channel_id(session_id),
            WsEvent::CollaborationTopology {
                topology: response.topology,
            },
        )
        .await;
    Ok(())
}

pub(crate) async fn publish_collaboration_presence(
    state: &SharedState,
    session_id: &str,
) -> AppResult<()> {
    let connected_participants =
        count_active_collaboration_socket_sessions(&state.pool, session_id).await?;
    state
        .realtime
        .publish(
            &collaboration_channel_id(session_id),
            WsEvent::CollaborationPresence {
                session_id: session_id.to_string(),
                connected_participants,
            },
        )
        .await;
    Ok(())
}

pub(crate) async fn expire_pending_collaboration_invites_for_session(
    state: &SharedState,
    session_id: &str,
    now: &str,
) -> AppResult<Vec<CollaborationReconciliationAction>> {
    let rows = sqlx::query(
        r#"
        SELECT id, invitee_user_id
        FROM collaboration_invites
        WHERE session_id = ? AND state = 'pending' AND expires_at <= ?
        "#,
    )
    .bind(session_id)
    .bind(now)
    .fetch_all(&state.pool)
    .await?;
    let mut actions = Vec::with_capacity(rows.len());
    for row in rows {
        let invite_id: String = row.get("id");
        let invitee_user_id: String = row.get("invitee_user_id");
        sqlx::query(
            "UPDATE collaboration_invites SET state = 'expired', responded_at = COALESCE(responded_at, ?) WHERE id = ?",
        )
        .bind(now)
        .bind(&invite_id)
        .execute(&state.pool)
        .await?;
        let _ = publish_collaboration_reconciliation_event(
            state,
            session_id,
            None,
            None,
            "invite_expired",
            json!({
                "inviteId": invite_id,
                "inviteeUserId": invitee_user_id,
                "expiredAt": now,
            }),
        )
        .await;
        actions.push(CollaborationReconciliationAction {
            action_type: "invite_expired".to_string(),
            target_id: invite_id,
            previous_state: Some("pending".to_string()),
            next_state: Some("expired".to_string()),
            reason: "pending collaboration invite exceeded its expiry window".to_string(),
            occurred_at: now.to_string(),
        });
    }
    Ok(actions)
}

pub(crate) async fn expire_collaboration_mirror_grants_for_session(
    state: &SharedState,
    session_id: &str,
    now: &str,
) -> AppResult<Vec<CollaborationReconciliationAction>> {
    let rows = sqlx::query(
        r#"
        SELECT id, participant_id, state
        FROM collaboration_mirror_grants
        WHERE session_id = ? AND state IN ('issued', 'active') AND expires_at <= ?
        "#,
    )
    .bind(session_id)
    .bind(now)
    .fetch_all(&state.pool)
    .await?;
    let mut actions = Vec::with_capacity(rows.len());
    for row in rows {
        let grant_id: String = row.get("id");
        let participant_id: String = row.get("participant_id");
        let previous_state: String = row.get("state");
        sqlx::query(
            "UPDATE collaboration_mirror_grants SET state = 'expired', revoked_at = COALESCE(revoked_at, ?) WHERE id = ?",
        )
        .bind(now)
        .bind(&grant_id)
        .execute(&state.pool)
        .await?;
        let _ = publish_collaboration_reconciliation_event(
            state,
            session_id,
            None,
            Some(participant_id),
            "mirror_grant_expired",
            json!({
                "grantId": grant_id,
                "expiredAt": now,
            }),
        )
        .await;
        actions.push(CollaborationReconciliationAction {
            action_type: "mirror_grant_expired".to_string(),
            target_id: grant_id,
            previous_state: Some(previous_state),
            next_state: Some("expired".to_string()),
            reason: "collaboration mirror grant exceeded its expiry window".to_string(),
            occurred_at: now.to_string(),
        });
    }
    if !actions.is_empty() {
        let expired_grants =
            fetch_collaboration_mirror_grants_for_session(&state.pool, session_id).await?;
        let affected = expired_grants
            .into_iter()
            .filter(|grant| grant.state == "expired" && grant.revoked_at.as_deref() == Some(now))
            .collect::<Vec<_>>();
        deactivate_collaboration_mirror_pickups_for_grants(state, &affected, "expired", now)
            .await?;
    }
    Ok(actions)
}

pub(crate) async fn disconnect_stale_collaboration_socket_sessions_for_session(
    state: &SharedState,
    session_id: &str,
    now: &str,
    cutoff: &str,
) -> AppResult<Vec<CollaborationReconciliationAction>> {
    let rows = sqlx::query(
        r#"
        SELECT id
        FROM collaboration_socket_sessions
        WHERE collaboration_session_id = ? AND disconnected_at IS NULL AND last_seen_at < ?
        "#,
    )
    .bind(session_id)
    .bind(cutoff)
    .fetch_all(&state.pool)
    .await?;
    let mut actions = Vec::with_capacity(rows.len());
    for row in rows {
        let socket_id: String = row.get("id");
        sqlx::query(
            "UPDATE collaboration_socket_sessions SET disconnected_at = COALESCE(disconnected_at, ?), last_seen_at = MIN(last_seen_at, ?) WHERE id = ?",
        )
        .bind(now)
        .bind(cutoff)
        .bind(&socket_id)
        .execute(&state.pool)
        .await?;
        actions.push(CollaborationReconciliationAction {
            action_type: "socket_disconnected".to_string(),
            target_id: socket_id,
            previous_state: Some("connected".to_string()),
            next_state: Some("disconnected".to_string()),
            reason: "collaboration socket session exceeded the active presence TTL".to_string(),
            occurred_at: now.to_string(),
        });
    }
    if !actions.is_empty() {
        publish_collaboration_presence(state, session_id).await?;
        publish_collaboration_topology(state, session_id).await?;
    }
    Ok(actions)
}

pub(crate) async fn reconcile_single_collaboration_socket_session(
    state: SharedState,
    session_id: &str,
    socket_id: &str,
) -> AppResult<CollaborationSocketPresenceReconciliationReport> {
    let before =
        fetch_collaboration_socket_presence_by_id_raw(&state.pool, session_id, socket_id).await?;
    let now = Utc::now().to_rfc3339();
    let cutoff = active_presence_cutoff();
    let mut actions = Vec::new();

    if before.disconnected_at.is_none() && before.last_seen_at < cutoff {
        let updated = sqlx::query(
            "UPDATE collaboration_socket_sessions SET disconnected_at = COALESCE(disconnected_at, ?), last_seen_at = MIN(last_seen_at, ?) WHERE id = ? AND collaboration_session_id = ? AND disconnected_at IS NULL",
        )
        .bind(&now)
        .bind(&cutoff)
        .bind(socket_id)
        .bind(session_id)
        .execute(&state.pool)
        .await?;
        if updated.rows_affected() > 0 {
            actions.push(CollaborationSocketPresenceReconciliationAction {
                action_type: "socket_disconnected".to_string(),
                target_id: socket_id.to_string(),
                previous_state: Some("connected".to_string()),
                next_state: Some("disconnected".to_string()),
                reason: "collaboration socket session exceeded the active presence TTL".to_string(),
                occurred_at: now.clone(),
            });
        }
    }

    let socket_session =
        fetch_collaboration_socket_presence_by_id_raw(&state.pool, session_id, socket_id).await?;
    if !actions.is_empty() {
        publish_collaboration_presence(&state, session_id).await?;
        publish_collaboration_topology(&state, session_id).await?;
    }
    Ok(CollaborationSocketPresenceReconciliationReport {
        session_id: session_id.to_string(),
        socket_session_id: socket_id.to_string(),
        reconciled_at: now,
        actions,
        socket_session,
    })
}

pub(crate) async fn reconcile_single_collaboration_session(
    state: SharedState,
    session_id: &str,
) -> AppResult<CollaborationReconciliationReport> {
    let now = Utc::now().to_rfc3339();
    let cutoff = active_presence_cutoff();
    let mut actions = Vec::new();
    actions
        .extend(expire_pending_collaboration_invites_for_session(&state, session_id, &now).await?);
    actions.extend(expire_collaboration_mirror_grants_for_session(&state, session_id, &now).await?);
    actions.extend(
        disconnect_stale_collaboration_socket_sessions_for_session(
            &state, session_id, &now, &cutoff,
        )
        .await?,
    );
    let mut session = fetch_collaboration_session_by_id(&state.pool, session_id).await?;
    if session.status != "ended" {
        let source_broadcast = fetch_broadcast_by_id(
            &state.pool,
            &session.host_creator_id,
            &session.source_broadcast_id,
        )
        .await?;
        if source_broadcast.status != "ready" && source_broadcast.status != "live" {
            session = end_collaboration_session_internal_raw(
                &state,
                &session,
                None,
                json!({
                    "reason": "source broadcast is no longer active",
                    "broadcastId": session.source_broadcast_id,
                    "broadcastStatus": source_broadcast.status,
                }),
            )
            .await?;
            actions.push(CollaborationReconciliationAction {
                action_type: "session_ended".to_string(),
                target_id: session.id.clone(),
                previous_state: Some("active".to_string()),
                next_state: Some("ended".to_string()),
                reason: "source broadcast is no longer active".to_string(),
                occurred_at: now.clone(),
            });
        }
    }
    let control =
        build_creator_collaboration_control_response_for_host(&state.pool, session).await?;
    Ok(CollaborationReconciliationReport {
        session_id: session_id.to_string(),
        reconciled_at: now,
        actions,
        control,
    })
}
