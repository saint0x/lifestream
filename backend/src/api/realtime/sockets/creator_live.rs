use super::*;

pub(super) async fn handle_creator_live_socket(
    socket: WebSocket,
    state: SharedState,
    creator_id: String,
    identity: RequestIdentity,
    session_token: Option<String>,
) {
    let channel_id = creator_live_channel_id(&creator_id);
    let (mut sender, mut receiver) = socket.split();
    if ensure_identity_session_active(state.db.sqlite_adapter(), &identity)
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
        state.db.sqlite_adapter(),
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
                    state.db.sqlite_adapter(),
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
                    state.db.sqlite_adapter(),
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
                if ensure_identity_session_active(state.db.sqlite_adapter(), &identity).await.is_err() {
                    close_websocket(&mut sender).await;
                    break;
                }
                let _ = touch_creator_live_socket_session(
                    state.db.sqlite_adapter(),
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
                        if ensure_identity_session_active(state.db.sqlite_adapter(), &identity).await.is_err() {
                            close_websocket(&mut sender).await;
                            break;
                        }
                        let _ = touch_creator_live_socket_session(
                            state.db.sqlite_adapter(),
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
                        if ensure_identity_session_active(state.db.sqlite_adapter(), &identity).await.is_err() {
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
        state.db.sqlite_adapter(),
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
