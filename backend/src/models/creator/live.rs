use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLiveSocketPresence {
    pub id: Id,
    pub creator_id: Id,
    pub user_id: Id,
    pub connected_at: String,
    pub last_seen_at: String,
    pub disconnected_at: Option<String>,
    pub is_stale: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLiveSocketPresenceReconciliationAction {
    pub action_type: String,
    pub target_id: Id,
    pub previous_state: Option<String>,
    pub next_state: Option<String>,
    pub reason: String,
    pub occurred_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLiveSocketPresenceReconciliationReport {
    pub creator_id: Id,
    pub socket_session_id: Id,
    pub reconciled_at: String,
    pub actions: Vec<CreatorLiveSocketPresenceReconciliationAction>,
    pub socket_session: CreatorLiveSocketPresence,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLiveControlResponse {
    pub snapshot: CreatorLiveSnapshot,
    pub settings: CreatorLiveSettings,
    pub health: CreatorLiveHealth,
    pub collaboration: CreatorLiveCollaborationSummary,
    pub subscriber_tiers: Vec<CreatorSubscriberTier>,
    pub is_live: bool,
    pub current_viewers: i64,
    pub bitrate_history: Vec<i64>,
    pub viewer_history: Vec<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveIngestEvent {
    pub id: Id,
    pub session_id: Id,
    pub creator_id: Id,
    pub broadcast_id: Id,
    pub event_type: String,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLiveRuntimeResponse {
    pub snapshot: CreatorLiveSnapshot,
    pub health: CreatorLiveHealth,
    pub collaboration: CreatorLiveCollaborationSummary,
    pub active_session: Option<LiveIngestSession>,
    pub recent_sessions: Vec<LiveIngestSession>,
    pub recent_events: Vec<LiveIngestEvent>,
}
