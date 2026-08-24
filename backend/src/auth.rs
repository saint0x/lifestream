use std::{
    collections::HashMap,
    sync::OnceLock,
    time::{Duration, Instant},
};

use axum::http::HeaderMap;
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use crate::db::Database;
use crate::error::{AppError, AppResult};

const AUTH_SESSION_TOUCH_MIN_INTERVAL: Duration = Duration::from_secs(30);

fn auth_session_touch_gates() -> &'static Mutex<HashMap<String, Instant>> {
    static GATES: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    GATES.get_or_init(|| Mutex::new(HashMap::new()))
}

async fn should_touch_auth_session(session_id: &str) -> bool {
    let now = Instant::now();
    let mut guard = auth_session_touch_gates().lock().await;
    match guard.get(session_id).copied() {
        Some(last_touch) if now.duration_since(last_touch) < AUTH_SESSION_TOUCH_MIN_INTERVAL => {
            false
        }
        _ => {
            guard.insert(session_id.to_string(), now);
            true
        }
    }
}

#[derive(Clone, Debug)]
pub struct RequestIdentity {
    pub session_id: String,
    pub user_id: String,
    pub creator_id: Option<String>,
    pub scopes: Vec<String>,
}

impl RequestIdentity {
    pub fn require_admin_scope(&self) -> AppResult<&str> {
        let has_scope = self
            .scopes
            .iter()
            .any(|scope| matches!(scope.as_str(), "admin" | "admin:write" | "operator"));

        if !has_scope {
            return Err(AppError::Forbidden);
        }

        Ok(&self.user_id)
    }

    pub fn require_creator_scope(&self) -> AppResult<&str> {
        let has_scope = self
            .scopes
            .iter()
            .any(|scope| scope == "creator" || scope == "creator:write");

        if !has_scope {
            return Err(AppError::Forbidden);
        }

        self.creator_id.as_deref().ok_or(AppError::Forbidden)
    }
}

pub async fn optional_identity(
    database: &Database,
    headers: &HeaderMap,
) -> AppResult<Option<RequestIdentity>> {
    let Some(token) = extract_bearer_token(headers)? else {
        return Ok(None);
    };

    lookup_identity(database, &token).await.map(Some)
}

pub async fn require_identity(
    database: &Database,
    headers: &HeaderMap,
) -> AppResult<RequestIdentity> {
    let token = extract_bearer_token(headers)?.ok_or(AppError::Unauthorized)?;
    lookup_identity(database, &token).await
}

pub async fn lookup_identity(database: &Database, token: &str) -> AppResult<RequestIdentity> {
    let token_hash = hash_token(token);
    let now = Utc::now().to_rfc3339();

    let identity = database.lookup_identity(&token_hash, &now).await?;
    if should_touch_auth_session(&identity.session_id).await {
        database
            .touch_auth_session(&identity.session_id, &now)
            .await?;
    }

    Ok(identity)
}

pub fn hash_token(token: &str) -> String {
    hash_token_with_secret(
        token,
        std::env::var("VANTA_TOKEN_HASH_SECRET").ok().as_deref(),
    )
}

pub(crate) fn hash_token_with_secret(token: &str, secret: Option<&str>) -> String {
    if let Some(secret) = secret.filter(|value| !value.trim().is_empty()) {
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .expect("HMAC-SHA256 accepts any key length");
        mac.update(token.as_bytes());
        return format!("{:x}", mac.finalize().into_bytes());
    }

    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::hash_token_with_secret;

    #[test]
    fn token_hash_uses_legacy_sha256_without_secret() {
        assert_eq!(
            hash_token_with_secret("session-token", None),
            "c101e911469c969171040b50d70543313cf968fdef5bacc780776f8fb399ab36"
        );
    }

    #[test]
    fn token_hash_uses_secret_hmac_when_configured() {
        let legacy = hash_token_with_secret("session-token", None);
        let keyed =
            hash_token_with_secret("session-token", Some("0123456789abcdef0123456789abcdef"));
        let other_secret =
            hash_token_with_secret("session-token", Some("fedcba9876543210fedcba9876543210"));

        assert_ne!(keyed, legacy);
        assert_ne!(keyed, other_secret);
        assert_eq!(
            keyed,
            hash_token_with_secret("session-token", Some("0123456789abcdef0123456789abcdef")),
        );
    }
}

pub fn extract_bearer_token(headers: &HeaderMap) -> AppResult<Option<String>> {
    let Some(value) = headers.get(axum::http::header::AUTHORIZATION) else {
        return Ok(None);
    };

    let value = value.to_str().map_err(|_| AppError::Unauthorized)?;
    let token = value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
        .ok_or(AppError::Unauthorized)?
        .trim();

    if token.is_empty() {
        return Err(AppError::Unauthorized);
    }

    Ok(Some(token.to_string()))
}
