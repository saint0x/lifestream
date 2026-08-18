use super::*;

pub(super) async fn fetch_visible_collaboration_mirror_pickups_for_session_view(
    pool: &SqlitePool,
    session: &CollaborationSessionView,
) -> AppResult<Vec<CollaborationMirrorPickup>> {
    if session.participant.role == "host" {
        fetch_collaboration_mirror_pickups_for_session(pool, &session.id).await
    } else {
        fetch_collaboration_mirror_pickups_for_participant(pool, &session.participant.id).await
    }
}

pub(super) async fn fetch_visible_collaboration_mirror_grants_for_session_view(
    pool: &SqlitePool,
    session: &CollaborationSessionView,
) -> AppResult<Vec<CollaborationMirrorGrant>> {
    if session.participant.role == "host" {
        fetch_collaboration_mirror_grants_for_session(pool, &session.id).await
    } else {
        fetch_collaboration_mirror_grants_for_participant(pool, &session.participant.id).await
    }
}

async fn fetch_collaboration_socket_presence_for_session(
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

pub(super) async fn fetch_collaboration_socket_presence_by_id_raw(
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

pub(super) fn build_collaboration_runtime_topology(
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

pub(super) async fn build_collaboration_runtime_response_for_participant(
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

pub(super) async fn build_collaboration_runtime_response_for_host(
    pool: &SqlitePool,
    session: CollaborationSession,
) -> AppResult<CollaborationRuntimeResponse> {
    let host = fetch_collaboration_host_summary(pool, &session.host_creator_id).await?;
    let view = collaboration_session_view_for_host(session, host)?;
    build_collaboration_runtime_response_for_participant(pool, view).await
}

pub(super) async fn build_creator_collaboration_control_response_for_host(
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

pub(super) async fn fetch_creator_live_collaboration_summary(
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

pub(super) async fn publish_collaboration_topology(
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

pub(super) async fn publish_collaboration_presence(
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

pub(super) async fn expire_pending_collaboration_invites_for_session(
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

pub(super) async fn expire_collaboration_mirror_grants_for_session(
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

pub(super) async fn disconnect_stale_collaboration_socket_sessions_for_session(
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

pub(super) async fn reconcile_single_collaboration_socket_session(
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

pub(super) async fn reconcile_single_collaboration_session(
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
