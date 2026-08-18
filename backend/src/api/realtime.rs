use super::*;

mod chat;
mod collaboration;
mod sockets;

pub(super) use chat::persist_chat_message;
pub(super) use collaboration::{
    CollaborationSocketCommand, execute_collaboration_socket_command,
    fetch_current_collaboration_socket_session_view, reconcile_collaboration_expiry_for_host_read,
    reconcile_collaboration_expiry_for_participant_read,
    reconcile_collaboration_session_expiry_for_read,
};
pub(super) use sockets::auth_session_channel_id;

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/ws/live/:stream_id", get(sockets::ws_live))
        .route("/ws/creator/live", get(sockets::ws_creator_live))
        .route(
            "/ws/live/collabs/:session_id",
            get(sockets::ws_collaboration),
        )
}
