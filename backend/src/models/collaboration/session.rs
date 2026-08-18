use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationInvite {
    pub id: Id,
    pub session_id: Id,
    pub host_creator_id: Id,
    pub invitee_user_id: Id,
    pub invitee_creator_id: Option<Id>,
    pub role: String,
    pub state: String,
    pub mirror_to_guest_channel: bool,
    pub message: Option<String>,
    pub created_at: String,
    pub responded_at: Option<String>,
    pub expires_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationParticipant {
    pub id: Id,
    pub session_id: Id,
    pub invite_id: Option<Id>,
    pub user_id: Id,
    pub creator_id: Option<Id>,
    pub role: String,
    pub state: String,
    pub publish_to_host: bool,
    pub mirror_to_guest_channel: bool,
    pub can_speak_in_chat: bool,
    pub joined_at: Option<String>,
    pub left_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationSession {
    pub id: Id,
    pub host_creator_id: Id,
    pub source_broadcast_id: Id,
    pub title: String,
    pub status: String,
    pub chat_mode: String,
    pub recording_policy: String,
    pub last_event_seq: i64,
    pub created_at: String,
    pub updated_at: String,
    pub activated_at: Option<String>,
    pub ended_at: Option<String>,
    pub invites: Vec<CollaborationInvite>,
    pub participants: Vec<CollaborationParticipant>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationHostSummary {
    pub creator_id: Id,
    pub user_id: Id,
    pub handle: String,
    pub display_name: String,
    pub avatar: String,
    pub partner_status: String,
    pub live_status: String,
    pub current_broadcast_id: Option<Id>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationSessionView {
    pub id: Id,
    pub host_creator_id: Id,
    pub source_broadcast_id: Id,
    pub title: String,
    pub status: String,
    pub chat_mode: String,
    pub recording_policy: String,
    pub last_event_seq: i64,
    pub created_at: String,
    pub updated_at: String,
    pub activated_at: Option<String>,
    pub ended_at: Option<String>,
    pub host: CollaborationHostSummary,
    pub participant: CollaborationParticipant,
    pub participants: Vec<CollaborationParticipant>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationEvent {
    pub id: Id,
    pub session_id: Id,
    pub sequence: i64,
    pub actor_user_id: Option<Id>,
    pub participant_id: Option<Id>,
    pub event_type: String,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationMirrorGrant {
    pub id: Id,
    pub session_id: Id,
    pub participant_id: Id,
    pub host_creator_id: Id,
    pub guest_creator_id: Id,
    pub scope: String,
    pub state: String,
    pub publish_to_host: bool,
    pub mirror_to_guest_channel: bool,
    pub issued_at: String,
    pub activated_at: Option<String>,
    pub revoked_at: Option<String>,
    pub expires_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationMirrorPickup {
    pub id: Id,
    pub session_id: Id,
    pub participant_id: Id,
    pub grant_id: Id,
    pub host_creator_id: Id,
    pub guest_creator_id: Id,
    pub source_broadcast_id: Id,
    pub guest_broadcast_id: Id,
    pub state: String,
    pub activated_at: String,
    pub updated_at: String,
    pub ended_at: Option<String>,
}
