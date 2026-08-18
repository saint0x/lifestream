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
