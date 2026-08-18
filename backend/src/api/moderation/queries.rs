use super::*;

pub(crate) async fn fetch_creator_moderators(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<CreatorModerator>> {
    let rows = sqlx::query(
        "SELECT creator_id, user_id, role, created_at FROM creator_moderators WHERE creator_id = ? ORDER BY created_at DESC",
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| CreatorModerator {
            creator_id: row.get("creator_id"),
            user_id: row.get("user_id"),
            role: row.get("role"),
            created_at: row.get("created_at"),
        })
        .collect())
}

pub(crate) async fn fetch_creator_moderator(
    pool: &SqlitePool,
    creator_id: &str,
    user_id: &str,
) -> AppResult<CreatorModerator> {
    let row = sqlx::query(
        "SELECT creator_id, user_id, role, created_at FROM creator_moderators WHERE creator_id = ? AND user_id = ?",
    )
    .bind(creator_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(CreatorModerator {
        creator_id: row.get("creator_id"),
        user_id: row.get("user_id"),
        role: row.get("role"),
        created_at: row.get("created_at"),
    })
}

pub(crate) async fn fetch_live_moderation_actions(
    pool: &SqlitePool,
    stream_id: &str,
    creator_id: &str,
) -> AppResult<Vec<LiveModerationAction>> {
    reconcile_expired_live_moderation_actions_for_read(
        pool,
        Some(stream_id),
        Some(creator_id),
        None,
        None,
    )
    .await?;
    let rows = sqlx::query(
        r#"
        SELECT id, stream_id, creator_id, subject_user_id, actor_user_id, action_type, reason, state,
               expires_at, created_at, revoked_at
        FROM live_moderation_actions
        WHERE stream_id = ? AND creator_id = ?
        ORDER BY created_at DESC
        "#,
    )
    .bind(stream_id)
    .bind(creator_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(live_moderation_action_from_row)
        .collect())
}

pub(crate) async fn fetch_live_moderation_action_by_id(
    pool: &SqlitePool,
    action_id: &str,
) -> AppResult<LiveModerationAction> {
    reconcile_expired_live_moderation_actions_for_read(pool, None, None, None, Some(action_id))
        .await?;
    fetch_live_moderation_action_by_id_raw(pool, action_id).await
}

pub(crate) async fn fetch_live_moderation_action_by_id_raw(
    pool: &SqlitePool,
    action_id: &str,
) -> AppResult<LiveModerationAction> {
    let row = sqlx::query(
        r#"
        SELECT id, stream_id, creator_id, subject_user_id, actor_user_id, action_type, reason, state,
               expires_at, created_at, revoked_at
        FROM live_moderation_actions
        WHERE id = ?
        "#,
    )
    .bind(action_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(live_moderation_action_from_row(row))
}

pub(crate) async fn fetch_active_live_moderation_action(
    pool: &SqlitePool,
    stream_id: &str,
    subject_user_id: &str,
) -> AppResult<Option<LiveModerationAction>> {
    reconcile_expired_live_moderation_actions_for_read(
        pool,
        Some(stream_id),
        None,
        Some(subject_user_id),
        None,
    )
    .await?;
    let now = Utc::now().to_rfc3339();
    let row = sqlx::query(
        r#"
        SELECT id, stream_id, creator_id, subject_user_id, actor_user_id, action_type, reason, state,
               expires_at, created_at, revoked_at
        FROM live_moderation_actions
        WHERE stream_id = ?
          AND subject_user_id = ?
          AND state = 'active'
          AND (expires_at IS NULL OR expires_at > ?)
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(stream_id)
    .bind(subject_user_id)
    .bind(&now)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(live_moderation_action_from_row))
}

pub(crate) fn live_moderation_action_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> LiveModerationAction {
    LiveModerationAction {
        id: row.get("id"),
        stream_id: row.get("stream_id"),
        creator_id: row.get("creator_id"),
        subject_user_id: row.get("subject_user_id"),
        actor_user_id: row.get("actor_user_id"),
        action_type: row.get("action_type"),
        reason: row.get("reason"),
        state: row.get("state"),
        expires_at: row.get("expires_at"),
        created_at: row.get("created_at"),
        revoked_at: row.get("revoked_at"),
    }
}
