use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRuntimeAdvisoryAction {
    pub code: String,
    pub severity: String,
    pub repairable: bool,
    pub title: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRuntimeArtifactState {
    pub expected_relative_path: Option<String>,
    pub persisted_relative_path: Option<String>,
    pub state: String,
    pub ready: bool,
    pub valid: bool,
    pub issue: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRuntimeArtifactHealth {
    pub status: String,
    pub checked_at: String,
    pub manifest: LiveRuntimeArtifactState,
    pub archive: LiveRuntimeArtifactState,
    pub collaboration: Option<LiveRuntimeArtifactState>,
    pub issues: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRuntimeAdvisory {
    pub status: String,
    pub summary: String,
    pub requires_operator_action: bool,
    pub blocking_issue_count: i64,
    pub repairable_issue_count: i64,
    pub source_validation_state: Option<String>,
    pub runtime_failure_present: bool,
    pub recommended_actions: Vec<LiveRuntimeAdvisoryAction>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRuntimeRepairAction {
    pub field: String,
    pub previous_value: Option<String>,
    pub next_value: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRuntimeRepairReport {
    pub session_id: Id,
    pub creator_id: Id,
    pub broadcast_id: Id,
    pub actor_user_id: Id,
    pub actor_scope: String,
    pub reason: String,
    pub repaired_at: String,
    pub actions: Vec<LiveRuntimeRepairAction>,
    pub record: AdminLiveIngestSessionRecord,
}
