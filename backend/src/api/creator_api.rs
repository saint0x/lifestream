use super::*;
use serde::Deserialize;

const API_KEY_SCOPES: &[&str] = &[
    "creator:read",
    "creator:profile:write",
    "creator:uploads:read",
    "creator:uploads:write",
    "creator:live:read",
    "creator:live:control",
    "creator:ads:read",
    "creator:ads:write",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateCreatorApiProfileRequest {
    display_name: Option<String>,
    avatar: Option<String>,
    banner: Option<String>,
    tagline: Option<String>,
    bio: Option<String>,
}

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/api/v1/me/api-keys",
            get(list_api_keys).post(create_api_key),
        )
        .route("/api/v1/me/api-keys/:id", delete(revoke_api_key))
        .route(
            "/api/v1/creator-api/profile",
            get(api_get_profile).patch(api_patch_profile),
        )
        .route("/api/v1/creator-api/dashboard", get(api_get_dashboard))
        .route(
            "/api/v1/creator-api/upload-jobs",
            get(api_list_upload_jobs).post(api_create_upload_job),
        )
        .route(
            "/api/v1/creator-api/upload-jobs/:id",
            get(api_get_upload_job).patch(api_update_upload_job),
        )
        .route(
            "/api/v1/creator-api/media-assets",
            get(api_list_media_assets),
        )
        .route(
            "/api/v1/creator-api/upload-jobs/:id/media-asset",
            get(api_get_media_asset_for_upload_job),
        )
        .route(
            "/api/v1/creator-api/live",
            get(api_get_live).patch(api_update_live),
        )
        .route("/api/v1/creator-api/live/health", get(api_get_live_health))
        .route(
            "/api/v1/creator-api/live/runtime",
            get(api_get_live_runtime),
        )
        .route(
            "/api/v1/creator-api/broadcasts/start",
            post(api_start_broadcast),
        )
        .route(
            "/api/v1/creator-api/broadcasts/:id/end",
            post(api_end_broadcast),
        )
        .route(
            "/api/v1/creator-api/stream-key/rotate",
            post(api_rotate_stream_key),
        )
}

async fn list_api_keys(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<CreatorApiKey>>> {
    let identity = require_identity(&state.db, &headers).await?;
    identity.require_creator_scope()?;
    Ok(Json(
        state
            .db
            .list_creator_api_keys_for_user(&identity.user_id)
            .await?,
    ))
}

async fn create_api_key(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<CreateCreatorApiKeyRequest>,
) -> AppResult<Json<CreatorApiKeyTokenResponse>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?.to_string();
    enforce_rate_limit(
        &state,
        &format!("creator-api-key-create:{}", identity.user_id),
        10,
        Duration::from_secs(60),
    )
    .await?;

    let name = input.name.trim();
    if name.is_empty() || name.len() > 80 {
        return Err(AppError::BadRequest(
            "name must be between 1 and 80 characters".to_string(),
        ));
    }
    let scopes = validate_requested_scopes(input.scopes)?;
    let expires_at = match input.expires_in_days {
        Some(days) if !(1..=3650).contains(&days) => {
            return Err(AppError::BadRequest(
                "expiresInDays must be between 1 and 3650".to_string(),
            ));
        }
        Some(days) => Some((Utc::now() + ChronoDuration::days(days)).to_rfc3339()),
        None => None,
    };

    let id = format!("cak-{}", Uuid::new_v4().simple());
    let prefix = Uuid::new_v4().simple().to_string()[..12].to_string();
    let secret = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let access_token = format!("vnta_live_{prefix}_{secret}");
    let key_hash = hash_api_key(&access_token);
    let now = Utc::now().to_rfc3339();
    state
        .db
        .insert_creator_api_key(crate::db::NewCreatorApiKey {
            id: &id,
            user_id: &identity.user_id,
            creator_id: &creator_id,
            name,
            key_prefix: &prefix,
            access_token: &access_token,
            key_hash: &key_hash,
            scopes: &scopes,
            created_at: &now,
            expires_at: expires_at.as_deref(),
        })
        .await?;
    Ok(Json(CreatorApiKeyTokenResponse {
        api_key: state
            .db
            .get_creator_api_key_for_user(&id, &identity.user_id)
            .await?,
        access_token,
    }))
}

