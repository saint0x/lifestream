use super::discovery::fetch_live_stream_by_id;
use super::live_ingest::update_creator_live;
use super::moderation::{validate_auto_mod_level, validate_slow_mode_seconds};
use super::*;

pub(super) fn routes() -> Router<SharedState> {
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

pub(super) async fn get_creator_live(
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

pub(super) async fn get_creator_live_socket_session(
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

pub(super) async fn reconcile_creator_live_socket_session(
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
    let current = fetch_creator_live_settings(&state.pool, creator_id).await?;
    if let Some(value) = input.slow_mode_seconds {
        validate_slow_mode_seconds(value)?;
    }
    if let Some(value) = input.auto_mod_level.as_deref() {
        validate_auto_mod_level(value)?;
    }

    let scenes = input.scenes.unwrap_or(current.scenes);
    let active_scene_id = input
        .active_scene_id
        .unwrap_or_else(|| current.active_scene_id.clone());

    sqlx::query(
        r#"
        UPDATE creator_live_settings
        SET subscriber_only = ?, slow_mode_seconds = ?, auto_mod_level = ?,
            notify_followers_default = ?, active_scene_id = ?, scenes_json = ?
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
    .bind(&active_scene_id)
    .bind(to_json(&scenes)?)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;

    let settings = fetch_creator_live_settings(&state.pool, creator_id).await?;
    publish_creator_live_state(&state, creator_id).await?;
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

pub(super) fn creator_live_channel_id(creator_id: &str) -> String {
    format!("creator-live:{creator_id}")
}

pub(super) async fn fetch_creator_live_control_response(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorLiveControlResponse> {
    reconcile_stale_creator_live_socket_sessions_for_read(pool, Some(creator_id), None).await?;
    let snapshot = build_creator_live_snapshot(pool, creator_id).await?;
    let settings = fetch_creator_live_settings(pool, creator_id).await?;
    let health = fetch_creator_live_health(pool, creator_id).await?;
    let collaboration =
        fetch_creator_live_collaboration_summary(pool, creator_id, &snapshot).await?;
    let subscriber_tiers = fetch_creator_subscriber_tiers(pool, creator_id).await?;
    let viewer_history = health.samples.iter().map(|sample| sample.viewers).collect();
    let bitrate_history = health
        .samples
        .iter()
        .map(|sample| sample.bitrate_kbps)
        .collect();
    let current_viewers = if let Some(session) = snapshot.ingest_session.as_ref() {
        session.viewers
    } else if snapshot.current_broadcast.is_some() {
        if let Some(viewers) = health.samples.last().map(|sample| sample.viewers) {
            viewers
        } else {
            fetch_live_stream_by_id(pool, &format!("lv-{}-live", snapshot.profile.handle))
                .await
                .map(|stream| stream.viewers)
                .unwrap_or(0)
        }
    } else {
        0
    };

    Ok(CreatorLiveControlResponse {
        is_live: snapshot.current_broadcast.is_some(),
        current_viewers,
        snapshot,
        settings,
        health,
        collaboration,
        subscriber_tiers,
        viewer_history,
        bitrate_history,
    })
}

pub(super) async fn fetch_creator_live_runtime_response(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorLiveRuntimeResponse> {
    reconcile_stale_creator_live_socket_sessions_for_read(pool, Some(creator_id), None).await?;
    let snapshot = build_creator_live_snapshot(pool, creator_id).await?;
    let health = fetch_creator_live_health(pool, creator_id).await?;
    let collaboration =
        fetch_creator_live_collaboration_summary(pool, creator_id, &snapshot).await?;

    Ok(CreatorLiveRuntimeResponse {
        snapshot,
        health,
        collaboration,
        active_session: fetch_active_live_ingest_session(pool, creator_id).await?,
        recent_sessions: fetch_recent_live_ingest_sessions(pool, creator_id, 10).await?,
        recent_events: fetch_live_ingest_events_for_creator(pool, creator_id, 25).await?,
    })
}

pub(super) async fn fetch_authoritative_creator_live_control_response(
    state: &SharedState,
    creator_id: &str,
) -> AppResult<CreatorLiveControlResponse> {
    reconcile_collaboration_expiry_for_host_read(state, creator_id).await?;
    fetch_creator_live_control_response(&state.pool, creator_id).await
}

pub(super) async fn fetch_authoritative_creator_live_runtime_response(
    state: &SharedState,
    creator_id: &str,
) -> AppResult<CreatorLiveRuntimeResponse> {
    reconcile_collaboration_expiry_for_host_read(state, creator_id).await?;
    fetch_creator_live_runtime_response(&state.pool, creator_id).await
}

pub(super) async fn publish_raw_creator_live_state(
    state: &SharedState,
    creator_id: &str,
) -> AppResult<()> {
    let event = WsEvent::CreatorLiveState {
        control: fetch_creator_live_control_response(&state.pool, creator_id).await?,
        runtime: fetch_creator_live_runtime_response(&state.pool, creator_id).await?,
    };
    state
        .realtime
        .publish(&creator_live_channel_id(creator_id), event)
        .await;
    Ok(())
}

pub(super) async fn publish_authoritative_creator_live_state(
    state: &SharedState,
    creator_id: &str,
) -> AppResult<()> {
    let event = WsEvent::CreatorLiveState {
        control: fetch_authoritative_creator_live_control_response(state, creator_id).await?,
        runtime: fetch_authoritative_creator_live_runtime_response(state, creator_id).await?,
    };
    state
        .realtime
        .publish(&creator_live_channel_id(creator_id), event)
        .await;
    Ok(())
}

pub(super) async fn publish_creator_live_state(
    state: &SharedState,
    creator_id: &str,
) -> AppResult<()> {
    publish_authoritative_creator_live_state(state, creator_id).await
}

pub(super) async fn fetch_creator_live_socket_presence_by_id_raw(
    pool: &SqlitePool,
    creator_id: &str,
    socket_id: &str,
) -> AppResult<CreatorLiveSocketPresence> {
    let cutoff = active_presence_cutoff();
    let row = sqlx::query(
        r#"
        SELECT id, creator_id, user_id, connected_at, last_seen_at, disconnected_at
        FROM creator_live_socket_sessions
        WHERE creator_id = ? AND id = ?
        "#,
    )
    .bind(creator_id)
    .bind(socket_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let last_seen_at: String = row.get("last_seen_at");
    let disconnected_at: Option<String> = row.get("disconnected_at");
    Ok(CreatorLiveSocketPresence {
        id: row.get("id"),
        creator_id: row.get("creator_id"),
        user_id: row.get("user_id"),
        connected_at: row.get("connected_at"),
        last_seen_at: last_seen_at.clone(),
        disconnected_at,
        is_stale: last_seen_at < cutoff,
    })
}

pub(super) async fn build_creator_live_snapshot(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorLiveSnapshot> {
    if let Some(session) = fetch_active_live_ingest_session(pool, creator_id).await? {
        if is_live_ingest_session_stale(&session) {
            mark_live_ingest_session_stale_in_db(pool, &session).await?;
        }
    }
    let broadcasts = fetch_broadcasts(pool, creator_id).await?;
    let profile = normalize_creator_live_profile(pool, creator_id, broadcasts.clone()).await?;
    let current_broadcast = broadcasts
        .iter()
        .find(|item| item.status == "live")
        .cloned();
    let pending_broadcast = broadcasts
        .iter()
        .find(|item| item.status == "ready")
        .cloned();
    let ingest_session = fetch_active_live_ingest_session(pool, creator_id).await?;
    Ok(CreatorLiveSnapshot {
        profile: contract_creator_profile(profile),
        current_broadcast: current_broadcast.map(contract_broadcast),
        pending_broadcast: pending_broadcast.map(contract_broadcast),
        ingest_session,
    })
}

pub(super) fn contract_live_status(status: &str) -> String {
    match status {
        "ready" => "starting".to_string(),
        other => other.to_string(),
    }
}

pub(super) fn contract_broadcast_status(status: &str) -> String {
    match status {
        "ready" => "scheduled".to_string(),
        other => other.to_string(),
    }
}

pub(super) fn contract_creator_profile(mut profile: CreatorProfile) -> CreatorProfile {
    profile.live_status = contract_live_status(&profile.live_status);
    profile
}

pub(super) fn contract_broadcast(mut broadcast: Broadcast) -> Broadcast {
    broadcast.status = contract_broadcast_status(&broadcast.status);
    broadcast
}

pub(super) fn contract_broadcasts(broadcasts: Vec<Broadcast>) -> Vec<Broadcast> {
    broadcasts.into_iter().map(contract_broadcast).collect()
}

pub(super) async fn normalize_creator_live_profile(
    pool: &SqlitePool,
    creator_id: &str,
    broadcasts: Vec<Broadcast>,
) -> AppResult<CreatorProfile> {
    let mut profile = fetch_creator_profile(pool, creator_id).await?;
    let current_broadcast = broadcasts
        .iter()
        .find(|item| item.status == "live")
        .map(|item| item.id.clone());
    let pending_broadcast = broadcasts
        .iter()
        .find(|item| item.status == "ready")
        .map(|item| item.id.clone());
    let desired_current_broadcast_id = current_broadcast.or(pending_broadcast);
    let desired_live_status = if broadcasts.iter().any(|item| item.status == "live") {
        "live"
    } else if broadcasts.iter().any(|item| item.status == "ready") {
        "ready"
    } else {
        "offline"
    };

    if profile.current_broadcast_id != desired_current_broadcast_id
        || profile.live_status != desired_live_status
    {
        sqlx::query(
            "UPDATE creator_profiles SET live_status = ?, current_broadcast_id = ? WHERE id = ?",
        )
        .bind(desired_live_status)
        .bind(desired_current_broadcast_id.clone())
        .bind(creator_id)
        .execute(pool)
        .await?;
        profile = fetch_creator_profile(pool, creator_id).await?;
    }

    Ok(profile)
}
