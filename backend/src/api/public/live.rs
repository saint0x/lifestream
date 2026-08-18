use super::super::discovery::{
    fetch_categories, fetch_live_stream_by_id, fetch_live_stream_by_slug, fetch_live_streams,
    fetch_user, sort_live_streams,
};
use super::super::moderation::{
    authorize_live_stream_moderation, authorize_live_stream_owner, fetch_creator_moderator,
    fetch_creator_moderators, fetch_live_moderation_action_by_id,
    fetch_live_moderation_action_by_id_raw, fetch_live_moderation_actions,
    fetch_live_stream_owner_creator_id, fetch_live_stream_report_by_id, fetch_live_stream_reports,
    fetch_moderation_audit_log, validate_creator_moderator_role,
    validate_live_moderation_action_type, validate_live_moderation_subject,
    validate_live_report_status, write_moderation_audit_entry,
};
use super::super::realtime::persist_chat_message;
use super::*;
use serde::Deserialize;

pub(crate) async fn list_live_streams(
    State(state): State<SharedState>,
) -> AppResult<Json<Vec<LiveStream>>> {
    Ok(Json(fetch_live_streams(&state.pool, None).await?))
}

pub(super) async fn get_live_stream(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> AppResult<Json<LiveStream>> {
    Ok(Json(fetch_live_stream_by_slug(&state.pool, &slug).await?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LiveDiscoveryQuery {
    category: Option<String>,
    sort: Option<String>,
    limit: Option<i64>,
}

pub(super) async fn get_live_discovery(
    State(state): State<SharedState>,
    Query(query): Query<LiveDiscoveryQuery>,
) -> AppResult<Json<LiveDiscoveryResponse>> {
    let categories = fetch_categories(&state.pool).await?;
    let active_category = match query.category.as_deref() {
        Some("all") | None => None,
        Some(category_name) => {
            if categories.iter().any(|item| item.name == category_name) {
                Some(category_name.to_string())
            } else {
                return Err(AppError::BadRequest(
                    "unknown live category filter".to_string(),
                ));
            }
        }
    };
    let active_sort = match query.sort.as_deref().unwrap_or("viewers") {
        "viewers" | "newest" => query.sort.unwrap_or_else(|| "viewers".to_string()),
        _ => {
            return Err(AppError::BadRequest(
                "sort must be either 'viewers' or 'newest'".to_string(),
            ));
        }
    };

    let limit = query.limit.unwrap_or(200).clamp(1, 500) as usize;
    let mut streams = fetch_live_streams(&state.pool, None).await?;
    let total_viewers = streams.iter().map(|stream| stream.viewers).sum();
    let total_channels = streams.len() as i64;
    if let Some(category_name) = active_category.as_deref() {
        streams.retain(|stream| stream.category == category_name);
    }
    sort_live_streams(&mut streams, &active_sort);
    if streams.len() > limit {
        streams.truncate(limit);
    }

    Ok(Json(LiveDiscoveryResponse {
        streams,
        categories,
        total_viewers,
        total_channels,
        active_category,
        active_sort,
    }))
}

#[derive(Deserialize)]
pub(crate) struct LimitQuery {
    pub(crate) limit: Option<i64>,
    pub(crate) after_seq: Option<i64>,
}

pub(crate) async fn list_chat_messages(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
    Query(query): Query<LimitQuery>,
) -> AppResult<Json<Vec<ChatMessage>>> {
    let maybe_identity = optional_identity(&state.pool, &headers).await?;
    ensure_stream_exists(&state.pool, &stream_id).await?;
    Ok(Json(
        fetch_chat_messages_for_viewer(
            &state.pool,
            &stream_id,
            maybe_identity
                .as_ref()
                .map(|identity| identity.user_id.as_str()),
            query.limit.unwrap_or(100),
            query.after_seq,
        )
        .await?,
    ))
}

#[derive(Debug)]
pub(crate) struct PersistedChatMessage {
    pub(crate) message: ChatMessage,
    pub(crate) hidden_by_moderation: bool,
}

pub(super) async fn enable_live_notify(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
) -> AppResult<Json<LiveNotifyPreference>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let stream = fetch_live_stream_by_id(&state.pool, &stream_id).await?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO live_stream_notification_preferences (user_id, streamer_id, enabled, created_at)
        VALUES (?, ?, 1, ?)
        ON CONFLICT(user_id, streamer_id) DO UPDATE SET enabled = 1
        "#,
    )
    .bind(&identity.user_id)
    .bind(&stream.streamer.id)
    .bind(&now)
    .execute(&state.pool)
    .await?;

    Ok(Json(LiveNotifyPreference {
        streamer_id: stream.streamer.id,
        enabled: true,
    }))
}

pub(crate) async fn create_clip_request(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
) -> AppResult<StatusCode> {
    let identity = require_identity(&state.pool, &headers).await?;
    ensure_stream_exists(&state.pool, &stream_id).await?;
    let now = Utc::now();
    let now_rfc3339 = now.to_rfc3339();
    let clip_dedupe_after = (now - chrono::Duration::seconds(30)).to_rfc3339();
    let existing = sqlx::query(
        r#"
        SELECT id
        FROM live_stream_clip_requests
        WHERE stream_id = ?
          AND user_id = ?
          AND created_at >= ?
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(&stream_id)
    .bind(&identity.user_id)
    .bind(&clip_dedupe_after)
    .fetch_optional(&state.pool)
    .await?;
    if existing.is_none() {
        sqlx::query(
            "INSERT INTO live_stream_clip_requests (id, stream_id, user_id, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&stream_id)
        .bind(&identity.user_id)
        .bind(&now_rfc3339)
        .execute(&state.pool)
        .await?;
    }
    Ok(StatusCode::ACCEPTED)
}

pub(super) async fn report_live_stream(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
    Json(input): Json<LiveReportRequest>,
) -> AppResult<StatusCode> {
    let identity = require_identity(&state.pool, &headers).await?;
    let stream = fetch_live_stream_by_id(&state.pool, &stream_id).await?;
    if input.reason.trim().is_empty() {
        return Err(AppError::BadRequest("reason is required".to_string()));
    }
    let report_id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO live_stream_reports (id, stream_id, user_id, reason, details, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&report_id)
    .bind(&stream_id)
    .bind(&identity.user_id)
    .bind(input.reason.trim())
    .bind(input.details)
    .bind(&created_at)
    .execute(&state.pool)
    .await?;
    let reporter = fetch_user(&state.pool, &identity.user_id).await?;
    let creator_id = fetch_live_stream_owner_creator_id(&state.pool, &stream_id).await?;
    enqueue_notification_event(
        &state.pool,
        "live_report_received",
        &format!("{} reported {}.", reporter.display_name, stream.title),
        Some(&identity.user_id),
        Some(&reporter.display_name),
        Some(&creator_id),
        Some(&stream_id),
        None,
        json!({
            "reportId": report_id,
            "reason": input.reason.trim(),
        }),
        &[],
        &[creator_id.clone()],
    )
    .await?;
    Ok(StatusCode::ACCEPTED)
}

pub(super) async fn list_live_stream_moderators(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
) -> AppResult<Json<Vec<CreatorModerator>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = authorize_live_stream_moderation(&state.pool, &stream_id, &identity).await?;
    Ok(Json(
        fetch_creator_moderators(&state.pool, &creator_id).await?,
    ))
}

pub(super) async fn add_live_stream_moderator(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
    Json(input): Json<CreateCreatorModeratorRequest>,
) -> AppResult<Json<CreatorModerator>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = authorize_live_stream_owner(&state.pool, &stream_id, &identity).await?;
    fetch_user(&state.pool, &input.user_id).await?;
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
    .execute(&state.pool)
    .await?;
    write_moderation_audit_entry(
        &state.pool,
        &creator_id,
        Some(&stream_id),
        &identity.user_id,
        Some(&input.user_id),
        "moderator_added",
        json!({"role": input.role}),
    )
    .await?;
    Ok(Json(
        fetch_creator_moderator(&state.pool, &creator_id, &input.user_id).await?,
    ))
}

pub(crate) async fn remove_live_stream_moderator(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((stream_id, user_id)): Path<(String, String)>,
) -> AppResult<StatusCode> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = authorize_live_stream_owner(&state.pool, &stream_id, &identity).await?;
    let result = sqlx::query("DELETE FROM creator_moderators WHERE creator_id = ? AND user_id = ?")
        .bind(&creator_id)
        .bind(&user_id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    write_moderation_audit_entry(
        &state.pool,
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
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = authorize_live_stream_moderation(&state.pool, &stream_id, &identity).await?;
    Ok(Json(
        fetch_live_moderation_actions(&state.pool, &stream_id, &creator_id).await?,
    ))
}

pub(crate) async fn get_live_moderation_action(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((stream_id, action_id)): Path<(String, String)>,
) -> AppResult<Json<LiveModerationAction>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = authorize_live_stream_moderation(&state.pool, &stream_id, &identity).await?;
    let action = fetch_live_moderation_action_by_id_raw(&state.pool, &action_id).await?;
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
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = authorize_live_stream_moderation(&state.pool, &stream_id, &identity).await?;
    let action = fetch_live_moderation_action_by_id_raw(&state.pool, &action_id).await?;
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
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = authorize_live_stream_moderation(&state.pool, &stream_id, &identity).await?;
    let subject = fetch_user(&state.pool, &input.subject_user_id).await?;
    validate_live_moderation_action_type(&input.action_type)?;
    if input.reason.trim().is_empty() {
        return Err(AppError::BadRequest("reason is required".to_string()));
    }
    validate_live_moderation_subject(&state.pool, &stream_id, &creator_id, &identity, &subject.id)
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
    .execute(&state.pool)
    .await?;
    let action = fetch_live_moderation_action_by_id(&state.pool, &action_id).await?;
    write_moderation_audit_entry(
        &state.pool,
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
    let actor = fetch_user(&state.pool, &identity.user_id).await?;
    enqueue_notification_event(
        &state.pool,
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
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = authorize_live_stream_moderation(&state.pool, &stream_id, &identity).await?;
    let action = fetch_live_moderation_action_by_id(&state.pool, &action_id).await?;
    if action.stream_id != stream_id || action.creator_id != creator_id {
        return Err(AppError::NotFound);
    }
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE live_moderation_actions SET state = 'revoked', revoked_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(&action_id)
    .execute(&state.pool)
    .await?;
    write_moderation_audit_entry(
        &state.pool,
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
    let revoked = fetch_live_moderation_action_by_id(&state.pool, &action_id).await?;
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

pub(super) async fn list_live_stream_reports(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
) -> AppResult<Json<Vec<LiveStreamReportRecord>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    authorize_live_stream_moderation(&state.pool, &stream_id, &identity).await?;
    Ok(Json(
        fetch_live_stream_reports(&state.pool, &stream_id).await?,
    ))
}

pub(crate) async fn resolve_live_stream_report(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((stream_id, report_id)): Path<(String, String)>,
    Json(input): Json<ResolveLiveStreamReportRequest>,
) -> AppResult<Json<LiveStreamReportRecord>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = authorize_live_stream_moderation(&state.pool, &stream_id, &identity).await?;
    validate_live_report_status(&input.status)?;
    let report = fetch_live_stream_report_by_id(&state.pool, &report_id).await?;
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
    .execute(&state.pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    write_moderation_audit_entry(
        &state.pool,
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
        fetch_live_stream_report_by_id(&state.pool, &report_id).await?,
    ))
}

pub(super) async fn list_live_moderation_audit_log(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
) -> AppResult<Json<Vec<ModerationAuditEntry>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = authorize_live_stream_moderation(&state.pool, &stream_id, &identity).await?;
    Ok(Json(
        fetch_moderation_audit_log(&state.pool, &creator_id, Some(&stream_id)).await?,
    ))
}

pub(crate) async fn get_live_viewer_preview(
    State(state): State<SharedState>,
    Path(stream_id): Path<String>,
) -> AppResult<Json<ViewerPreview>> {
    ensure_stream_exists(&state.pool, &stream_id).await?;
    Ok(Json(ViewerPreview {
        total_viewers: effective_live_viewer_count(&state.pool, &stream_id).await?,
        sample_users: fetch_live_viewer_sample_users(&state.pool, &stream_id, 8).await?,
    }))
}

pub(super) async fn post_chat_message(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
    Json(input): Json<ChatInput>,
) -> AppResult<Json<ChatMessage>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let persisted = persist_chat_message(&state, &stream_id, &identity, input).await?;
    Ok(Json(persisted.message))
}
