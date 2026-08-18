use super::*;
use axum::extract::ws::{Message, WebSocket};
use futures_util::sink::SinkExt;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum CollaborationSocketCommand {
    Heartbeat,
    RevokeInvite {
        invite_id: String,
    },
    RequestStateChange {
        state: String,
    },
    UpdateParticipant {
        participant_id: String,
        state: Option<String>,
        publish_to_host: Option<bool>,
        mirror_to_guest_channel: Option<bool>,
        can_speak_in_chat: Option<bool>,
    },
    RemoveParticipant {
        participant_id: String,
    },
    IssueMirrorGrant {
        participant_id: String,
    },
    RevokeMirrorGrants {
        participant_id: String,
    },
}

pub(crate) struct CollaborationSocketCommandOutcome {
    pub(crate) command_type: &'static str,
    pub(crate) participant_id: Option<String>,
    pub(crate) state: Option<String>,
}

pub(crate) fn collaboration_socket_command_name(value: &Value) -> String {
    value
        .get("type")
        .and_then(Value::as_str)
        .map(|command_type| command_type.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

pub(crate) async fn send_collaboration_command_accepted(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    command_type: &str,
    participant_id: Option<String>,
    state: Option<String>,
) -> bool {
    sender
        .send(Message::Text(
            serde_json::to_string(&WsEvent::CollaborationCommandAccepted {
                command_type: command_type.to_string(),
                participant_id,
                state,
            })
            .unwrap_or_default(),
        ))
        .await
        .is_ok()
}

pub(crate) async fn send_collaboration_command_rejected(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    command_type: &str,
    reason: impl Into<String>,
) -> bool {
    sender
        .send(Message::Text(
            serde_json::to_string(&WsEvent::CollaborationCommandRejected {
                command_type: command_type.to_string(),
                reason: reason.into(),
            })
            .unwrap_or_default(),
        ))
        .await
        .is_ok()
}

pub(crate) async fn execute_collaboration_socket_command(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    session: &CollaborationSessionView,
    command: CollaborationSocketCommand,
) -> AppResult<CollaborationSocketCommandOutcome> {
    match command {
        CollaborationSocketCommand::Heartbeat => Ok(CollaborationSocketCommandOutcome {
            command_type: "heartbeat",
            participant_id: None,
            state: None,
        }),
        CollaborationSocketCommand::RevokeInvite { invite_id } => {
            if session.participant.role != "host" {
                return Err(AppError::BadRequest(
                    "only the collaboration host can revoke invites over realtime control"
                        .to_string(),
                ));
            }
            let creator_id = identity.creator_id.as_deref().ok_or_else(|| {
                AppError::BadRequest(
                    "creator scope is required for host collaboration controls".to_string(),
                )
            })?;
            let host_session =
                fetch_collaboration_session_for_host(&state.pool, creator_id, session_id).await?;
            let invite = revoke_collaboration_invite_internal(
                state,
                &host_session,
                &invite_id,
                &identity.user_id,
                "host_revoked",
            )
            .await?;
            Ok(CollaborationSocketCommandOutcome {
                command_type: "revokeInvite",
                participant_id: None,
                state: Some(invite.state),
            })
        }
        CollaborationSocketCommand::RequestStateChange {
            state: requested_state,
        } => {
            if session.participant.role == "host" {
                return Err(AppError::BadRequest(
                    "host participants cannot request collaboration state changes".to_string(),
                ));
            }
            let current_participant =
                fetch_collaboration_participant_by_id(&state.pool, &session.participant.id).await?;
            validate_collaboration_participant_access(&current_participant)?;
            let current_session =
                fetch_collaboration_session_by_id(&state.pool, session_id).await?;
            if current_session.status == "ended" {
                return Err(AppError::BadRequest(
                    "cannot request collaboration state changes for an ended session".to_string(),
                ));
            }
            if requested_state != "backstage" && requested_state != "live" {
                return Err(AppError::BadRequest(
                    "requested collaboration state must be backstage or live".to_string(),
                ));
            }
            if current_participant.state == requested_state {
                return Err(AppError::BadRequest(
                    "participant is already in the requested collaboration state".to_string(),
                ));
            }
            let requested_at = Utc::now().to_rfc3339();
            publish_collaboration_event(
                state,
                session_id,
                Some(identity.user_id.clone()),
                Some(current_participant.id.clone()),
                "participant_state_requested",
                json!({
                    "participantId": current_participant.id,
                    "currentState": current_participant.state,
                    "requestedState": requested_state,
                    "requestedAt": requested_at,
                }),
            )
            .await?;
            Ok(CollaborationSocketCommandOutcome {
                command_type: "requestStateChange",
                participant_id: Some(current_participant.id),
                state: Some(requested_state),
            })
        }
        CollaborationSocketCommand::UpdateParticipant {
            participant_id,
            state: requested_state,
            publish_to_host,
            mirror_to_guest_channel,
            can_speak_in_chat,
        } => {
            if session.participant.role != "host" {
                return Err(AppError::BadRequest(
                    "only the collaboration host can update participants over realtime control"
                        .to_string(),
                ));
            }
            let creator_id = identity.creator_id.as_deref().ok_or_else(|| {
                AppError::BadRequest(
                    "creator scope is required for host collaboration controls".to_string(),
                )
            })?;
            let host_session =
                fetch_collaboration_session_for_host(&state.pool, creator_id, session_id).await?;
            let update = UpdateCollaborationParticipantRequest {
                state: requested_state,
                publish_to_host,
                mirror_to_guest_channel,
                can_speak_in_chat,
            };
            let updated = apply_collaboration_participant_update(
                state,
                &host_session,
                &participant_id,
                &identity.user_id,
                &update,
            )
            .await?;
            Ok(CollaborationSocketCommandOutcome {
                command_type: "updateParticipant",
                participant_id: Some(updated.id),
                state: Some(updated.state),
            })
        }
        CollaborationSocketCommand::RemoveParticipant { participant_id } => {
            if session.participant.role != "host" {
                return Err(AppError::BadRequest(
                    "only the collaboration host can remove participants over realtime control"
                        .to_string(),
                ));
            }
            let creator_id = identity.creator_id.as_deref().ok_or_else(|| {
                AppError::BadRequest(
                    "creator scope is required for host collaboration controls".to_string(),
                )
            })?;
            let host_session =
                fetch_collaboration_session_for_host(&state.pool, creator_id, session_id).await?;
            if host_session.status == "ended" {
                return Err(AppError::BadRequest(
                    "cannot remove participants from an ended collaboration session".to_string(),
                ));
            }
            let participant =
                fetch_collaboration_participant_by_id(&state.pool, &participant_id).await?;
            if participant.session_id != *session_id {
                return Err(AppError::NotFound);
            }
            if participant.role == "host" {
                return Err(AppError::BadRequest(
                    "the host cannot be removed from a collaboration session".to_string(),
                ));
            }
            if participant.state != "removed" {
                let now = Utc::now().to_rfc3339();
                sqlx::query(
                    r#"
                    UPDATE collaboration_participants
                    SET state = 'removed', left_at = COALESCE(left_at, ?), updated_at = ?
                    WHERE id = ? AND session_id = ?
                    "#,
                )
                .bind(&now)
                .bind(&now)
                .bind(&participant_id)
                .bind(session_id)
                .execute(&state.pool)
                .await?;
                revoke_collaboration_mirror_grants_for_participant(
                    state,
                    session_id,
                    &participant_id,
                    Some(identity.user_id.clone()),
                    &now,
                    "participant_removed",
                )
                .await?;
                publish_collaboration_event(
                    state,
                    session_id,
                    Some(identity.user_id.clone()),
                    Some(participant_id.clone()),
                    "participant_removed",
                    json!({
                        "participantId": participant_id,
                        "removedAt": now,
                    }),
                )
                .await?;
            }
            let updated =
                fetch_collaboration_participant_by_id(&state.pool, &participant_id).await?;
            Ok(CollaborationSocketCommandOutcome {
                command_type: "removeParticipant",
                participant_id: Some(updated.id),
                state: Some(updated.state),
            })
        }
        CollaborationSocketCommand::IssueMirrorGrant { participant_id } => {
            if session.participant.role != "host" {
                return Err(AppError::BadRequest(
                    "only the collaboration host can issue mirror grants over realtime control"
                        .to_string(),
                ));
            }
            let creator_id = identity.creator_id.as_deref().ok_or_else(|| {
                AppError::BadRequest(
                    "creator scope is required for host collaboration controls".to_string(),
                )
            })?;
            ensure_creator_collaboration_enabled(&state.pool, creator_id).await?;
            let host_session =
                fetch_collaboration_session_for_host(&state.pool, creator_id, session_id).await?;
            if host_session.status == "ended" {
                return Err(AppError::BadRequest(
                    "cannot issue collaboration grants for an ended session".to_string(),
                ));
            }
            let participant =
                fetch_collaboration_participant_by_id(&state.pool, &participant_id).await?;
            if participant.session_id != *session_id {
                return Err(AppError::NotFound);
            }
            if participant.state != "live" {
                return Err(AppError::BadRequest(
                    "mirror grants can only be issued for live participants".to_string(),
                ));
            }
            if !participant.mirror_to_guest_channel {
                return Err(AppError::BadRequest(
                    "participant is not enabled for mirrored guest channel pickup".to_string(),
                ));
            }
            if participant.creator_id.is_none() {
                return Err(AppError::BadRequest(
                    "participant must have a creator profile to receive a mirror grant".to_string(),
                ));
            }
            let grant = issue_mirror_grant_for_participant(
                state,
                &host_session,
                &participant,
                &identity.user_id,
            )
            .await?;
            Ok(CollaborationSocketCommandOutcome {
                command_type: "issueMirrorGrant",
                participant_id: Some(grant.participant_id),
                state: None,
            })
        }
        CollaborationSocketCommand::RevokeMirrorGrants { participant_id } => {
            if session.participant.role != "host" {
                return Err(AppError::BadRequest(
                    "only the collaboration host can revoke mirror grants over realtime control"
                        .to_string(),
                ));
            }
            let creator_id = identity.creator_id.as_deref().ok_or_else(|| {
                AppError::BadRequest(
                    "creator scope is required for host collaboration controls".to_string(),
                )
            })?;
            let host_session =
                fetch_collaboration_session_for_host(&state.pool, creator_id, session_id).await?;
            let participant =
                fetch_collaboration_participant_by_id(&state.pool, &participant_id).await?;
            if participant.session_id != host_session.id {
                return Err(AppError::NotFound);
            }
            let now = Utc::now().to_rfc3339();
            revoke_collaboration_mirror_grants_for_participant(
                state,
                session_id,
                &participant_id,
                Some(identity.user_id.clone()),
                &now,
                "host_revoked",
            )
            .await?;
            Ok(CollaborationSocketCommandOutcome {
                command_type: "revokeMirrorGrants",
                participant_id: Some(participant_id),
                state: None,
            })
        }
    }
}

pub(crate) fn validate_collaboration_socket_access(
    session: &CollaborationSessionView,
) -> AppResult<()> {
    if session.status == "ended" {
        return Err(AppError::Forbidden);
    }
    if matches!(session.participant.state.as_str(), "left" | "removed") {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

pub(crate) async fn fetch_current_collaboration_socket_session_view(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
) -> AppResult<CollaborationSessionView> {
    reconcile_collaboration_session_expiry_for_read(state, session_id).await?;
    if let Ok(session) =
        fetch_collaboration_session_for_participant(&state.pool, &identity.user_id, session_id)
            .await
    {
        validate_collaboration_socket_access(&session)?;
        return Ok(session);
    }

    let creator_id = identity.creator_id.as_deref().ok_or(AppError::Forbidden)?;
    let host_session =
        fetch_collaboration_session_for_host(&state.pool, creator_id, session_id).await?;
    let host = fetch_collaboration_host_summary(&state.pool, &host_session.host_creator_id).await?;
    let host_view = collaboration_session_view_for_host(host_session, host)?;
    validate_collaboration_socket_access(&host_view)?;
    Ok(host_view)
}

pub(crate) async fn reconcile_collaboration_session_expiry_for_read(
    state: &SharedState,
    session_id: &str,
) -> AppResult<()> {
    let _ = reconcile_single_collaboration_session(state.clone(), session_id).await?;
    Ok(())
}

pub(crate) async fn reconcile_collaboration_expiry_for_host_read(
    state: &SharedState,
    creator_id: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let cutoff = active_presence_cutoff();
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT session_id
        FROM (
            SELECT i.session_id AS session_id
            FROM collaboration_invites i
            JOIN collaboration_sessions s ON s.id = i.session_id
            WHERE s.host_creator_id = ?
              AND i.state = 'pending'
              AND i.expires_at <= ?
            UNION
            SELECT g.session_id AS session_id
            FROM collaboration_mirror_grants g
            JOIN collaboration_sessions s ON s.id = g.session_id
            WHERE s.host_creator_id = ?
              AND g.state IN ('issued', 'active')
              AND g.expires_at <= ?
            UNION
            SELECT css.collaboration_session_id AS session_id
            FROM collaboration_socket_sessions css
            JOIN collaboration_sessions s ON s.id = css.collaboration_session_id
            WHERE s.host_creator_id = ?
              AND css.disconnected_at IS NULL
              AND css.last_seen_at < ?
            UNION
            SELECT s.id AS session_id
            FROM collaboration_sessions s
            JOIN broadcasts b ON b.id = s.source_broadcast_id
            WHERE s.host_creator_id = ?
              AND s.status != 'ended'
              AND b.status NOT IN ('ready', 'live')
        )
        "#,
    )
    .bind(creator_id)
    .bind(&now)
    .bind(creator_id)
    .bind(&now)
    .bind(creator_id)
    .bind(&cutoff)
    .bind(creator_id)
    .fetch_all(&state.pool)
    .await?;
    for row in rows {
        let session_id: String = row.get("session_id");
        reconcile_collaboration_session_expiry_for_read(state, &session_id).await?;
    }
    Ok(())
}

pub(crate) async fn reconcile_collaboration_expiry_for_participant_read(
    state: &SharedState,
    user_id: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let cutoff = active_presence_cutoff();
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT session_id
        FROM (
            SELECT i.session_id AS session_id
            FROM collaboration_invites i
            WHERE i.invitee_user_id = ?
              AND i.state = 'pending'
              AND i.expires_at <= ?
            UNION
            SELECT g.session_id AS session_id
            FROM collaboration_mirror_grants g
            JOIN collaboration_participants p ON p.id = g.participant_id
            WHERE p.user_id = ?
              AND g.state IN ('issued', 'active')
              AND g.expires_at <= ?
            UNION
            SELECT css.collaboration_session_id AS session_id
            FROM collaboration_socket_sessions css
            WHERE css.user_id = ?
              AND css.disconnected_at IS NULL
              AND css.last_seen_at < ?
            UNION
            SELECT p.session_id AS session_id
            FROM collaboration_participants p
            JOIN collaboration_sessions s ON s.id = p.session_id
            JOIN broadcasts b ON b.id = s.source_broadcast_id
            WHERE p.user_id = ?
              AND s.status != 'ended'
              AND b.status NOT IN ('ready', 'live')
        )
        "#,
    )
    .bind(user_id)
    .bind(&now)
    .bind(user_id)
    .bind(&now)
    .bind(user_id)
    .bind(&cutoff)
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;
    for row in rows {
        let session_id: String = row.get("session_id");
        reconcile_collaboration_session_expiry_for_read(state, &session_id).await?;
    }
    Ok(())
}
