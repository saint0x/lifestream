use super::*;

pub(super) async fn get_admin_creator_enforcement_state(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(creator_id): Path<String>,
) -> AppResult<Json<CreatorEnforcementState>> {
    let identity = require_identity(&state.db, &headers).await?;
    identity.require_admin_scope()?;
    let profile = fetch_creator_profile(state.db.try_sqlite_adapter()?, &creator_id).await?;
    Ok(Json(
        fetch_creator_enforcement_state(state.db.try_sqlite_adapter()?, &profile).await?,
    ))
}

pub(super) async fn create_admin_creator_enforcement_action(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(creator_id): Path<String>,
    Json(input): Json<CreateCreatorEnforcementActionRequest>,
) -> AppResult<Json<CreatorEnforcementAction>> {
    let identity = require_identity(&state.db, &headers).await?;
    identity.require_admin_scope()?;
    let profile = fetch_creator_profile(state.db.try_sqlite_adapter()?, &creator_id).await?;
    validate_creator_enforcement_scope(&input.scope)?;
    if input.reason.trim().is_empty() {
        return Err(AppError::BadRequest("reason is required".to_string()));
    }

    let expires_at = parse_optional_future_timestamp(input.expires_at.as_deref())?;
    let now = Utc::now().to_rfc3339();
    let action_id = format!("cea-{}", Uuid::new_v4().simple());

    sqlx::query(
        r#"
        INSERT INTO creator_enforcement_actions (
            id, creator_id, scope, state, reason, resolution_note, created_by_user_id,
            released_by_user_id, created_at, released_at, expires_at
        ) VALUES (?, ?, ?, 'active', ?, NULL, ?, NULL, ?, NULL, ?)
        "#,
    )
    .bind(&action_id)
    .bind(&creator_id)
    .bind(input.scope.trim())
    .bind(input.reason.trim())
    .bind(&identity.user_id)
    .bind(&now)
    .bind(expires_at.as_deref())
    .execute(state.db.try_sqlite_adapter()?)
    .await?;

    write_moderation_audit_entry(
        state.db.try_sqlite_adapter()?,
        &creator_id,
        None,
        &identity.user_id,
        Some(&profile.user_id),
        "creator_enforcement_applied",
        json!({
            "actionId": action_id,
            "scope": input.scope.trim(),
            "reason": input.reason.trim(),
            "expiresAt": expires_at,
        }),
    )
    .await?;
    enqueue_notification_event(
        state.db.try_sqlite_adapter()?,
        "creator_enforcement_applied",
        &format!(
            "A creator enforcement action was applied to {}.",
            profile.display_name
        ),
        Some(&identity.user_id),
        Some("operator"),
        Some(&creator_id),
        None,
        None,
        json!({
            "actionId": action_id,
            "scope": input.scope.trim(),
            "reason": input.reason.trim(),
            "expiresAt": expires_at,
        }),
        &[],
        &[creator_id.clone()],
    )
    .await?;

    Ok(Json(
        fetch_creator_enforcement_action_by_id(state.db.try_sqlite_adapter()?, &action_id).await?,
    ))
}

pub(crate) async fn get_admin_creator_enforcement_action(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((creator_id, action_id)): Path<(String, String)>,
) -> AppResult<Json<CreatorEnforcementAction>> {
    let identity = require_identity(&state.db, &headers).await?;
    identity.require_admin_scope()?;
    let action =
        fetch_creator_enforcement_action_by_id_raw(state.db.try_sqlite_adapter()?, &action_id)
            .await?;
    if action.creator_id != creator_id {
        return Err(AppError::NotFound);
    }
    Ok(Json(action))
}

pub(crate) async fn reconcile_admin_creator_enforcement_action(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((creator_id, action_id)): Path<(String, String)>,
) -> AppResult<Json<CreatorEnforcementReconciliationReport>> {
    let identity = require_identity(&state.db, &headers).await?;
    identity.require_admin_scope()?;
    let action =
        fetch_creator_enforcement_action_by_id_raw(state.db.try_sqlite_adapter()?, &action_id)
            .await?;
    if action.creator_id != creator_id {
        return Err(AppError::NotFound);
    }
    Ok(Json(
        reconcile_single_creator_enforcement_action(state, &action_id).await?,
    ))
}

pub(super) async fn release_admin_creator_enforcement_action(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((creator_id, action_id)): Path<(String, String)>,
    Json(input): Json<ReleaseCreatorEnforcementActionRequest>,
) -> AppResult<Json<CreatorEnforcementAction>> {
    let identity = require_identity(&state.db, &headers).await?;
    identity.require_admin_scope()?;
    let profile = fetch_creator_profile(state.db.try_sqlite_adapter()?, &creator_id).await?;
    let action =
        fetch_creator_enforcement_action_by_id(state.db.try_sqlite_adapter()?, &action_id).await?;
    if action.creator_id != creator_id {
        return Err(AppError::NotFound);
    }
    if action.state != "active" {
        return Ok(Json(action));
    }
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        UPDATE creator_enforcement_actions
        SET state = 'released', resolution_note = ?, released_by_user_id = ?, released_at = ?
        WHERE id = ? AND creator_id = ?
        "#,
    )
    .bind(input.resolution_note.as_deref())
    .bind(&identity.user_id)
    .bind(&now)
    .bind(&action_id)
    .bind(&creator_id)
    .execute(state.db.try_sqlite_adapter()?)
    .await?;

    write_moderation_audit_entry(
        state.db.try_sqlite_adapter()?,
        &creator_id,
        None,
        &identity.user_id,
        Some(&profile.user_id),
        "creator_enforcement_released",
        json!({
            "actionId": action_id,
            "scope": action.scope,
            "resolutionNote": input.resolution_note,
            "releasedAt": now,
        }),
    )
    .await?;
    enqueue_notification_event(
        state.db.try_sqlite_adapter()?,
        "creator_enforcement_released",
        &format!(
            "A creator enforcement action was released for {}.",
            profile.display_name
        ),
        Some(&identity.user_id),
        Some("operator"),
        Some(&creator_id),
        None,
        None,
        json!({
            "actionId": action_id,
            "scope": action.scope,
            "resolutionNote": input.resolution_note,
            "releasedAt": now,
        }),
        &[],
        &[creator_id.clone()],
    )
    .await?;

    Ok(Json(
        fetch_creator_enforcement_action_by_id(state.db.try_sqlite_adapter()?, &action_id).await?,
    ))
}
