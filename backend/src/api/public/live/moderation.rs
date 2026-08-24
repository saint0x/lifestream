use super::*;

pub(crate) async fn list_live_stream_moderators(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
) -> AppResult<Json<Vec<CreatorModerator>>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id =
        authorize_live_stream_moderation(state.db.try_sqlite_adapter()?, &stream_id, &identity)
            .await?;
    Ok(Json(
        fetch_creator_moderators(state.db.try_sqlite_adapter()?, &creator_id).await?,
    ))
}

pub(crate) async fn add_live_stream_moderator(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
    Json(input): Json<CreateCreatorModeratorRequest>,
) -> AppResult<Json<CreatorModerator>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id =
        authorize_live_stream_owner(state.db.try_sqlite_adapter()?, &stream_id, &identity).await?;
    fetch_user(state.db.try_sqlite_adapter()?, &input.user_id).await?;
    validate_creator_moderator_role(&input.role)?;
    let created_at = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO creator_moderators (creator_id, user_id, role, created_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(creator_id, user_id) DO UPDATE SET role = excluded.role
        "#,
    )
    .bind(&creator_id)
    .bind(&input.user_id)
    .bind(&input.role)
    .bind(&created_at)
    .execute(state.db.try_sqlite_adapter()?)
    .await?;
    write_moderation_audit_entry(
        state.db.try_sqlite_adapter()?,
        &creator_id,
        Some(&stream_id),
        &identity.user_id,
        Some(&input.user_id),
        "moderator_added",
        json!({"role": input.role}),
    )
    .await?;
    Ok(Json(
        fetch_creator_moderator(state.db.try_sqlite_adapter()?, &creator_id, &input.user_id)
            .await?,
    ))
}

pub(crate) async fn remove_live_stream_moderator(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((stream_id, user_id)): Path<(String, String)>,
) -> AppResult<StatusCode> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id =
        authorize_live_stream_owner(state.db.try_sqlite_adapter()?, &stream_id, &identity).await?;
    let result = sqlx::query("DELETE FROM creator_moderators WHERE creator_id = ? AND user_id = ?")
        .bind(&creator_id)
        .bind(&user_id)
        .execute(state.db.try_sqlite_adapter()?)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    write_moderation_audit_entry(
        state.db.try_sqlite_adapter()?,
        &creator_id,
        Some(&stream_id),
        &identity.user_id,
        Some(&user_id),
        "moderator_removed",
        json!({}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn list_live_moderation_actions(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
) -> AppResult<Json<Vec<LiveModerationAction>>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id =
        authorize_live_stream_moderation(state.db.try_sqlite_adapter()?, &stream_id, &identity)
            .await?;
    Ok(Json(
        fetch_live_moderation_actions(state.db.try_sqlite_adapter()?, &stream_id, &creator_id)
            .await?,
    ))
}

pub(crate) async fn get_live_moderation_action(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((stream_id, action_id)): Path<(String, String)>,
) -> AppResult<Json<LiveModerationAction>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id =
        authorize_live_stream_moderation(state.db.try_sqlite_adapter()?, &stream_id, &identity)
            .await?;
    let action =
        fetch_live_moderation_action_by_id_raw(state.db.try_sqlite_adapter()?, &action_id).await?;
    if action.stream_id != stream_id || action.creator_id != creator_id {
        return Err(AppError::NotFound);
    }
    Ok(Json(action))
}

pub(crate) async fn reconcile_live_moderation_action(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((stream_id, action_id)): Path<(String, String)>,
) -> AppResult<Json<LiveModerationReconciliationReport>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id =
        authorize_live_stream_moderation(state.db.try_sqlite_adapter()?, &stream_id, &identity)
            .await?;
    let action =
        fetch_live_moderation_action_by_id_raw(state.db.try_sqlite_adapter()?, &action_id).await?;
    if action.stream_id != stream_id || action.creator_id != creator_id {
        return Err(AppError::NotFound);
    }
    Ok(Json(
        reconcile_single_live_moderation_action(state, &action_id).await?,
    ))
}

