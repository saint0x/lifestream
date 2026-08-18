use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorProfile {
    pub id: Id,
    pub user_id: Id,
    pub handle: String,
    pub display_name: String,
    pub avatar: String,
    pub banner: String,
    pub tagline: String,
    pub bio: String,
    pub partner_status: String,
    pub joined_at: String,
    pub stream_key: String,
    pub rtmp_url: String,
    pub default_category: String,
    pub default_tags: Vec<String>,
    pub followers: i64,
    pub subscribers: i64,
    pub monthly_viewers: i64,
    pub total_watch_hours: i64,
    pub live_status: String,
    pub current_broadcast_id: Option<Id>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Broadcast {
    pub id: Id,
    pub title: String,
    pub category: String,
    pub tags: Vec<String>,
    pub status: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_sec: Option<i64>,
    pub peak_viewers: i64,
    pub average_viewers: i64,
    pub chat_messages: i64,
    pub new_followers: i64,
    pub new_subscribers: i64,
    pub revenue: f64,
    pub thumbnail: String,
    pub is_mature: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveIngestSession {
    pub id: Id,
    pub creator_id: Id,
    pub broadcast_id: Id,
    pub protocol: String,
    pub ingest_server: String,
    pub status: String,
    pub bitrate_kbps: i64,
    pub viewers: i64,
    pub dropped_frames: i64,
    pub connected_at: String,
    pub last_heartbeat_at: String,
    pub disconnected_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminLiveIngestSessionRecord {
    pub session: LiveIngestSession,
    pub stale_connection: bool,
    pub recent_events: Vec<LiveIngestEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveIngestReconciliationAction {
    pub action_type: String,
    pub target_id: Id,
    pub previous_status: Option<String>,
    pub next_status: Option<String>,
    pub reason: String,
    pub occurred_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveIngestReconciliationReport {
    pub session_id: Id,
    pub reconciled_at: String,
    pub actions: Vec<LiveIngestReconciliationAction>,
    pub record: AdminLiveIngestSessionRecord,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLiveSnapshot {
    pub profile: CreatorProfile,
    pub current_broadcast: Option<Broadcast>,
    pub pending_broadcast: Option<Broadcast>,
    pub ingest_session: Option<LiveIngestSession>,
}

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
    pub recording_owner_creator_id: Option<Id>,
    pub connected_participants: i64,
    pub host_output_participant_ids: Vec<Id>,
    pub backstage_participant_ids: Vec<Id>,
    pub live_participant_ids: Vec<Id>,
    pub mirrored_creator_ids: Vec<Id>,
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
