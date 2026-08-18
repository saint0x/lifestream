use super::*;

pub(super) fn auth_headers(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).expect("valid auth header"),
    );
    headers
}

pub(super) async fn insert_creator_auth_session(
    pool: &SqlitePool,
    creator: &CreatorProfile,
) -> AppResult<String> {
    let token = format!("test-creator-token-{}", Uuid::new_v4().simple());
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO auth_sessions (
            id, user_id, label, token_hash, scopes_json, created_at, expires_at, revoked_at, last_used_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(format!("sess-test-{}", Uuid::new_v4().simple()))
    .bind(&creator.user_id)
    .bind("test-creator-session")
    .bind(hash_token(&token))
    .bind(
        json!(["user", "creator", "creator:write", "admin"]).to_string(),
    )
    .bind(&now)
    .bind((Utc::now() + chrono::Duration::hours(2)).to_rfc3339())
    .bind(Option::<String>::None)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(token)
}

pub(super) async fn insert_user_auth_session(
    pool: &SqlitePool,
    user_id: &str,
    scopes: &[&str],
) -> AppResult<String> {
    let token = format!("test-user-token-{}", Uuid::new_v4().simple());
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO auth_sessions (
            id, user_id, label, token_hash, scopes_json, created_at, expires_at, revoked_at, last_used_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(format!("sess-user-test-{}", Uuid::new_v4().simple()))
    .bind(user_id)
    .bind("test-user-session")
    .bind(hash_token(&token))
    .bind(serde_json::to_string(scopes)?)
    .bind(&now)
    .bind((Utc::now() + chrono::Duration::hours(2)).to_rfc3339())
    .bind(Option::<String>::None)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(token)
}
