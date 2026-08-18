use super::*;

pub(super) async fn insert_playback_session_for_upload(
    pool: &SqlitePool,
    upload_id: &str,
    user_id: Option<&str>,
    auth_session_id: Option<&str>,
    access_scope: &str,
) -> AppResult<(String, String, MediaAsset)> {
    let target = fetch_upload_playback_target(pool, upload_id).await?;
    let session_id = format!("test-pbs-{}", Uuid::new_v4().simple());
    let playback_token = format!("test-pbt-{}", Uuid::new_v4().simple());
    let now = Utc::now();
    let now_rfc3339 = now.to_rfc3339();
    let expires_at = (now + chrono::Duration::hours(1)).to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO playback_sessions (
            id, auth_session_id, user_id, creator_id, asset_id, content_id, content_kind, token_hash,
            access_scope, created_at, expires_at, last_used_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&session_id)
    .bind(auth_session_id)
    .bind(user_id)
    .bind(Some(target.creator_id.clone()))
    .bind(&target.asset.id)
    .bind(upload_id)
    .bind(&target.asset.kind)
    .bind(hash_token(&playback_token))
    .bind(access_scope)
    .bind(&now_rfc3339)
    .bind(&expires_at)
    .bind(&now_rfc3339)
    .execute(pool)
    .await?;
    Ok((session_id, playback_token, target.asset))
}

pub(super) async fn seed_content_purchase_for_user(
    pool: &SqlitePool,
    user_id: &str,
    creator_id: &str,
    upload_id: &str,
    access_policy: &str,
    amount_cents: i64,
    currency: &str,
    purchased_at: &str,
    expires_at: Option<&str>,
    status: &str,
) -> AppResult<String> {
    sqlx::query("DELETE FROM content_purchases WHERE user_id = ? AND upload_id = ?")
        .bind(user_id)
        .bind(upload_id)
        .execute(pool)
        .await?;
    let purchase_id = format!("pur-test-{}", Uuid::new_v4().simple());
    sqlx::query(
        r#"
        INSERT INTO content_purchases (
            id, user_id, creator_id, upload_id, access_policy, amount_cents, currency,
            status, purchased_at, expires_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&purchase_id)
    .bind(user_id)
    .bind(creator_id)
    .bind(upload_id)
    .bind(access_policy)
    .bind(amount_cents)
    .bind(currency)
    .bind(status)
    .bind(purchased_at)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(purchase_id)
}
