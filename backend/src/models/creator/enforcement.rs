use super::*;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorAnalyticsSummary {
    pub window_days: i64,
    pub total_viewers: i64,
    pub total_watch_minutes: i64,
    pub total_revenue: f64,
    pub total_new_followers: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorRevenueBreakdownEntry {
    pub source: String,
    pub amount: f64,
    pub share: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorRevenueSummary {
    pub total_earnings_30d: f64,
    pub total_subscribers: i64,
    pub blended_monthly_price: f64,
    pub estimated_next_payout: f64,
    pub breakdown: Vec<CreatorRevenueBreakdownEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorOperationalChecklistItem {
    pub key: String,
    pub label: String,
    pub complete: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorOperationalState {
    pub creator_id: Id,
    pub legal_name: String,
    pub support_email: String,
    pub business_type: String,
    pub payout_country: String,
    pub payout_provider: String,
    pub onboarding_status: String,
    pub identity_status: String,
    pub tax_status: String,
    pub payout_status: String,
    pub hold_reasons: Vec<String>,
    pub active_enforcement_actions: Vec<CreatorEnforcementAction>,
    pub live_streaming_enabled: bool,
    pub upload_ingest_enabled: bool,
    pub collaboration_enabled: bool,
    pub monetization_enabled: bool,
    pub payouts_enabled: bool,
    pub can_receive_payouts: bool,
    pub can_monetize: bool,
    pub can_publish_paid_content: bool,
    pub requires_action: bool,
    pub checklist: Vec<CreatorOperationalChecklistItem>,
    pub created_at: String,
    pub updated_at: String,
    pub last_reviewed_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorEnforcementAction {
    pub id: Id,
    pub creator_id: Id,
    pub scope: String,
    pub state: String,
    pub reason: String,
    pub resolution_note: Option<String>,
    pub created_by_user_id: Id,
    pub released_by_user_id: Option<Id>,
    pub created_at: String,
    pub released_at: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorEnforcementReconciliationAction {
    pub action_type: String,
    pub target_id: Id,
    pub previous_state: Option<String>,
    pub next_state: Option<String>,
    pub reason: String,
    pub occurred_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorEnforcementReconciliationReport {
    pub action_id: Id,
    pub reconciled_at: String,
    pub actions: Vec<CreatorEnforcementReconciliationAction>,
    pub action: CreatorEnforcementAction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorEnforcementState {
    pub creator_id: Id,
    pub live_streaming_enabled: bool,
    pub upload_ingest_enabled: bool,
    pub collaboration_enabled: bool,
    pub monetization_enabled: bool,
    pub payouts_enabled: bool,
    pub active_actions: Vec<CreatorEnforcementAction>,
    pub history: Vec<CreatorEnforcementAction>,
}
