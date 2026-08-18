use super::*;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollaborationSessionRequest {
    pub broadcast_id: Option<Id>,
    pub title: Option<String>,
    pub chat_mode: Option<String>,
    pub recording_policy: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollaborationInviteRequest {
    pub invitee_user_id: Id,
    pub role: String,
    pub mirror_to_guest_channel: bool,
    pub message: Option<String>,
    pub expires_in_minutes: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCollaborationParticipantRequest {
    pub state: Option<String>,
    pub publish_to_host: Option<bool>,
    pub mirror_to_guest_channel: Option<bool>,
    pub can_speak_in_chat: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationEventsQuery {
    pub after_seq: Option<i64>,
    pub limit: Option<i64>,
}
