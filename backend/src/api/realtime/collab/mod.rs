use super::*;
use axum::extract::ws::{Message, WebSocket};
use futures_util::sink::SinkExt;
use serde::Deserialize;
use serde_json::Value;

mod commands;
mod protocol;
mod reconciliation;

pub(crate) use commands::execute_collaboration_socket_command;
pub(crate) use protocol::{
    CollaborationSocketCommand, CollaborationSocketCommandOutcome,
    collaboration_socket_command_name, send_collaboration_command_accepted,
    send_collaboration_command_rejected,
};
pub(crate) use reconciliation::{
    fetch_current_collaboration_socket_session_view, reconcile_collaboration_expiry_for_host_read,
    reconcile_collaboration_expiry_for_participant_read,
    reconcile_collaboration_session_expiry_for_read, validate_collaboration_socket_access,
};
