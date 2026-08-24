use super::*;
use crate::api::{
    fetch_collaboration_socket_presence_for_session,
    filter_visible_collaboration_mirror_grants_for_session_view,
    filter_visible_collaboration_mirror_pickups_for_session_view,
};
use tokio::sync::broadcast;

pub(super) struct SocketBootstrap {
    pub(super) session: CollaborationSessionView,
    pub(super) presence_session_token: String,
    pub(super) last_seen_at: String,
    pub(super) subscription: broadcast::Receiver<WsEvent>,
    pub(super) auth_subscription: broadcast::Receiver<WsEvent>,
}

pub(super) async fn bootstrap_socket(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    after_seq: i64,
    session_token: Option<&str>,
    channel_id: &str,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
) -> Option<SocketBootstrap> {
    if ensure_identity_session_active(state.db.sqlite_adapter(), identity)
        .await
        .is_err()
    {
        return None;
    }
    let session = fetch_current_collaboration_socket_session_view(state, session_id, identity)
        .await
        .ok()?;
    let (presence_session_token, resumed, last_seen_at) = register_collaboration_socket_session(
        state.db.sqlite_adapter(),
        &session,
        identity,
        session_token,
    )
    .await
    .ok()?;
    let session_grants =
        fetch_collaboration_mirror_grants_for_session(state.db.sqlite_adapter(), session_id)
            .await
            .unwrap_or_default();
    let session_pickups =
        fetch_collaboration_mirror_pickups_for_session(state.db.sqlite_adapter(), session_id)
            .await
            .unwrap_or_default();
    let socket_sessions =
        fetch_collaboration_socket_presence_for_session(state.db.sqlite_adapter(), session_id)
            .await
            .unwrap_or_default();
    let topology = match build_collaboration_runtime_topology(
        state.db.sqlite_adapter(),
        &session,
        &session_grants,
        &session_pickups,
        &socket_sessions,
    )
    .await
    {
        Ok(topology) => topology,
        Err(_) => {
            let _ = disconnect_collaboration_socket_session(
                state.db.sqlite_adapter(),
                session_id,
                &presence_session_token,
                &last_seen_at,
            )
            .await;
            return None;
        }
    };
    let (snapshot_events, replay_events) =
        load_collaboration_socket_event_bootstrap(state.db.sqlite_adapter(), session_id, after_seq)
            .await
            .unwrap_or_default();
    let (subscription, _) = state.realtime.join(channel_id).await;
    let (auth_subscription, _) = state
        .realtime
        .join(&auth_session_channel_id(&identity.session_id))
        .await;
    let visible_snapshot_events =
        filter_visible_collaboration_events_for_session(&session, snapshot_events);
    let visible_replay_events =
        filter_visible_collaboration_events_for_session(&session, replay_events);

    let _ = sender
        .send(Message::Text(
            serde_json::to_string(&WsEvent::SessionReady {
                channel: channel_id.to_string(),
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
                grants: filter_visible_collaboration_mirror_grants_for_session_view(
                    &session,
                    &session_grants,
                ),
                pickups: filter_visible_collaboration_mirror_pickups_for_session_view(
                    &session,
                    &session_pickups,
                ),
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
    let connected_participants = topology.connected_participants;
    let _ = sender
        .send(Message::Text(
            serde_json::to_string(&WsEvent::CollaborationTopology { topology }).unwrap_or_default(),
        ))
        .await;
    state
        .realtime
        .publish(
            channel_id,
            WsEvent::CollaborationPresence {
                session_id: session_id.to_string(),
                connected_participants,
            },
        )
        .await;
    let _ = publish_collaboration_topology(state, session_id).await;

    Some(SocketBootstrap {
        session,
        presence_session_token,
        last_seen_at,
        subscription,
        auth_subscription,
    })
}