pub(crate) async fn create_live_moderation_action(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
    Json(input): Json<CreateLiveModerationActionRequest>,
) -> AppResult<Json<LiveModerationAction>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id =
        authorize_live_stream_moderation(state.db.try_sqlite_adapter()?, &stream_id, &identity)
            .await?;
    let subject = fetch_user(state.db.try_sqlite_adapter()?, &input.subject_user_id).await?;
    validate_live_moderation_action_type(&input.action_type)?;
    if input.reason.trim().is_empty() {
        return Err(AppError::BadRequest("reason is required".to_string()));
    }
    validate_live_moderation_subject(
        state.db.try_sqlite_adapter()?,
        &stream_id,
        &creator_id,
        &identity,
        &subject.id,
    )
    .await?;
    let now = Utc::now();
    let action_id = format!("lma-{}", Uuid::new_v4().simple());
    let created_at = now.to_rfc3339();
    let expires_at = input.duration_minutes.map(|minutes| {
        (now + chrono::Duration::minutes(minutes.clamp(1, 60 * 24 * 30))).to_rfc3339()
    });
    sqlx::query(
        r#"
        INSERT INTO live_moderation_actions (
            id, stream_id, creator_id, subject_user_id, actor_user_id, action_type, reason,
            state, expires_at, created_at, revoked_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, NULL)
        "#,
    )
    .bind(&action_id)
    .bind(&stream_id)
    .bind(&creator_id)
    .bind(&input.subject_user_id)
    .bind(&identity.user_id)
    .bind(&input.action_type)
    .bind(input.reason.trim())
    .bind(&expires_at)
    .bind(&created_at)
    .execute(state.db.try_sqlite_adapter()?)
    .await?;
    let action =
        fetch_live_moderation_action_by_id(state.db.try_sqlite_adapter()?, &action_id).await?;
    write_moderation_audit_entry(
        state.db.try_sqlite_adapter()?,
        &creator_id,
        Some(&stream_id),
        &identity.user_id,
        Some(&input.subject_user_id),
        "moderation_action_created",
        json!({
            "actionId": action_id,
            "actionType": input.action_type,
            "reason": input.reason.trim(),
            "expiresAt": expires_at,
        }),
    )
    .await?;
    let actor = fetch_user(state.db.try_sqlite_adapter()?, &identity.user_id).await?;
    enqueue_notification_event(
        state.db.try_sqlite_adapter()?,
        "moderation_action",
        &format!(
            "{} applied a moderation action to your live chat access.",
            actor.display_name
        ),
        Some(&identity.user_id),
        Some(&actor.display_name),
        Some(&creator_id),
        Some(&stream_id),
        None,
        json!({
            "actionId": action_id,
            "actionType": input.action_type,
            "reason": input.reason.trim(),
            "expiresAt": expires_at,
        }),
        &[input.subject_user_id.clone()],
        &[],
    )
    .await?;
    state
        .realtime
        .publish(
            &stream_channel_id(&stream_id),
            WsEvent::ModerationAction {
                action: action.clone(),
            },
        )
        .await;
    Ok(Json(action))
}

pub(crate) async fn revoke_live_moderation_action(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((stream_id, action_id)): Path<(String, String)>,
) -> AppResult<Json<LiveModerationAction>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id =
        authorize_live_stream_moderation(state.db.try_sqlite_adapter()?, &stream_id, &identity)
            .await?;
    let action =
        fetch_live_moderation_action_by_id(state.db.try_sqlite_adapter()?, &action_id).await?;
    if action.stream_id != stream_id || action.creator_id != creator_id {
        return Err(AppError::NotFound);
    }
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE live_moderation_actions SET state = 'revoked', revoked_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(&action_id)
    .execute(state.db.try_sqlite_adapter()?)
    .await?;
    write_moderation_audit_entry(
        state.db.try_sqlite_adapter()?,
        &creator_id,
        Some(&stream_id),
        &identity.user_id,
        Some(&action.subject_user_id),
        "moderation_action_revoked",
        json!({
            "actionId": action_id,
            "revokedAt": now,
        }),
    )
    .await?;
    let revoked =
        fetch_live_moderation_action_by_id(state.db.try_sqlite_adapter()?, &action_id).await?;
    state
        .realtime
        .publish(
            &stream_channel_id(&stream_id),
            WsEvent::ModerationAction {
                action: revoked.clone(),
            },
        )
        .await;
    Ok(Json(revoked))
}

pub(crate) async fn list_live_stream_reports(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
) -> AppResult<Json<Vec<LiveStreamReportRecord>>> {
    let identity = require_identity(&state.db, &headers).await?;
    authorize_live_stream_moderation(state.db.try_sqlite_adapter()?, &stream_id, &identity).await?;
    Ok(Json(
        fetch_live_stream_reports(state.db.try_sqlite_adapter()?, &stream_id).await?,
    ))
}

pub(crate) async fn resolve_live_stream_report(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((stream_id, report_id)): Path<(String, String)>,
    Json(input): Json<ResolveLiveStreamReportRequest>,
) -> AppResult<Json<LiveStreamReportRecord>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id =
        authorize_live_stream_moderation(state.db.try_sqlite_adapter()?, &stream_id, &identity)
            .await?;
    validate_live_report_status(&input.status)?;
    let report = fetch_live_stream_report_by_id(state.db.try_sqlite_adapter()?, &report_id).await?;
    if report.stream_id != stream_id {
        return Err(AppError::NotFound);
    }
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        r#"
        UPDATE live_stream_reports
        SET status = ?, resolved_by_user_id = ?, resolution_note = ?, resolved_at = ?
        WHERE id = ? AND stream_id = ?
        "#,
    )
    .bind(&input.status)
    .bind(&identity.user_id)
    .bind(&input.resolution_note)
    .bind(&now)
    .bind(&report_id)
    .bind(&stream_id)
    .execute(state.db.try_sqlite_adapter()?)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    write_moderation_audit_entry(
        state.db.try_sqlite_adapter()?,
        &creator_id,
        Some(&stream_id),
        &identity.user_id,
        None,
        "report_resolved",
        json!({
            "reportId": report_id,
            "status": input.status,
            "resolutionNote": input.resolution_note,
        }),
    )
    .await?;
    Ok(Json(
        fetch_live_stream_report_by_id(state.db.try_sqlite_adapter()?, &report_id).await?,
    ))
}

pub(crate) async fn list_live_moderation_audit_log(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
) -> AppResult<Json<Vec<ModerationAuditEntry>>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id =
        authorize_live_stream_moderation(state.db.try_sqlite_adapter()?, &stream_id, &identity)
            .await?;
    Ok(Json(
        fetch_moderation_audit_log(
            state.db.try_sqlite_adapter()?,
            &creator_id,
            Some(&stream_id),
        )
        .await?,
    ))
}
