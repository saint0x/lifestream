use super::*;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthDependencyStatus {
    pub ready: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthDependencies {
    pub media_root: HealthDependencyStatus,
    pub ffmpeg: HealthDependencyStatus,
    pub ffprobe: HealthDependencyStatus,
    pub background_worker: HealthDependencyStatus,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub ready: bool,
    pub database: bool,
    pub dependencies: HealthDependencies,
    pub uptime_sec: u64,
    pub timestamp: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSession {
    pub id: Id,
    pub label: String,
    pub scopes: Vec<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub revoked_at: Option<String>,
    pub last_used_at: Option<String>,
    pub is_current: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTokenResponse {
    pub session: AuthSession,
    pub access_token: String,
}
