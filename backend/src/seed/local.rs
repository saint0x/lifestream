use chrono::Utc;
use sqlx::SqlitePool;

use crate::{auth::hash_token, config::Config};

use super::json;

pub(super) async fn seed_local_auth_session(
    pool: &SqlitePool,
    config: &Config,
) -> Result<(), sqlx::Error> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO auth_sessions (
            id, user_id, label, token_hash, scopes_json, created_at, expires_at, revoked_at, last_used_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            user_id = excluded.user_id,
            label = excluded.label,
            token_hash = excluded.token_hash,
            scopes_json = excluded.scopes_json,
            revoked_at = NULL,
            expires_at = excluded.expires_at
        "#,
    )
    .bind("sess-local-admin")
    .bind("usr-1")
    .bind("local-dev-admin")
    .bind(hash_token(&config.local_seed_token))
    .bind(json(&vec!["user", "creator", "creator:write", "admin"])?)
    .bind(&now)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO auth_sessions (
            id, user_id, label, token_hash, scopes_json, created_at, expires_at, revoked_at, last_used_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            user_id = excluded.user_id,
            label = excluded.label,
            token_hash = excluded.token_hash,
            scopes_json = excluded.scopes_json,
            revoked_at = NULL,
            expires_at = excluded.expires_at
        "#,
    )
    .bind("sess-local-collaborator")
    .bind("usr-2")
    .bind("local-dev-collaborator")
    .bind(hash_token("lifestream-local-collaborator-token"))
    .bind(json(&vec!["user", "creator"])?)
    .bind(&now)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .execute(pool)
    .await?;

    Ok(())
}
