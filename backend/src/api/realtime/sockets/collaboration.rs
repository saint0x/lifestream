use super::*;

pub(super) async fn handle_collaboration_socket(
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
