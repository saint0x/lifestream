use std::{
    collections::HashMap,
    sync::OnceLock,
    time::{Duration, Instant},
};

use axum::http::HeaderMap;
use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use tokio::sync::Mutex;

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
    pool: &SqlitePool,
    headers: &HeaderMap,
) -> AppResult<Option<RequestIdentity>> {
    let Some(token) = extract_bearer_token(headers)? else {
        return Ok(None);
    };

    lookup_identity(pool, &token).await.map(Some)
}

pub async fn require_identity(
    pool: &SqlitePool,
    headers: &HeaderMap,
) -> AppResult<RequestIdentity> {
    let token = extract_bearer_token(headers)?.ok_or(AppError::Unauthorized)?;
    lookup_identity(pool, &token).await
}

pub async fn lookup_identity(pool: &SqlitePool, token: &str) -> AppResult<RequestIdentity> {
    let token_hash = hash_token(token);
    let now = Utc::now().to_rfc3339();

    let row = sqlx::query(
        r#"
        SELECT
            auth_sessions.id,
            auth_sessions.user_id,
            auth_sessions.scopes_json,
            creator_profiles.id AS creator_id
        FROM auth_sessions
        LEFT JOIN creator_profiles ON creator_profiles.user_id = auth_sessions.user_id
        WHERE auth_sessions.token_hash = ?
          AND auth_sessions.revoked_at IS NULL
          AND (auth_sessions.expires_at IS NULL OR auth_sessions.expires_at > ?)
        "#,
    )
    .bind(&token_hash)
    .bind(&now)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    let session_id: String = row.get("id");
    if should_touch_auth_session(&session_id).await {
        sqlx::query("UPDATE auth_sessions SET last_used_at = ? WHERE id = ?")
            .bind(&now)
            .bind(&session_id)
            .execute(pool)
            .await?;
    }

    Ok(RequestIdentity {
        session_id,
        user_id: row.get("user_id"),
        creator_id: row.get("creator_id"),
        scopes: serde_json::from_str(&row.get::<String, _>("scopes_json"))?,
    })
}

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
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
