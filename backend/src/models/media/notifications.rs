use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorNotification {
    pub id: Id,
    pub kind: String,
    pub body: String,
    pub sent_at: String,
    pub amount: Option<f64>,
    pub actor: Option<String>,
    pub delivery_state: Option<String>,
    pub read_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserNotification {
    pub id: Id,
    pub kind: String,
    pub body: String,
    pub sent_at: String,
    pub amount: Option<f64>,
    pub actor: Option<String>,
    pub delivery_state: String,
    pub read_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationDeliveryRecord {
    pub id: Id,
    pub event_id: Id,
    pub kind: String,
    pub body: String,
    pub channel: String,
    pub state: String,
    pub actor: Option<String>,
    pub recipient_user_id: Option<Id>,
    pub recipient_creator_id: Option<Id>,
    pub sent_at: String,
    pub delivered_at: Option<String>,
    pub read_at: Option<String>,
    pub failed_at: Option<String>,
    pub last_error: Option<String>,
    pub retry_count: i64,
    pub last_attempted_at: Option<String>,
    pub next_attempt_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationDeliveryReconciliationAction {
    pub action_type: String,
    pub target_id: Id,
    pub previous_state: Option<String>,
    pub next_state: Option<String>,
    pub reason: String,
    pub occurred_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationDeliveryReconciliationReport {
    pub delivery_id: Id,
    pub reconciled_at: String,
    pub actions: Vec<NotificationDeliveryReconciliationAction>,
    pub delivery: NotificationDeliveryRecord,
}
