use super::*;

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

pub(crate) async fn enable_live_notify(
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

pub(crate) async fn report_live_stream(
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

pub(crate) async fn post_chat_message(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
    Json(input): Json<ChatInput>,
) -> AppResult<Json<ChatMessage>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let persisted = persist_chat_message(&state, &stream_id, &identity, input).await?;
    Ok(Json(persisted.message))
}
