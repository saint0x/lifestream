use super::*;

#[derive(Debug, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum CollaborationSocketCommand {
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
        media_transport: Option<String>,
        contribution_endpoint_url: Option<String>,
        return_endpoint_url: Option<String>,
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

pub(crate) struct CollaborationSocketCommandOutcome {
    pub(crate) command_type: &'static str,
    pub(crate) participant_id: Option<String>,
    pub(crate) state: Option<String>,
}

pub(crate) fn collaboration_socket_command_name(value: &Value) -> String {
    value
        .get("type")
        .and_then(Value::as_str)
        .map(|command_type| command_type.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

pub(crate) async fn send_collaboration_command_accepted(
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

pub(crate) async fn send_collaboration_command_rejected(
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
