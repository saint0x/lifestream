use super::discovery::fetch_user;
use super::moderation::{
    can_bypass_live_chat_restrictions, fetch_active_live_moderation_action,
    fetch_live_stream_owner_creator_id,
};
use super::*;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::Deserialize;
use serde_json::Value;

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/ws/live/:stream_id", get(ws_live))
        .route("/ws/creator/live", get(ws_creator_live))
        .route("/ws/live/collabs/:session_id", get(ws_collaboration))
}

#[derive(Deserialize)]
struct WsAuthQuery {
    #[serde(alias = "accessToken")]
    access_token: Option<String>,
    #[serde(alias = "afterSeq")]
    after_seq: Option<i64>,
    #[serde(alias = "sessionToken")]
    session_token: Option<String>,
}

async fn ws_live(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<WsAuthQuery>,
    Path(stream_id): Path<String>,
) -> AppResult<impl axum::response::IntoResponse> {
    validate_request_origin(&state, &headers)?;
    ensure_stream_exists(&state.pool, &stream_id).await?;
    let viewer_identity = match query.access_token {
        Some(token) => Some(lookup_identity(&state.pool, &token).await?),
        None => None,
    };
    Ok(ws.on_upgrade(move |socket| {
        handle_socket(
            socket,
            state,
            stream_id,
            viewer_identity,
            query.after_seq.unwrap_or(0),
            query.session_token,
        )
    }))
}

async fn ws_creator_live(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<WsAuthQuery>,
) -> AppResult<impl axum::response::IntoResponse> {
    validate_request_origin(&state, &headers)?;
    let token = query.access_token.ok_or(AppError::Unauthorized)?;
    let identity = lookup_identity(&state.pool, &token).await?;
    let creator_id = identity.creator_id.clone().ok_or(AppError::Forbidden)?;
    Ok(ws.on_upgrade(move |socket| {
        handle_creator_live_socket(socket, state, creator_id, identity, query.session_token)
    }))
}

