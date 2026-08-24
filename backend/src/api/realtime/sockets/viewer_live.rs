use super::*;

pub(super) async fn handle_socket(
    socket: WebSocket,
    state: SharedState,
    stream_id: String,
    viewer_identity: Option<RequestIdentity>,
    after_seq: i64,
    session_token: Option<String>,
    attribution: LiveViewerAttribution,
) {
    let (mut sender, mut receiver) = socket.split();
    if let Some(identity) = viewer_identity.as_ref() {
        if ensure_identity_session_active(state.db.sqlite_adapter(), identity)
            .await
            .is_err()
        {
            return;
        }
    }
    let (presence_session_token, resumed, last_seen_at) = match register_live_viewer_session(
        state.db.sqlite_adapter(),
        &stream_id,
        viewer_identity.as_ref(),
        session_token.as_deref(),
        Some(&attribution),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => return,
    };
    let history = fetch_chat_messages_for_viewer(
        state.db.sqlite_adapter(),
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
        state.db.sqlite_adapter(),
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
        Some(identity) => fetch_active_live_moderation_action(
            state.db.sqlite_adapter(),
            &stream_id,
            &identity.user_id,
        )
        .await
        .unwrap_or_default(),
        None => None,
    };
    let total_viewers = effective_live_viewer_count(state.db.sqlite_adapter(), &stream_id)
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
                    if ensure_identity_session_active(state.db.sqlite_adapter(), identity).await.is_err() {
                        close_websocket(&mut sender).await;
                        break;
                    }
                }
                let _ = touch_live_viewer_session(
                    state.db.sqlite_adapter(),
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
                            if ensure_identity_session_active(state.db.sqlite_adapter(), identity).await.is_err() {
                                close_websocket(&mut sender).await;
                                break;
                            }
                        }
                        let _ = touch_live_viewer_session(
                            state.db.sqlite_adapter(),
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
                            if ensure_identity_session_active(state.db.sqlite_adapter(), identity).await.is_err() {
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
        state.db.sqlite_adapter(),
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
    let remaining = effective_live_viewer_count(state.db.sqlite_adapter(), &stream_id)
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
