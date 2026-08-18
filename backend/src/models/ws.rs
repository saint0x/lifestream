use super::*;

#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum WsEvent {
    SessionReady {
        channel: String,
        session_token: String,
        resumed: bool,
        last_seen_at: String,
    },
    SessionInvalidated {
        reason: String,
    },
    ChatReplay {
        after_seq: i64,
        messages: Vec<ChatMessage>,
    },
    ChatHistory {
        messages: Vec<ChatMessage>,
    },
    ChatMessage {
        message: ChatMessage,
    },
    ChatMessageRejected {
        reason: String,
    },
    ViewerCount {
        viewer_count: i64,
    },
    CollaborationSnapshot {
        session: CollaborationSessionView,
        grants: Vec<CollaborationMirrorGrant>,
        pickups: Vec<CollaborationMirrorPickup>,
        events: Vec<CollaborationEvent>,
    },
    CollaborationReplay {
        after_seq: i64,
        events: Vec<CollaborationEvent>,
    },
    CollaborationEvent {
        event: CollaborationEvent,
    },
    CollaborationPresence {
        session_id: Id,
        connected_participants: i64,
    },
    CollaborationHeartbeat {
        session_id: Id,
        received_at: String,
    },
    CollaborationCommandAccepted {
        command_type: String,
        participant_id: Option<Id>,
        state: Option<String>,
    },
    CollaborationCommandRejected {
        command_type: String,
        reason: String,
    },
    CollaborationTopology {
        topology: CollaborationRuntimeTopology,
    },
    CreatorLiveState {
        control: CreatorLiveControlResponse,
        runtime: CreatorLiveRuntimeResponse,
    },
    ModerationAction {
        action: LiveModerationAction,
    },
}
