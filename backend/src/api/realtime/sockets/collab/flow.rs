use super::*;
use tokio::sync::broadcast;

pub(super) async fn run_socket_loop(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    presence_session_token: &str,
    last_seen_at: &str,
    session: &mut CollaborationSessionView,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    receiver: &mut futures_util::stream::SplitStream<WebSocket>,
    subscription: &mut broadcast::Receiver<WsEvent>,
    auth_subscription: &mut broadcast::Receiver<WsEvent>,
) {
    let mut touch_interval = interval(Duration::from_secs(WS_PRESENCE_TOUCH_INTERVAL_SECONDS));

    loop {
        tokio::select! {
            _ = touch_interval.tick() => {
                if refresh_socket_session(state, session_id, identity, presence_session_token, last_seen_at, session).await.is_err() {
                    close_websocket(sender).await;
                    break;
                }
            }
            incoming = receiver.next() => {
                if !handle_incoming_message(
                    state,
                    session_id,
                    identity,
                    presence_session_token,
                    last_seen_at,
                    session,
                    sender,
                    incoming,
                ).await {
                    break;
                }
            }
            outbound = subscription.recv() => {
                if !handle_outbound_event(
                    state,
                    session_id,
                    identity,
                    session,
                    sender,
                    outbound,
                ).await {
                    break;
                }
            }
            revoked = auth_subscription.recv() => {
                match revoked {
                    Ok(WsEvent::SessionInvalidated { .. }) => {
                        close_websocket(sender).await;
                        break;
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        }
    }
}

async fn refresh_socket_session(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    presence_session_token: &str,
    last_seen_at: &str,
    session: &mut CollaborationSessionView,
) -> AppResult<()> {
    ensure_identity_session_active(state.db.sqlite_adapter(), identity).await?;
    *session = fetch_current_collaboration_socket_session_view(state, session_id, identity).await?;
    let _ = touch_collaboration_socket_session(
        state.db.sqlite_adapter(),
        session_id,
        presence_session_token,
        last_seen_at,
    )
    .await;
    Ok(())
}

async fn handle_incoming_message(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    presence_session_token: &str,
    last_seen_at: &str,
    session: &mut CollaborationSessionView,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    incoming: Option<Result<Message, axum::Error>>,
) -> bool {
    match incoming {
        Some(Ok(Message::Text(text))) => {
            if refresh_socket_session(
                state,
                session_id,
                identity,
                presence_session_token,
                last_seen_at,
                session,
            )
            .await
            .is_err()
            {
                close_websocket(sender).await;
                return false;
            }
            let command_value = match serde_json::from_str::<Value>(&text) {
                Ok(value) => value,
                Err(_) => {
                    return send_collaboration_command_rejected(
                        sender,
                        "unknown",
                        "invalid collaboration command payload",
                    )
                    .await;
                }
            };
            let command_type = collaboration_socket_command_name(&command_value);
            let command = match serde_json::from_value::<CollaborationSocketCommand>(command_value)
            {
                Ok(command) => command,
                Err(_) => {
                    return send_collaboration_command_rejected(
                        sender,
                        &command_type,
                        "unsupported collaboration command",
                    )
                    .await;
                }
            };

            match execute_collaboration_socket_command(
                state, session_id, identity, session, command,
            )
            .await
            {
                Ok(outcome) if outcome.command_type == "heartbeat" => sender
                    .send(Message::Text(
                        serde_json::to_string(&WsEvent::CollaborationHeartbeat {
                            session_id: session_id.to_string(),
                            received_at: Utc::now().to_rfc3339(),
                        })
                        .unwrap_or_default(),
                    ))
                    .await
                    .is_ok(),
                Ok(outcome) => {
                    send_collaboration_command_accepted(
                        sender,
                        outcome.command_type,
                        outcome.participant_id,
                        outcome.state,
                    )
                    .await
                }
                Err(error) => {
                    send_collaboration_command_rejected(sender, &command_type, error.to_string())
                        .await
                }
            }
        }
        Some(Ok(Message::Close(_))) | None => false,
        Some(Ok(_)) => true,
        Some(Err(_)) => false,
    }
}

async fn handle_outbound_event(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    session: &mut CollaborationSessionView,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    outbound: Result<WsEvent, broadcast::error::RecvError>,
) -> bool {
    match outbound {
        Ok(event) => {
            if matches!(event, WsEvent::SessionInvalidated { .. }) {
                close_websocket(sender).await;
                return false;
            }
            if ensure_identity_session_active(state.db.sqlite_adapter(), identity)
                .await
                .is_err()
            {
                close_websocket(sender).await;
                return false;
            }
            match fetch_current_collaboration_socket_session_view(state, session_id, identity).await
            {
                Ok(current_session) => {
                    *session = current_session;
                }
                Err(_) => return false,
            }
            if let WsEvent::CollaborationEvent {
                event: collaboration_event,
            } = &event
            {
                if !collaboration_event_is_visible_to_session(session, collaboration_event) {
                    return true;
                }
            }
            sender
                .send(Message::Text(
                    serde_json::to_string(&event).unwrap_or_default(),
                ))
                .await
                .is_ok()
        }
        Err(_) => false,
    }
}