async fn revoke_api_key(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let identity = require_identity(&state.db, &headers).await?;
    identity.require_creator_scope()?;
    let now = Utc::now().to_rfc3339();
    let rows = state
        .db
        .revoke_creator_api_key(&id, &identity.user_id, &now)
        .await?;
    if rows == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn api_get_profile(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorProfile>> {
    let identity = require_api_identity(&state, &headers, "creator:read").await?;
    Ok(Json(
        fetch_creator_profile_for_api(&state.db, &identity.creator_id).await?,
    ))
}

async fn api_patch_profile(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<UpdateCreatorApiProfileRequest>,
) -> AppResult<Json<CreatorProfile>> {
    let identity = require_api_identity(&state, &headers, "creator:profile:write").await?;
    update_creator_profile_for_api(&state.db, &identity.creator_id, input).await?;
    Ok(Json(
        fetch_creator_profile_for_api(&state.db, &identity.creator_id).await?,
    ))
}

async fn api_get_dashboard(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Value>> {
    let identity = require_api_identity(&state, &headers, "creator:read").await?;
    Ok(Json(json!({
        "profile": fetch_creator_profile_for_api(&state.db, &identity.creator_id).await?,
        "uploadJobs": crate::api::media::jobs::list_creator_upload_jobs(&state.db, &identity.creator_id).await?,
        "mediaAssets": crate::api::media::jobs::list_creator_media_assets(&state.db, &identity.creator_id).await.unwrap_or_default(),
        "live": fetch_live_state_for_api(&state.db, &identity.creator_id).await?,
    })))
}

async fn api_list_upload_jobs(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<UploadJob>>> {
    let identity = require_api_identity(&state, &headers, "creator:uploads:read").await?;
    Ok(Json(
        crate::api::media::jobs::list_creator_upload_jobs(&state.db, &identity.creator_id).await?,
    ))
}

async fn api_create_upload_job(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<CreateUploadJobRequest>,
) -> AppResult<Json<UploadJob>> {
    let identity = require_api_identity(&state, &headers, "creator:uploads:write").await?;
    Ok(Json(
        crate::api::media::jobs::create_creator_upload_job(&state.db, &identity.creator_id, input)
            .await?,
    ))
}

async fn api_get_upload_job(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<UploadJob>> {
    let identity = require_api_identity(&state, &headers, "creator:uploads:read").await?;
    Ok(Json(
        crate::api::media::jobs::get_creator_upload_job(&state.db, &identity.creator_id, &id)
            .await?,
    ))
}

async fn api_update_upload_job(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<UpdateUploadJobRequest>,
) -> AppResult<Json<UploadJob>> {
    let identity = require_api_identity(&state, &headers, "creator:uploads:write").await?;
    Ok(Json(
        crate::api::media::jobs::update_creator_upload_job(
            &state.db,
            &identity.creator_id,
            &id,
            input,
        )
        .await?,
    ))
}

async fn api_list_media_assets(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<MediaAsset>>> {
    let identity = require_api_identity(&state, &headers, "creator:uploads:read").await?;
    Ok(Json(
        crate::api::media::jobs::list_creator_media_assets(&state.db, &identity.creator_id).await?,
    ))
}

async fn api_get_media_asset_for_upload_job(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<MediaAsset>> {
    let identity = require_api_identity(&state, &headers, "creator:uploads:read").await?;
    Ok(Json(
        crate::api::media::jobs::get_creator_media_asset_for_upload_job(
            &state.db,
            &identity.creator_id,
            &id,
        )
        .await?,
    ))
}

async fn api_get_live(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Value>> {
    let identity = require_api_identity(&state, &headers, "creator:live:read").await?;
    Ok(Json(
        fetch_live_state_for_api(&state.db, &identity.creator_id).await?,
    ))
}

async fn api_update_live(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<UpdateLiveRequest>,
) -> AppResult<Json<Value>> {
    let identity = require_api_identity(&state, &headers, "creator:live:control").await?;
    update_live_for_api(&state.db, &identity.creator_id, input).await?;
    Ok(Json(
        fetch_live_state_for_api(&state.db, &identity.creator_id).await?,
    ))
}

async fn api_get_live_health(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Value>> {
    let identity = require_api_identity(&state, &headers, "creator:live:read").await?;
    Ok(Json(
        fetch_live_health_for_api(&state.db, &identity.creator_id).await?,
    ))
}

async fn api_get_live_runtime(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Value>> {
    let identity = require_api_identity(&state, &headers, "creator:live:read").await?;
    Ok(Json(
        fetch_live_runtime_for_api(&state.db, &identity.creator_id).await?,
    ))
}

async fn api_start_broadcast(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<StartBroadcastRequest>,
) -> AppResult<Json<Broadcast>> {
    let identity = require_api_identity(&state, &headers, "creator:live:control").await?;
    Ok(Json(
        start_broadcast_for_api(&state.db, &identity.creator_id, input).await?,
    ))
}

async fn api_end_broadcast(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<Broadcast>> {
    let identity = require_api_identity(&state, &headers, "creator:live:control").await?;
    Ok(Json(
        end_broadcast_for_api(&state.db, &identity.creator_id, &id).await?,
    ))
}

async fn api_rotate_stream_key(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorProfile>> {
    let identity = require_api_identity(&state, &headers, "creator:live:control").await?;
    let new_key = format!(
        "live_sk_{}{}",
        Uuid::new_v4().simple(),
        &Uuid::new_v4().simple().to_string()[..8]
    );
    state
        .db
        .update_creator_stream_key(&identity.creator_id, &new_key)
        .await?;
    Ok(Json(
        fetch_creator_profile_for_api(&state.db, &identity.creator_id).await?,
    ))
}

async fn require_api_identity(
    state: &SharedState,
    headers: &HeaderMap,
    required_scope: &str,
) -> AppResult<crate::db::CreatorApiKeyIdentity> {
    let token = extract_api_key_token(headers)?;
    let token_hash = hash_api_key(&token);
    let now = Utc::now().to_rfc3339();
    let identity = state
        .db
        .lookup_creator_api_key_identity(&token_hash, &now)
        .await?;
    if !identity.scopes.iter().any(|scope| scope == required_scope) {
        return Err(AppError::Forbidden);
    }
    enforce_rate_limit(
        state,
        &format!("creator-api-key:{}", identity.key_id),
        600,
        Duration::from_secs(60),
    )
    .await?;
    state
        .db
        .touch_creator_api_key(&identity.key_id, &now)
        .await?;
    Ok(identity)
}

fn extract_api_key_token(headers: &HeaderMap) -> AppResult<String> {
    if let Some(value) = headers.get("x-vanta-api-key") {
        let token = value.to_str().map_err(|_| AppError::Unauthorized)?.trim();
        if !token.is_empty() {
            return Ok(token.to_string());
        }
    }
    let token = crate::auth::extract_bearer_token(headers)?.ok_or(AppError::Unauthorized)?;
    if !token.starts_with("vnta_live_") && !token.starts_with("vnta_test_") {
        return Err(AppError::Unauthorized);
    }
    Ok(token)
}

fn hash_api_key(token: &str) -> String {
    crate::auth::hash_token_with_secret(
        token,
        std::env::var("VANTA_API_KEY_HASH_SECRET")
            .ok()
            .or_else(|| std::env::var("VANTA_TOKEN_HASH_SECRET").ok())
            .as_deref(),
    )
}

fn validate_requested_scopes(input: Option<Vec<String>>) -> AppResult<Vec<String>> {
    let scopes = input.unwrap_or_else(|| {
        API_KEY_SCOPES
            .iter()
            .map(|scope| scope.to_string())
            .collect()
    });
    if scopes.is_empty() {
        return Err(AppError::BadRequest(
            "api key must include at least one scope".to_string(),
        ));
    }
    let mut clean = Vec::new();
    for scope in scopes {
        let scope = scope.trim();
        if !API_KEY_SCOPES.contains(&scope) {
            return Err(AppError::BadRequest(format!("unsupported scope: {scope}")));
        }
        if !clean.iter().any(|existing| existing == scope) {
            clean.push(scope.to_string());
        }
    }
    Ok(clean)
}

async fn fetch_creator_profile_for_api(
    db: &crate::db::Database,
    creator_id: &str,
) -> AppResult<CreatorProfile> {
    db.get_creator_profile_for_api(creator_id).await
}

async fn update_creator_profile_for_api(
    db: &crate::db::Database,
    creator_id: &str,
    input: UpdateCreatorApiProfileRequest,
) -> AppResult<()> {
    let current = fetch_creator_profile_for_api(db, creator_id).await?;
    db.update_creator_profile_for_api(
        creator_id,
        crate::db::CreatorApiProfileUpdate {
            display_name: input
                .display_name
                .unwrap_or(current.display_name)
                .trim()
                .to_string(),
            avatar: input.avatar.unwrap_or(current.avatar).trim().to_string(),
            banner: input.banner.unwrap_or(current.banner).trim().to_string(),
            tagline: input.tagline.unwrap_or(current.tagline).trim().to_string(),
            bio: input.bio.unwrap_or(current.bio).trim().to_string(),
        },
    )
    .await
}

async fn fetch_live_state_for_api(db: &crate::db::Database, creator_id: &str) -> AppResult<Value> {
    let profile = fetch_creator_profile_for_api(db, creator_id).await?;
    let current_broadcast = match profile.current_broadcast_id.as_deref() {
        Some(id) => Some(fetch_broadcast_for_api(db, creator_id, id).await?),
        None => None,
    };
    Ok(json!({ "profile": profile, "currentBroadcast": current_broadcast }))
}

async fn fetch_live_health_for_api(db: &crate::db::Database, creator_id: &str) -> AppResult<Value> {
    let active = if let Ok(pool) = db.try_postgres_adapter() {
        sqlx::query(
            "SELECT COUNT(*)::BIGINT AS count FROM live_ingest_sessions WHERE creator_id = $1 AND status IN ('connected', 'stale')",
        )
        .bind(creator_id)
        .fetch_one(pool)
        .await?
        .get::<i64, _>("count")
    } else {
        0
    };
    Ok(json!({ "activeIngestSessions": active }))
}

async fn fetch_live_runtime_for_api(
    db: &crate::db::Database,
    creator_id: &str,
) -> AppResult<Value> {
    if let Ok(pool) = db.try_postgres_adapter() {
        let rows = sqlx::query(
            r#"
            SELECT id, broadcast_id, status, contribution_state, connected_at,
                   disconnected_at, last_heartbeat_at, viewers, bitrate_kbps, dropped_frames
            FROM live_ingest_sessions
            WHERE creator_id = $1
            ORDER BY connected_at DESC
            LIMIT 20
            "#,
        )
        .bind(creator_id)
        .fetch_all(pool)
        .await?;
        let sessions = rows
            .into_iter()
            .map(|row| {
                json!({
                    "id": row.get::<String, _>("id"),
                    "broadcastId": row.get::<String, _>("broadcast_id"),
                    "status": row.get::<String, _>("status"),
                    "contributionState": row.get::<String, _>("contribution_state"),
                    "connectedAt": row.get::<String, _>("connected_at"),
                    "disconnectedAt": row.get::<Option<String>, _>("disconnected_at"),
                    "lastHeartbeatAt": row.get::<String, _>("last_heartbeat_at"),
                    "viewers": row.get::<i64, _>("viewers"),
                    "bitrateKbps": row.get::<i64, _>("bitrate_kbps"),
                    "droppedFrames": row.get::<i64, _>("dropped_frames"),
                })
            })
            .collect::<Vec<_>>();
        return Ok(json!({ "recentSessions": sessions }));
    }
    Ok(json!({ "recentSessions": [] }))
}

async fn update_live_for_api(
    db: &crate::db::Database,
    creator_id: &str,
    input: UpdateLiveRequest,
) -> AppResult<()> {
    let profile = fetch_creator_profile_for_api(db, creator_id).await?;
    let category = input.category.unwrap_or(profile.default_category);
    let tags = input.tags.unwrap_or(profile.default_tags);
    let tags_json = to_json(&tags)?;
    db.update_creator_live_defaults_for_api(
        creator_id,
        &category,
        &tags_json,
        input.title,
        input.is_mature,
        profile.current_broadcast_id.as_deref(),
    )
    .await
}

async fn start_broadcast_for_api(
    db: &crate::db::Database,
    creator_id: &str,
    input: StartBroadcastRequest,
) -> AppResult<Broadcast> {
    let profile = fetch_creator_profile_for_api(db, creator_id).await?;
    if let Some(id) = profile.current_broadcast_id.as_deref() {
        return fetch_broadcast_for_api(db, creator_id, id).await;
    }
    if input.title.trim().is_empty() {
        return Err(AppError::BadRequest("title is required".to_string()));
    }
    let id = format!("brd-{}", Uuid::new_v4().simple());
    let now = Utc::now().to_rfc3339();
    let tags_json = to_json(&input.tags)?;
    if let Ok(pool) = db.try_postgres_adapter() {
        sqlx::query(
            r#"
            INSERT INTO broadcasts (
                id, creator_id, title, category, tags_json, status, started_at, ended_at,
                duration_sec, peak_viewers, average_viewers, chat_messages, new_followers,
                new_subscribers, revenue, thumbnail, is_mature
            ) VALUES ($1, $2, $3, $4, $5, 'ready', $6, NULL, NULL, 0, 0, 0, $7, 0, 0, $8, $9)
            "#,
        )
        .bind(&id)
        .bind(creator_id)
        .bind(input.title.trim())
        .bind(&input.category)
        .bind(&tags_json)
        .bind(&now)
        .bind(if input.notify_followers { 3_i64 } else { 0_i64 })
        .bind(input.thumbnail.unwrap_or_default())
        .bind(input.is_mature)
        .execute(pool)
        .await?;
        sqlx::query(
            "UPDATE creator_profiles SET live_status = 'ready', current_broadcast_id = $1, default_category = $2, default_tags_json = $3 WHERE id = $4",
        )
        .bind(&id)
        .bind(&input.category)
        .bind(&tags_json)
        .bind(creator_id)
        .execute(pool)
        .await?;
        return fetch_broadcast_for_api(db, creator_id, &id).await;
    }
    Err(AppError::BadRequest(
        "creator api live control requires postgres".to_string(),
    ))
}

async fn end_broadcast_for_api(
    db: &crate::db::Database,
    creator_id: &str,
    id: &str,
) -> AppResult<Broadcast> {
    let broadcast = fetch_broadcast_for_api(db, creator_id, id).await?;
    let started_at = chrono::DateTime::parse_from_rfc3339(&broadcast.started_at)
        .map_err(|_| AppError::BadRequest("invalid broadcast timestamp".to_string()))?
        .with_timezone(&Utc);
    let ended_at = Utc::now();
    let duration_sec = (ended_at - started_at).num_seconds().max(0);
    if let Ok(pool) = db.try_postgres_adapter() {
        sqlx::query(
            "UPDATE broadcasts SET status = 'ended', ended_at = $1, duration_sec = $2 WHERE id = $3 AND creator_id = $4",
        )
        .bind(ended_at.to_rfc3339())
        .bind(duration_sec)
        .bind(id)
        .bind(creator_id)
        .execute(pool)
        .await?;
        sqlx::query(
            "UPDATE creator_profiles SET live_status = 'offline', current_broadcast_id = NULL WHERE id = $1 AND current_broadcast_id = $2",
        )
        .bind(creator_id)
        .bind(id)
        .execute(pool)
        .await?;
        return fetch_broadcast_for_api(db, creator_id, id).await;
    }
    Err(AppError::BadRequest(
        "creator api live control requires postgres".to_string(),
    ))
}

async fn fetch_broadcast_for_api(
    db: &crate::db::Database,
    creator_id: &str,
    id: &str,
) -> AppResult<Broadcast> {
    if let Ok(pool) = db.try_postgres_adapter() {
        let row = sqlx::query(
            r#"
            SELECT id, title, category, tags_json, status, started_at, ended_at,
                   duration_sec, peak_viewers, average_viewers, chat_messages,
                   new_followers, new_subscribers, revenue, thumbnail, is_mature
            FROM broadcasts
            WHERE creator_id = $1 AND id = $2
            "#,
        )
        .bind(creator_id)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
        return Ok(Broadcast {
            id: row.get("id"),
            title: row.get("title"),
            category: row.get("category"),
            tags: from_json(row.get::<String, _>("tags_json"))?,
            status: row.get("status"),
            started_at: row.get("started_at"),
            ended_at: row.get("ended_at"),
            duration_sec: row.get("duration_sec"),
            peak_viewers: row.get("peak_viewers"),
            average_viewers: row.get("average_viewers"),
            chat_messages: row.get("chat_messages"),
            new_followers: row.get("new_followers"),
            new_subscribers: row.get("new_subscribers"),
            revenue: row.get("revenue"),
            thumbnail: row.get("thumbnail"),
            is_mature: row.get("is_mature"),
        });
    }
    Err(AppError::BadRequest(
        "creator api live control requires postgres".to_string(),
    ))
}
