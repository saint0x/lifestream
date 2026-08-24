use super::*;

mod boot;
mod flow;

use boot::{SocketBootstrap, bootstrap_socket};
use flow::run_socket_loop;

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
    let Some(bootstrap) = bootstrap_socket(
        &state,
        &session_id,
        &identity,
        after_seq,
        session_token.as_deref(),
        &channel_id,
        &mut sender,
    )
    .await
    else {
        return;
    };

    let SocketBootstrap {
        mut session,
        presence_session_token,
        last_seen_at,
        mut subscription,
        mut auth_subscription,
    } = bootstrap;

    run_socket_loop(
        &state,
        &session_id,
        &identity,
        &presence_session_token,
        &last_seen_at,
        &mut session,
        &mut sender,
        &mut receiver,
        &mut subscription,
        &mut auth_subscription,
    )
    .await;

    let _ = disconnect_collaboration_socket_session(
        state.db.sqlite_adapter(),
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
    let remaining =
        count_active_collaboration_socket_sessions(state.db.sqlite_adapter(), &session_id)
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