async fn ws_collaboration(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<WsAuthQuery>,
    Path(session_id): Path<String>,
) -> AppResult<impl axum::response::IntoResponse> {
    validate_request_origin(&state, &headers)?;
    let token = query.access_token.ok_or(AppError::Unauthorized)?;
    let identity = lookup_identity(&state.pool, &token).await?;
    let participant_access =
        fetch_collaboration_session_for_participant(&state.pool, &identity.user_id, &session_id)
            .await;
    if participant_access.is_err() {
        let creator_id = identity.creator_id.as_deref().ok_or(AppError::Forbidden)?;
        let host_session =
            fetch_collaboration_session_for_host(&state.pool, creator_id, &session_id).await?;
        let host =
            fetch_collaboration_host_summary(&state.pool, &host_session.host_creator_id).await?;
        let host_view = collaboration_session_view_for_host(host_session, host)?;
        validate_collaboration_socket_access(&host_view)?;
    } else if let Ok(session) = &participant_access {
        validate_collaboration_socket_access(session)?;
    }
    Ok(ws.on_upgrade(move |socket| {
        handle_collaboration_socket(
            socket,
            state,
            session_id,
            identity,
            query.after_seq.unwrap_or(0),
            query.session_token,
        )
    }))
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(super) enum CollaborationSocketCommand {
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

pub(super) struct CollaborationSocketCommandOutcome {
    pub(super) command_type: &'static str,
    pub(super) participant_id: Option<String>,
    pub(super) state: Option<String>,
}

fn collaboration_socket_command_name(value: &Value) -> String {
    value
        .get("type")
        .and_then(Value::as_str)
        .map(|command_type| command_type.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

async fn send_collaboration_command_accepted(
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

async fn send_collaboration_command_rejected(
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

pub(super) async fn execute_collaboration_socket_command(
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

async fn close_websocket(sender: &mut futures_util::stream::SplitSink<WebSocket, Message>) {
    let _ = sender.send(Message::Close(None)).await;
}

async fn send_chat_message_rejected(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    reason: impl Into<String>,
) -> bool {
    sender
        .send(Message::Text(
            serde_json::to_string(&WsEvent::ChatMessageRejected {
                reason: reason.into(),
            })
            .unwrap_or_default(),
        ))
        .await
        .is_ok()
}

fn validate_collaboration_socket_access(session: &CollaborationSessionView) -> AppResult<()> {
    if session.status == "ended" {
        return Err(AppError::Forbidden);
    }
    if matches!(session.participant.state.as_str(), "left" | "removed") {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

pub(super) async fn fetch_current_collaboration_socket_session_view(
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

pub(super) async fn reconcile_collaboration_session_expiry_for_read(
    state: &SharedState,
    session_id: &str,
) -> AppResult<()> {
    let _ = reconcile_single_collaboration_session(state.clone(), session_id).await?;
    Ok(())
}

pub(super) async fn reconcile_collaboration_expiry_for_host_read(
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

pub(super) async fn reconcile_collaboration_expiry_for_participant_read(
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

async fn ensure_identity_session_active(
    pool: &SqlitePool,
    identity: &RequestIdentity,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS count
        FROM auth_sessions
        WHERE id = ?
          AND user_id = ?
          AND revoked_at IS NULL
          AND (expires_at IS NULL OR expires_at > ?)
        "#,
    )
    .bind(&identity.session_id)
    .bind(&identity.user_id)
    .bind(&now)
    .fetch_one(pool)
    .await?;
    let count: i64 = row.get("count");
    if count == 0 {
        return Err(AppError::Unauthorized);
    }
    Ok(())
}

pub(super) fn auth_session_channel_id(session_id: &str) -> String {
    format!("auth-session:{session_id}")
}

async fn handle_socket(
    socket: WebSocket,
    state: SharedState,
    stream_id: String,
    viewer_identity: Option<RequestIdentity>,
    after_seq: i64,
    session_token: Option<String>,
) {
    let (mut sender, mut receiver) = socket.split();
    if let Some(identity) = viewer_identity.as_ref() {
        if ensure_identity_session_active(&state.pool, identity)
            .await
            .is_err()
        {
            return;
        }
    }
    let (presence_session_token, resumed, last_seen_at) = match register_live_viewer_session(
        &state.pool,
        &stream_id,
        viewer_identity.as_ref(),
        session_token.as_deref(),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => return,
    };
    let history = fetch_chat_messages_for_viewer(
        &state.pool,
        &stream_id,
        viewer_identity
            .as_ref()
            .map(|identity| identity.user_id.as_str()),
        50,
        None,
    )
    .await
    .unwrap_or_default();
    let replay = fetch_chat_messages_for_viewer(
        &state.pool,
        &stream_id,
        viewer_identity
            .as_ref()
            .map(|identity| identity.user_id.as_str()),
        200,
        Some(after_seq.max(0)),
    )
    .await
    .unwrap_or_default();
    let active_moderation_action = match viewer_identity.as_ref() {
        Some(identity) => {
            fetch_active_live_moderation_action(&state.pool, &stream_id, &identity.user_id)
                .await
                .unwrap_or_default()
        }
        None => None,
    };
    let total_viewers = effective_live_viewer_count(&state.pool, &stream_id)
        .await
        .unwrap_or_default();
    let channel_id = stream_channel_id(&stream_id);
    let (mut subscription, _) = state.realtime.join(&channel_id).await;
    let mut auth_subscription = match viewer_identity.as_ref() {
        Some(identity) => {
            let (receiver, _) = state
                .realtime
                .join(&auth_session_channel_id(&identity.session_id))
                .await;
            Some(receiver)
        }
        None => None,
    };
    let mut touch_interval = interval(Duration::from_secs(WS_PRESENCE_TOUCH_INTERVAL_SECONDS));

    let _ = sender
        .send(Message::Text(
            serde_json::to_string(&WsEvent::SessionReady {
                channel: channel_id.clone(),
                session_token: presence_session_token.clone(),
                resumed,
                last_seen_at: last_seen_at.clone(),
            })
            .unwrap_or_default(),
        ))
        .await;
    if let Some(action) = active_moderation_action {
        let _ = sender
            .send(Message::Text(
                serde_json::to_string(&WsEvent::ModerationAction { action }).unwrap_or_default(),
            ))
            .await;
    }
    let _ = sender
        .send(Message::Text(if after_seq > 0 {
            serde_json::to_string(&WsEvent::ChatReplay {
                after_seq,
                messages: replay,
            })
            .unwrap_or_else(|_| {
                "{\"type\":\"system\",\"body\":\"serialization failed\"}".to_string()
            })
        } else {
            serde_json::to_string(&WsEvent::ChatHistory { messages: history }).unwrap_or_else(
                |_| "{\"type\":\"system\",\"body\":\"serialization failed\"}".to_string(),
            )
        }))
        .await;
    let _ = sender
        .send(Message::Text(
            serde_json::to_string(&WsEvent::ViewerCount {
                viewer_count: total_viewers,
            })
            .unwrap_or_default(),
        ))
        .await;

    state
        .realtime
        .publish(
            &channel_id,
            WsEvent::ViewerCount {
                viewer_count: total_viewers,
            },
        )
        .await;

    loop {
        tokio::select! {
            _ = touch_interval.tick() => {
                if let Some(identity) = viewer_identity.as_ref() {
                    if ensure_identity_session_active(&state.pool, identity).await.is_err() {
                        close_websocket(&mut sender).await;
                        break;
                    }
                }
                let _ = touch_live_viewer_session(
                    &state.pool,
                    &stream_id,
                    &presence_session_token,
                    &last_seen_at,
                )
                .await;
            }
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if let Some(identity) = viewer_identity.as_ref() {
                            if ensure_identity_session_active(&state.pool, identity).await.is_err() {
                                close_websocket(&mut sender).await;
                                break;
                            }
                        }
                        let _ = touch_live_viewer_session(
                            &state.pool,
                            &stream_id,
                            &presence_session_token,
                            &last_seen_at,
                        )
                        .await;
                        if let (Some(identity), Ok(input)) = (&viewer_identity, serde_json::from_str::<ChatInput>(&text)) {
                            match persist_chat_message(&state, &stream_id, identity, input).await {
                                Ok(persisted) => {
                                    if persisted.hidden_by_moderation {
                                        let _ = sender
                                            .send(Message::Text(
                                                serde_json::to_string(&WsEvent::ChatMessage {
                                                    message: persisted.message,
                                                })
                                                .unwrap_or_default(),
                                            ))
                                            .await;
                                    }
                                }
                                Err(AppError::BadRequest(reason))
                                | Err(AppError::PaymentRequired(reason)) => {
                                    if !send_chat_message_rejected(&mut sender, reason).await {
                                        break;
                                    }
                                }
                                Err(AppError::Forbidden) => {
                                    if !send_chat_message_rejected(
                                        &mut sender,
                                        "chat access is currently restricted",
                                    )
                                    .await
                                    {
                                        break;
                                    }
                                }
                                Err(_) => {}
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            outbound = subscription.recv() => {
                match outbound {
                    Ok(event) => {
                        if matches!(event, WsEvent::SessionInvalidated { .. }) {
                            close_websocket(&mut sender).await;
                            break;
                        }
                        if let Some(identity) = viewer_identity.as_ref() {
                            if ensure_identity_session_active(&state.pool, identity).await.is_err() {
                                close_websocket(&mut sender).await;
                                break;
                            }
                        }
                        if sender
                            .send(Message::Text(serde_json::to_string(&event).unwrap_or_default()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            revoked = async { auth_subscription.as_mut().unwrap().recv().await }, if auth_subscription.is_some() => {
                match revoked {
                    Ok(WsEvent::SessionInvalidated { .. }) => {
                        close_websocket(&mut sender).await;
                        break;
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        }
    }

    let _ = disconnect_live_viewer_session(
        &state.pool,
        &stream_id,
        &presence_session_token,
        &last_seen_at,
    )
    .await;
    state.realtime.leave(&channel_id).await;
    if let Some(identity) = viewer_identity.as_ref() {
        state
            .realtime
            .leave(&auth_session_channel_id(&identity.session_id))
            .await;
    }
    let remaining = effective_live_viewer_count(&state.pool, &stream_id)
        .await
        .unwrap_or_default();
    state
        .realtime
        .publish(
            &channel_id,
            WsEvent::ViewerCount {
                viewer_count: remaining,
            },
        )
        .await;
}

async fn handle_collaboration_socket(
    socket: WebSocket,
    state: SharedState,
    session_id: String,
    identity: RequestIdentity,
    after_seq: i64,
    session_token: Option<String>,
) {
    let channel_id = collaboration_channel_id(&session_id);
    let (mut sender, mut receiver) = socket.split();
    if ensure_identity_session_active(&state.pool, &identity)
        .await
        .is_err()
    {
        return;
    }
    let mut session =
        match fetch_current_collaboration_socket_session_view(&state, &session_id, &identity).await
        {
            Ok(session) => session,
            Err(_) => return,
        };
    let (presence_session_token, resumed, last_seen_at) =
        match register_collaboration_socket_session(
            &state.pool,
            &session,
            &identity,
            session_token.as_deref(),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => return,
        };
    let session_grants = fetch_collaboration_mirror_grants_for_session(&state.pool, &session_id)
        .await
        .unwrap_or_default();
    let session_pickups = fetch_collaboration_mirror_pickups_for_session(&state.pool, &session_id)
        .await
        .unwrap_or_default();
    let visible_grants =
        fetch_visible_collaboration_mirror_grants_for_session_view(&state.pool, &session)
            .await
            .unwrap_or_default();
    let visible_pickups =
        fetch_visible_collaboration_mirror_pickups_for_session_view(&state.pool, &session)
            .await
            .unwrap_or_default();
    let (snapshot_events, replay_events) =
        load_collaboration_socket_event_bootstrap(&state.pool, &session_id, after_seq)
            .await
            .unwrap_or_default();
    let (mut subscription, _) = state.realtime.join(&channel_id).await;
    let (mut auth_subscription, _) = state
        .realtime
        .join(&auth_session_channel_id(&identity.session_id))
        .await;
    let connected_participants =
        count_active_collaboration_socket_sessions(&state.pool, &session_id)
            .await
            .unwrap_or_default();
    let topology = build_collaboration_runtime_topology(
        &session,
        &session_grants,
        &session_pickups,
        connected_participants,
    );
    let visible_snapshot_events =
        filter_visible_collaboration_events_for_session(&session, snapshot_events);
    let visible_replay_events =
        filter_visible_collaboration_events_for_session(&session, replay_events);
    let mut touch_interval = interval(Duration::from_secs(WS_PRESENCE_TOUCH_INTERVAL_SECONDS));

    let _ = sender
        .send(Message::Text(
            serde_json::to_string(&WsEvent::SessionReady {
                channel: channel_id.clone(),
                session_token: presence_session_token.clone(),
                resumed,
                last_seen_at: last_seen_at.clone(),
            })
            .unwrap_or_default(),
        ))
        .await;
    let _ = sender
        .send(Message::Text(
            serde_json::to_string(&WsEvent::CollaborationSnapshot {
                session: session.clone(),
                grants: visible_grants,
                pickups: visible_pickups,
                events: visible_snapshot_events,
            })
            .unwrap_or_default(),
        ))
        .await;
    if after_seq > 0 {
        let _ = sender
            .send(Message::Text(
                serde_json::to_string(&WsEvent::CollaborationReplay {
                    after_seq,
                    events: visible_replay_events,
                })
                .unwrap_or_default(),
            ))
            .await;
    }
    let _ = sender
        .send(Message::Text(
            serde_json::to_string(&WsEvent::CollaborationTopology { topology }).unwrap_or_default(),
        ))
        .await;
    state
        .realtime
        .publish(
            &channel_id,
            WsEvent::CollaborationPresence {
                session_id: session_id.clone(),
                connected_participants: connected_participants as i64,
            },
        )
        .await;
    let _ = publish_collaboration_topology(&state, &session_id).await;

    loop {
        tokio::select! {
            _ = touch_interval.tick() => {
                if ensure_identity_session_active(&state.pool, &identity).await.is_err() {
                    close_websocket(&mut sender).await;
                    break;
                }
                match fetch_current_collaboration_socket_session_view(&state, &session_id, &identity).await {
                    Ok(current_session) => {
                        session = current_session;
                    }
                    Err(_) => break,
                }
                let _ = touch_collaboration_socket_session(
                    &state.pool,
                    &session_id,
                    &presence_session_token,
                    &last_seen_at,
                )
                .await;
            }
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if ensure_identity_session_active(&state.pool, &identity).await.is_err() {
                            close_websocket(&mut sender).await;
                            break;
                        }
                        match fetch_current_collaboration_socket_session_view(&state, &session_id, &identity).await {
                            Ok(current_session) => {
                                session = current_session;
                            }
                            Err(_) => break,
                        }
                        let _ = touch_collaboration_socket_session(
                            &state.pool,
                            &session_id,
                            &presence_session_token,
                            &last_seen_at,
                        )
                        .await;
                        let command_value = match serde_json::from_str::<Value>(&text) {
                            Ok(value) => value,
                            Err(_) => {
                                if !send_collaboration_command_rejected(
                                    &mut sender,
                                    "unknown",
                                    "invalid collaboration command payload",
                                ).await {
                                    break;
                                }
                                continue;
                            }
                        };
                        let command_type = collaboration_socket_command_name(&command_value);
                        let command = match serde_json::from_value::<CollaborationSocketCommand>(command_value) {
                            Ok(command) => command,
                            Err(_) => {
                                if !send_collaboration_command_rejected(
                                    &mut sender,
                                    &command_type,
                                    "unsupported collaboration command",
                                ).await {
                                    break;
                                }
                                continue;
                            }
                        };

                        match execute_collaboration_socket_command(
                            &state,
                            &session_id,
                            &identity,
                            &session,
                            command,
                        )
                        .await
                        {
                            Ok(outcome) if outcome.command_type == "heartbeat" => {
                                if sender.send(Message::Text(
                                    serde_json::to_string(&WsEvent::CollaborationHeartbeat {
                                        session_id: session_id.clone(),
                                        received_at: Utc::now().to_rfc3339(),
                                    }).unwrap_or_default()
                                )).await.is_err() {
                                    break;
                                }
                            }
                            Ok(outcome) => {
                                if !send_collaboration_command_accepted(
                                    &mut sender,
                                    outcome.command_type,
                                    outcome.participant_id,
                                    outcome.state,
                                ).await {
                                    break;
                                }
                            }
                            Err(error) => {
                                if !send_collaboration_command_rejected(
                                    &mut sender,
                                    &command_type,
                                    error.to_string(),
                                ).await {
                                    break;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            outbound = subscription.recv() => {
                match outbound {
                    Ok(event) => {
                        if matches!(event, WsEvent::SessionInvalidated { .. }) {
                            close_websocket(&mut sender).await;
                            break;
                        }
                        if ensure_identity_session_active(&state.pool, &identity).await.is_err() {
                            close_websocket(&mut sender).await;
                            break;
                        }
                        match fetch_current_collaboration_socket_session_view(&state, &session_id, &identity).await {
                            Ok(current_session) => {
                                session = current_session;
                            }
                            Err(_) => break,
                        }
                        if let WsEvent::CollaborationEvent { event: collaboration_event } = &event {
                            if !collaboration_event_is_visible_to_session(&session, collaboration_event) {
                                continue;
                            }
                        }
                        if sender.send(Message::Text(serde_json::to_string(&event).unwrap_or_default())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            revoked = auth_subscription.recv() => {
                match revoked {
                    Ok(WsEvent::SessionInvalidated { .. }) => {
                        close_websocket(&mut sender).await;
                        break;
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        }
    }

    let _ = disconnect_collaboration_socket_session(
        &state.pool,
        &session_id,
        &presence_session_token,
        &last_seen_at,
    )
    .await;
    state.realtime.leave(&channel_id).await;
    state
        .realtime
        .leave(&auth_session_channel_id(&identity.session_id))
        .await;
    let remaining = count_active_collaboration_socket_sessions(&state.pool, &session_id)
        .await
        .unwrap_or_default();
    state
        .realtime
        .publish(
            &channel_id,
            WsEvent::CollaborationPresence {
                session_id,
                connected_participants: remaining,
            },
        )
        .await;
    let _ = publish_collaboration_topology(&state, &session.id).await;
}

async fn handle_creator_live_socket(
    socket: WebSocket,
    state: SharedState,
    creator_id: String,
    identity: RequestIdentity,
    session_token: Option<String>,
) {
    let channel_id = creator_live_channel_id(&creator_id);
    let (mut sender, mut receiver) = socket.split();
    if ensure_identity_session_active(&state.pool, &identity)
        .await
        .is_err()
    {
        return;
    }
    let (mut subscription, _) = state.realtime.join(&channel_id).await;
    let (mut auth_subscription, _) = state
        .realtime
        .join(&auth_session_channel_id(&identity.session_id))
        .await;
    let (presence_session_token, resumed, ready_at) = match register_creator_live_socket_session(
        &state.pool,
        &creator_id,
        &identity.user_id,
        session_token.as_deref(),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => {
            state.realtime.leave(&channel_id).await;
            return;
        }
    };

    let live_control =
        match fetch_authoritative_creator_live_control_response(&state, &creator_id).await {
            Ok(control) => control,
            Err(_) => {
                let _ = disconnect_creator_live_socket_session(
                    &state.pool,
                    &creator_id,
                    &presence_session_token,
                    &ready_at,
                )
                .await;
                state.realtime.leave(&channel_id).await;
                return;
            }
        };
    let live_runtime =
        match fetch_authoritative_creator_live_runtime_response(&state, &creator_id).await {
            Ok(runtime) => runtime,
            Err(_) => {
                let _ = disconnect_creator_live_socket_session(
                    &state.pool,
                    &creator_id,
                    &presence_session_token,
                    &ready_at,
                )
                .await;
                state.realtime.leave(&channel_id).await;
                return;
            }
        };
    let mut touch_interval = interval(Duration::from_secs(WS_PRESENCE_TOUCH_INTERVAL_SECONDS));

    let _ = sender
        .send(Message::Text(
            serde_json::to_string(&WsEvent::SessionReady {
                channel: channel_id.clone(),
                session_token: presence_session_token.clone(),
                resumed,
                last_seen_at: ready_at.clone(),
            })
            .unwrap_or_default(),
        ))
        .await;
    let _ = sender
        .send(Message::Text(
            serde_json::to_string(&WsEvent::CreatorLiveState {
                control: live_control,
                runtime: live_runtime,
            })
            .unwrap_or_default(),
        ))
        .await;

    loop {
        tokio::select! {
            _ = touch_interval.tick() => {
                if ensure_identity_session_active(&state.pool, &identity).await.is_err() {
                    close_websocket(&mut sender).await;
                    break;
                }
                let _ = touch_creator_live_socket_session(
                    &state.pool,
                    &creator_id,
                    &presence_session_token,
                    &ready_at,
                )
                .await;
            }
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Text(_))) => {
                        if ensure_identity_session_active(&state.pool, &identity).await.is_err() {
                            close_websocket(&mut sender).await;
                            break;
                        }
                        let _ = touch_creator_live_socket_session(
                            &state.pool,
                            &creator_id,
                            &presence_session_token,
                            &ready_at,
                        )
                        .await;
                        if identity.user_id.is_empty() {
                            break;
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            outbound = subscription.recv() => {
                match outbound {
                    Ok(event) => {
                        if matches!(event, WsEvent::SessionInvalidated { .. }) {
                            close_websocket(&mut sender).await;
                            break;
                        }
                        if ensure_identity_session_active(&state.pool, &identity).await.is_err() {
                            close_websocket(&mut sender).await;
                            break;
                        }
                        if sender
                            .send(Message::Text(serde_json::to_string(&event).unwrap_or_default()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            revoked = auth_subscription.recv() => {
                match revoked {
                    Ok(WsEvent::SessionInvalidated { .. }) => {
                        close_websocket(&mut sender).await;
                        break;
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        }
    }

    let _ = disconnect_creator_live_socket_session(
        &state.pool,
        &creator_id,
        &presence_session_token,
        &ready_at,
    )
    .await;
    state.realtime.leave(&channel_id).await;
    state
        .realtime
        .leave(&auth_session_channel_id(&identity.session_id))
        .await;
}

pub(super) async fn persist_chat_message(
    state: &SharedState,
    stream_id: &str,
    identity: &RequestIdentity,
    input: ChatInput,
) -> AppResult<PersistedChatMessage> {
    enforce_rate_limit(
        state,
        &format!("chat:{}:{}", stream_id, identity.user_id),
        20,
        Duration::from_secs(10),
    )
    .await?;
    ensure_stream_exists(&state.pool, stream_id).await?;
    let body = input.body.trim();
    if body.is_empty() {
        return Err(AppError::BadRequest("message body is required".to_string()));
    }
    if body.len() > 500 {
        return Err(AppError::BadRequest(
            "message body must be 500 characters or fewer".to_string(),
        ));
    }

    let stream_creator_id = fetch_live_stream_owner_creator_id(&state.pool, stream_id).await?;
    enforce_collaboration_chat_participation_permissions(
        &state.pool,
        stream_id,
        &stream_creator_id,
        &identity.user_id,
    )
    .await?;
    let stream_settings = fetch_creator_live_settings(&state.pool, &stream_creator_id).await?;
    let bypass_restrictions =
        can_bypass_live_chat_restrictions(&state.pool, &stream_creator_id, identity).await?;
    let moderation_action =
        fetch_active_live_moderation_action(&state.pool, stream_id, &identity.user_id).await?;
    if let Some(action) = moderation_action.as_ref() {
        match action.action_type.as_str() {
            "ban" | "mute" => {
                return Err(AppError::Forbidden);
            }
            _ => {}
        }
    }

    if !bypass_restrictions {
        if stream_settings.subscriber_only
            && !fetch_active_creator_membership(
                &state.pool,
                &identity.user_id,
                &stream_creator_id,
                None,
            )
            .await?
        {
            return Err(AppError::PaymentRequired(
                "subscriber-only chat requires an active creator membership".to_string(),
            ));
        }

        if stream_settings.slow_mode_seconds > 0 {
            enforce_live_chat_slow_mode(
                &state.pool,
                stream_id,
                &identity.user_id,
                stream_settings.slow_mode_seconds,
            )
            .await?;
        }

        if let Some(reason) =
            detect_live_chat_automod_violation(&stream_settings.auto_mod_level, body)
        {
            return Err(AppError::BadRequest(format!(
                "message rejected by automod: {reason}"
            )));
        }
    }

    let user = fetch_user(&state.pool, &identity.user_id).await?;
    let mut badges = Vec::new();
    if fetch_active_creator_membership(&state.pool, &identity.user_id, &stream_creator_id, None)
        .await?
    {
        badges.push("subscriber".to_string());
    }
    if identity.creator_id.is_some() {
        badges.push("partner".to_string());
    }

    let message = ChatMessage {
        id: Uuid::new_v4().to_string(),
        sequence: next_chat_message_sequence(&state.pool, stream_id).await?,
        user_handle: user.handle,
        display_name: user.display_name,
        color: input.color.unwrap_or_else(|| "#fafafa".to_string()),
        badges,
        body: body.to_string(),
        sent_at: Utc::now().to_rfc3339(),
    };
    let hidden_by_moderation = matches!(
        moderation_action,
        Some(action) if action.action_type == "shadowban"
    );

    sqlx::query(
        "INSERT INTO chat_messages (id, stream_id, user_id, creator_id, user_handle, display_name, color, badges_json, body, sent_at, hidden_by_moderation, sequence) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&message.id)
    .bind(stream_id)
    .bind(&identity.user_id)
    .bind(identity.creator_id.as_deref())
    .bind(&message.user_handle)
    .bind(&message.display_name)
    .bind(&message.color)
    .bind(to_json(&message.badges)?)
    .bind(&message.body)
    .bind(&message.sent_at)
    .bind(hidden_by_moderation as i64)
    .bind(message.sequence)
    .execute(&state.pool)
    .await?;

    if !hidden_by_moderation {
        state
            .realtime
            .publish(
                &stream_channel_id(stream_id),
                WsEvent::ChatMessage {
                    message: message.clone(),
                },
            )
            .await;
    }

    Ok(PersistedChatMessage {
        message,
        hidden_by_moderation,
    })
}

async fn enforce_collaboration_chat_participation_permissions(
    pool: &SqlitePool,
    _stream_id: &str,
    creator_id: &str,
    user_id: &str,
) -> AppResult<()> {
    let profile = fetch_creator_profile(pool, creator_id).await?;
    let Some(current_broadcast_id) = profile.current_broadcast_id.as_deref() else {
        return Ok(());
    };
    let Some(session) =
        fetch_active_collaboration_session_for_broadcast(pool, current_broadcast_id).await?
    else {
        return Ok(());
    };
    if session.status != "active" || session.chat_mode != "shared" {
        return Ok(());
    }
    let participant =
        match fetch_collaboration_participant_for_user(pool, &session.id, user_id).await {
            Ok(participant) => participant,
            Err(AppError::NotFound) => return Ok(()),
            Err(error) => return Err(error),
        };
    if matches!(participant.state.as_str(), "left" | "removed") {
        return Ok(());
    }
    if participant.can_speak_in_chat {
        return Ok(());
    }
    Err(AppError::Forbidden)
}

async fn enforce_live_chat_slow_mode(
    pool: &SqlitePool,
    stream_id: &str,
    user_id: &str,
    slow_mode_seconds: i64,
) -> AppResult<()> {
    let row = sqlx::query(
        "SELECT sent_at FROM chat_messages WHERE stream_id = ? AND user_id = ? ORDER BY sent_at DESC LIMIT 1",
    )
    .bind(stream_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(());
    };

    let last_sent_at = chrono::DateTime::parse_from_rfc3339(&row.get::<String, _>("sent_at"))
        .map_err(|_| AppError::BadRequest("invalid chat timestamp".to_string()))?
        .with_timezone(&Utc);
    let next_allowed_at = last_sent_at + ChronoDuration::seconds(slow_mode_seconds);
    if Utc::now() < next_allowed_at {
        return Err(AppError::BadRequest(format!(
            "slow mode is active; wait {} seconds before sending another message",
            slow_mode_seconds
        )));
    }

    Ok(())
}

fn detect_live_chat_automod_violation(level: &str, body: &str) -> Option<&'static str> {
    if level == "off" {
        return None;
    }

    let trimmed = body.trim();
    let lowercase = trimmed.to_lowercase();
    if contains_blocked_invite_or_link(&lowercase) {
        return Some("links and invite spam are blocked");
    }
    if contains_repeated_spam_pattern(&lowercase) {
        return Some("repetitive spam is blocked");
    }

    if level == "strict" {
        if is_excessive_caps(trimmed) {
            return Some("excessive capitalized shouting is blocked");
        }
        if lowercase.contains("@everyone") || lowercase.contains("@here") {
            return Some("mass-mention spam is blocked");
        }
    }

    None
}

fn contains_blocked_invite_or_link(body: &str) -> bool {
    ["http://", "https://", "www.", "discord.gg/", "bit.ly/"]
        .iter()
        .any(|needle| body.contains(needle))
}

fn contains_repeated_spam_pattern(body: &str) -> bool {
    let tokens = body
        .split_whitespace()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if tokens.len() >= 4 && tokens.windows(2).all(|pair| pair[0] == pair[1]) {
        return true;
    }

    let collapsed = body.replace(' ', "");
    collapsed
        .chars()
        .collect::<Vec<_>>()
        .windows(8)
        .any(|window| window.iter().all(|value| *value == window[0]))
}

fn is_excessive_caps(body: &str) -> bool {
    let letters = body.chars().filter(|value| value.is_ascii_alphabetic());
    let mut total = 0;
    let mut uppercase = 0;
    for letter in letters {
        total += 1;
        if letter.is_ascii_uppercase() {
            uppercase += 1;
        }
    }

    total >= 12 && uppercase * 10 >= total * 8
}
