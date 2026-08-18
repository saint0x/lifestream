use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationContributionAttachment {
    pub participant_id: Id,
    pub user_id: Id,
    pub creator_id: Option<Id>,
    pub transport_class: String,
    pub source_broadcast_id: Option<Id>,
    pub ingest_session_id: Option<Id>,
    pub contribution_state: String,
    pub attached_output_ids: Vec<Id>,
    pub mix_minus_required: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationOutputRoute {
    pub id: Id,
    pub output_kind: String,
    pub route_state: String,
    pub target_creator_id: Option<Id>,
    pub target_broadcast_id: Option<Id>,
    pub source_participant_ids: Vec<Id>,
    pub playback_enabled: bool,
    pub recording_enabled: bool,
    pub mix_minus_required: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationTopologyMember {
    pub participant_id: Id,
    pub user_id: Id,
    pub creator_id: Option<Id>,
    pub role: String,
    pub state: String,
    pub publish_to_host: bool,
    pub mirror_to_guest_channel: bool,
    pub can_speak_in_chat: bool,
    pub host_output_state: String,
    pub mirror_pickup_state: String,
    pub mirror_pickup_broadcast_id: Option<Id>,
    pub mirror_pickup_activated_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationRuntimeTopology {
    pub session_id: Id,
    pub source_broadcast_id: Id,
    pub chat_mode: String,
    pub recording_policy: String,
    pub shared_chat: bool,
    pub mix_minus_required: bool,
    pub recording_owner_creator_id: Option<Id>,
    pub connected_participants: i64,
    pub host_output_participant_ids: Vec<Id>,
    pub backstage_participant_ids: Vec<Id>,
    pub live_participant_ids: Vec<Id>,
    pub mirrored_creator_ids: Vec<Id>,
    pub contributions: Vec<CollaborationContributionAttachment>,
    pub outputs: Vec<CollaborationOutputRoute>,
    pub members: Vec<CollaborationTopologyMember>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationRuntimeResponse {
    pub session: CollaborationSessionView,
    pub topology: CollaborationRuntimeTopology,
    pub grants: Vec<CollaborationMirrorGrant>,
    pub pickups: Vec<CollaborationMirrorPickup>,
    pub recent_events: Vec<CollaborationEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationSocketPresence {
    pub id: Id,
    pub session_id: Id,
    pub user_id: Id,
    pub creator_id: Option<Id>,
    pub participant_id: Option<Id>,
    pub connected_at: String,
    pub last_seen_at: String,
    pub disconnected_at: Option<String>,
    pub is_stale: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationSocketPresenceReconciliationAction {
    pub action_type: String,
    pub target_id: Id,
    pub previous_state: Option<String>,
    pub next_state: Option<String>,
    pub reason: String,
    pub occurred_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationSocketPresenceReconciliationReport {
    pub session_id: Id,
    pub socket_session_id: Id,
    pub reconciled_at: String,
    pub actions: Vec<CollaborationSocketPresenceReconciliationAction>,
    pub socket_session: CollaborationSocketPresence,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorCollaborationControlResponse {
    pub runtime: CollaborationRuntimeResponse,
    pub socket_sessions: Vec<CollaborationSocketPresence>,
    pub pending_invite_count: i64,
    pub active_grant_count: i64,
    pub issued_grant_count: i64,
    pub stale_socket_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLiveCollaborationSummary {
    pub active_session: Option<CollaborationSession>,
    pub active_control: Option<CreatorCollaborationControlResponse>,
    pub recent_sessions: Vec<CollaborationSession>,
    pub total_sessions: i64,
    pub active_session_count: i64,
    pub pending_invite_count: i64,
    pub active_grant_count: i64,
    pub issued_grant_count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationReconciliationAction {
    pub action_type: String,
    pub target_id: Id,
    pub previous_state: Option<String>,
    pub next_state: Option<String>,
    pub reason: String,
    pub occurred_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationReconciliationReport {
    pub session_id: Id,
    pub reconciled_at: String,
    pub actions: Vec<CollaborationReconciliationAction>,
    pub control: CreatorCollaborationControlResponse,
}
