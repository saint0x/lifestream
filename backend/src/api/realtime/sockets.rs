use super::chat::{persist_chat_message, send_chat_message_rejected};
use super::collaboration::{
    CollaborationSocketCommand, collaboration_socket_command_name,
    fetch_current_collaboration_socket_session_view, send_collaboration_command_accepted,
    send_collaboration_command_rejected,
};
use super::*;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
pub(crate) struct WsAuthQuery {
    #[serde(alias = "accessToken")]
    access_token: Option<String>,
    #[serde(alias = "afterSeq")]
    after_seq: Option<i64>,
    #[serde(alias = "sessionToken")]
    session_token: Option<String>,
}

pub(crate) async fn ws_live(
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

pub(crate) async fn ws_creator_live(
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

pub(crate) async fn ws_collaboration(
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
        collaboration::validate_collaboration_socket_access(&host_view)?;
    } else if let Ok(session) = &participant_access {
        collaboration::validate_collaboration_socket_access(session)?;
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

pub(crate) async fn close_websocket(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
) {
    let _ = sender.send(Message::Close(None)).await;
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

pub(crate) fn auth_session_channel_id(session_id: &str) -> String {
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
