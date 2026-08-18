use super::*;

pub(crate) async fn fetch_creator_enforcement_state(
    pool: &SqlitePool,
    profile: &CreatorProfile,
) -> AppResult<CreatorEnforcementState> {
    reconcile_expired_creator_enforcement_actions_for_read(pool, Some(&profile.id), None).await?;
    let history = fetch_creator_enforcement_actions(pool, &profile.id).await?;
    let active_actions = fetch_active_creator_enforcement_actions(pool, &profile.id).await?;

    Ok(CreatorEnforcementState {
        creator_id: profile.id.clone(),
        live_streaming_enabled: !active_actions
            .iter()
            .any(|action| action.scope == "live_streaming"),
        upload_ingest_enabled: !active_actions
            .iter()
            .any(|action| action.scope == "uploads"),
        collaboration_enabled: !active_actions
            .iter()
            .any(|action| action.scope == "collaboration"),
        monetization_enabled: !active_actions
            .iter()
            .any(|action| action.scope == "monetization"),
        payouts_enabled: !active_actions
            .iter()
            .any(|action| action.scope == "payouts"),
        active_actions,
        history,
    })
}

pub(crate) async fn fetch_creator_enforcement_actions(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<CreatorEnforcementAction>> {
    let rows = sqlx::query(
        r#"
        SELECT id, creator_id, scope, state, reason, resolution_note, created_by_user_id,
               released_by_user_id, created_at, released_at, expires_at
        FROM creator_enforcement_actions
        WHERE creator_id = ?
        ORDER BY created_at DESC
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(creator_enforcement_action_from_row)
        .collect())
}

pub(crate) async fn fetch_active_creator_enforcement_actions(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<CreatorEnforcementAction>> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"
        SELECT id, creator_id, scope, state, reason, resolution_note, created_by_user_id,
               released_by_user_id, created_at, released_at, expires_at
        FROM creator_enforcement_actions
        WHERE creator_id = ?
          AND state = 'active'
          AND (expires_at IS NULL OR expires_at > ?)
        ORDER BY created_at DESC
        "#,
    )
    .bind(creator_id)
    .bind(&now)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(creator_enforcement_action_from_row)
        .collect())
}

pub(crate) async fn fetch_creator_enforcement_action_by_id(
    pool: &SqlitePool,
    action_id: &str,
) -> AppResult<CreatorEnforcementAction> {
    reconcile_expired_creator_enforcement_actions_for_read(pool, None, Some(action_id)).await?;
    fetch_creator_enforcement_action_by_id_raw(pool, action_id).await
}

pub(crate) async fn fetch_creator_enforcement_action_by_id_raw(
    pool: &SqlitePool,
    action_id: &str,
) -> AppResult<CreatorEnforcementAction> {
    let row = sqlx::query(
        r#"
        SELECT id, creator_id, scope, state, reason, resolution_note, created_by_user_id,
               released_by_user_id, created_at, released_at, expires_at
        FROM creator_enforcement_actions
        WHERE id = ?
        "#,
    )
    .bind(action_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(creator_enforcement_action_from_row(row))
}
