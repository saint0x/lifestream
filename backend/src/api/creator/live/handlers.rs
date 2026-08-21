use super::*;
use crate::api::moderation::validate_live_delivery_class;

pub(crate) fn routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/api/v1/creator/me/live",
            get(get_creator_live).patch(update_creator_live),
        )
        .route(
            "/api/v1/creator/me/live/control",
            get(get_creator_live_control),
        )
        .route(
            "/api/v1/creator/me/live/runtime",
            get(get_creator_live_runtime),
        )
        .route(
            "/api/v1/creator/me/live/socket-sessions/:socket_id",
            get(get_creator_live_socket_session),
        )
        .route(
            "/api/v1/creator/me/live/socket-sessions/:socket_id/reconcile",
            post(reconcile_creator_live_socket_session),
        )
        .route(
            "/api/v1/creator/me/live/settings",
            get(get_creator_live_settings).patch(update_creator_live_settings),
        )
        .route(
            "/api/v1/creator/me/live/health",
            get(get_creator_live_health),
        )
}

pub(crate) async fn get_creator_live(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorLiveSnapshot>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        build_creator_live_snapshot(&state.pool, creator_id).await?,
    ))
}

async fn get_creator_live_control(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorLiveControlResponse>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_authoritative_creator_live_control_response(&state, creator_id).await?,
    ))
}

async fn get_creator_live_runtime(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorLiveRuntimeResponse>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_authoritative_creator_live_runtime_response(&state, creator_id).await?,
    ))
}

pub(crate) async fn get_creator_live_socket_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(socket_id): Path<String>,
) -> AppResult<Json<CreatorLiveSocketPresence>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_creator_live_socket_presence_by_id_raw(&state.pool, creator_id, &socket_id).await?,
    ))
}

pub(crate) async fn reconcile_creator_live_socket_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(socket_id): Path<String>,
) -> AppResult<Json<CreatorLiveSocketPresenceReconciliationReport>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let socket_session =
        fetch_creator_live_socket_presence_by_id_raw(&state.pool, creator_id, &socket_id).await?;
    if socket_session.creator_id != creator_id {
        return Err(AppError::NotFound);
    }
    Ok(Json(
        reconcile_single_creator_live_socket_session(state, creator_id, &socket_id).await?,
    ))
}

async fn get_creator_live_settings(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorLiveSettings>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_creator_live_settings(&state.pool, creator_id).await?,
    ))
}

async fn update_creator_live_settings(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<UpdateCreatorLiveSettingsRequest>,
) -> AppResult<Json<CreatorLiveSettings>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-live-settings:{}", identity.user_id),
        30,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    ensure_creator_live_settings_row(&state.pool, creator_id).await?;
    let current = fetch_creator_live_settings(&state.pool, creator_id).await?;
    if let Some(value) = input.slow_mode_seconds {
        validate_slow_mode_seconds(value)?;
    }
    if let Some(value) = input.auto_mod_level.as_deref() {
        validate_auto_mod_level(value)?;
    }
    if let Some(value) = input.delivery_class.as_deref() {
        validate_live_delivery_class(value)?;
    }

    let scenes = input.scenes.unwrap_or(current.scenes);
    let active_scene_id = input
        .active_scene_id
        .unwrap_or_else(|| current.active_scene_id.clone());

    sqlx::query(
        r#"
        UPDATE creator_live_settings
        SET subscriber_only = ?, slow_mode_seconds = ?, auto_mod_level = ?,
            notify_followers_default = ?, delivery_class = ?, active_scene_id = ?, scenes_json = ?
        WHERE creator_id = ?
        "#,
    )
    .bind(input.subscriber_only.unwrap_or(current.subscriber_only) as i64)
    .bind(input.slow_mode_seconds.unwrap_or(current.slow_mode_seconds))
    .bind(
        input
            .auto_mod_level
            .as_deref()
            .unwrap_or(current.auto_mod_level.as_str()),
    )
    .bind(
        input
            .notify_followers_default
            .unwrap_or(current.notify_followers_default) as i64,
    )
    .bind(
        input
            .delivery_class
            .as_deref()
            .unwrap_or(current.delivery_class.as_str()),
    )
    .bind(&active_scene_id)
    .bind(to_json(&scenes)?)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;

    let settings = fetch_creator_live_settings(&state.pool, creator_id).await?;
    publish_current_creator_live_state(&state, creator_id).await?;
    Ok(Json(settings))
}

async fn get_creator_live_health(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorLiveHealth>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_creator_live_health(&state.pool, creator_id).await?,
    ))
}
