use super::grants::{build_live_playback_grant, build_upload_playback_grant};
use super::*;
use crate::api::control::ensure_live_runtime_output_ready_for_playback;
use serde::Deserialize;

const LIVE_PLAYBACK_GRANT_CACHE_TTL: Duration = Duration::from_secs(5);
const PLAYBACK_SESSION_DEVICE_ID_MAX_LEN: usize = 128;
const PLAYBACK_SESSION_DEVICE_NAME_MAX_LEN: usize = 128;
const PLAYBACK_SESSION_PLAYER_VERSION_MAX_LEN: usize = 64;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaybackSessionCreateRequest {
    device_id: Option<String>,
    device_name: Option<String>,
    player_version: Option<String>,
    capabilities: Option<Value>,
}

fn inserted_playback_session(
    session_id: String,
    content_id: String,
    content_kind: String,
    access_scope: String,
    created_at: String,
    expires_at: String,
) -> PlaybackSession {
    PlaybackSession {
        id: session_id,
        content_id,
        content_kind,
        access_scope,
        created_at: created_at.clone(),
        expires_at,
        last_used_at: created_at,
    }
}

fn live_playback_grant_cache_key(
    stream_id: &str,
    maybe_identity: Option<&RequestIdentity>,
    device_id: Option<&str>,
) -> String {
    match (maybe_identity, device_id) {
        (Some(identity), Some(device_id)) => {
            format!(
                "live-playback:{stream_id}:auth:{}:device:{device_id}",
                identity.session_id
            )
        }
        (Some(identity), None) => format!("live-playback:{stream_id}:auth:{}", identity.session_id),
        (None, Some(device_id)) => format!("live-playback:{stream_id}:anon-device:{device_id}"),
        (None, None) => format!("live-playback:{stream_id}:anon"),
    }
}

fn playback_session_from_reusable_row(
    row: sqlx::sqlite::SqliteRow,
) -> PlaybackSession {
    PlaybackSession {
        id: row.get("id"),
        content_id: row.get("content_id"),
        content_kind: row.get("content_kind"),
        access_scope: row.get("access_scope"),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        last_used_at: row.get("last_used_at"),
    }
}

fn normalize_optional_playback_metadata(
    value: Option<&str>,
    field_name: &str,
    max_len: usize,
) -> AppResult<Option<String>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.len() > max_len {
        return Err(AppError::BadRequest(format!(
            "{field_name} exceeds the maximum supported length"
        )));
    }
    Ok(Some(value.to_string()))
}

async fn fetch_reusable_live_playback_session(
    pool: &SqlitePool,
    stream_id: &str,
    maybe_identity: Option<&RequestIdentity>,
    device_id: Option<&str>,
) -> AppResult<Option<PlaybackSession>> {
    let Some(row) = (match (maybe_identity, device_id) {
        (Some(identity), Some(device_id)) => {
            sqlx::query(
                r#"
                SELECT id, content_id, content_kind, access_scope, created_at, expires_at, last_used_at
                FROM playback_sessions
                WHERE content_id = ?
                  AND content_kind = 'live'
                  AND access_scope = 'live'
                  AND auth_session_id = ?
                  AND device_id = ?
                  AND expires_at > ?
                ORDER BY last_used_at DESC
                LIMIT 1
                "#,
            )
            .bind(stream_id)
            .bind(&identity.session_id)
            .bind(device_id)
            .bind(Utc::now().to_rfc3339())
            .fetch_optional(pool)
            .await?
        }
        (Some(identity), None) => {
            sqlx::query(
                r#"
                SELECT id, content_id, content_kind, access_scope, created_at, expires_at, last_used_at
                FROM playback_sessions
                WHERE content_id = ?
                  AND content_kind = 'live'
                  AND access_scope = 'live'
                  AND auth_session_id = ?
                  AND expires_at > ?
                ORDER BY last_used_at DESC
                LIMIT 1
                "#,
            )
            .bind(stream_id)
            .bind(&identity.session_id)
            .bind(Utc::now().to_rfc3339())
            .fetch_optional(pool)
            .await?
        }
        (None, Some(device_id)) => {
            sqlx::query(
                r#"
                SELECT id, content_id, content_kind, access_scope, created_at, expires_at, last_used_at
                FROM playback_sessions
                WHERE content_id = ?
                  AND content_kind = 'live'
                  AND access_scope = 'live'
                  AND auth_session_id IS NULL
                  AND user_id IS NULL
                  AND device_id = ?
                  AND expires_at > ?
                ORDER BY last_used_at DESC
                LIMIT 1
                "#,
            )
            .bind(stream_id)
            .bind(device_id)
            .bind(Utc::now().to_rfc3339())
            .fetch_optional(pool)
            .await?
        }
        (None, None) => None,
    }) else {
        return Ok(None);
    };

    Ok(Some(playback_session_from_reusable_row(row)))
}

async fn rotate_reusable_live_playback_session(
    pool: &SqlitePool,
    session: PlaybackSession,
    device_name: Option<&str>,
    player_version: Option<&str>,
    capabilities_json: Option<&str>,
) -> AppResult<(PlaybackSession, String)> {
    let refreshed_token = format!("pbt_{}", Uuid::new_v4().simple());
    let refreshed_at = Utc::now().to_rfc3339();
    let refreshed_expires_at = (Utc::now() + chrono::Duration::hours(6)).to_rfc3339();

    let update = sqlx::query(
        r#"
        UPDATE playback_sessions
        SET token_hash = ?, expires_at = ?, last_used_at = ?,
            device_name = COALESCE(?, device_name),
            player_version = COALESCE(?, player_version),
            capabilities_json = COALESCE(?, capabilities_json)
        WHERE id = ? AND expires_at > ?
        "#,
    )
    .bind(hash_token(&refreshed_token))
    .bind(&refreshed_expires_at)
    .bind(&refreshed_at)
    .bind(device_name)
    .bind(player_version)
    .bind(capabilities_json)
    .bind(&session.id)
    .bind(&refreshed_at)
    .execute(pool)
    .await?;

    if update.rows_affected() != 1 {
        return Err(AppError::Unauthorized);
    }

    Ok((
        PlaybackSession {
            expires_at: refreshed_expires_at,
            last_used_at: refreshed_at,
            ..session
        },
        refreshed_token,
    ))
}

pub(crate) async fn create_upload_playback_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(upload_id): Path<String>,
) -> AppResult<Json<PlaybackGrant>> {
    create_playback_session_for_content_id(state, headers, upload_id).await
}

pub(crate) async fn create_content_playback_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(content_id): Path<String>,
) -> AppResult<Json<PlaybackGrant>> {
    create_playback_session_for_content_id(state, headers, content_id).await
}

pub(crate) async fn create_live_playback_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
    payload: Option<Json<PlaybackSessionCreateRequest>>,
) -> AppResult<Json<PlaybackGrant>> {
    let maybe_identity = optional_identity(&state.pool, &headers).await?;
    let payload = payload.map(|Json(payload)| payload).unwrap_or_default();
    let device_id = normalize_optional_playback_metadata(
        payload.device_id.as_deref(),
        "deviceId",
        PLAYBACK_SESSION_DEVICE_ID_MAX_LEN,
    )?;
    let device_name = normalize_optional_playback_metadata(
        payload.device_name.as_deref(),
        "deviceName",
        PLAYBACK_SESSION_DEVICE_NAME_MAX_LEN,
    )?;
    let player_version = normalize_optional_playback_metadata(
        payload.player_version.as_deref(),
        "playerVersion",
        PLAYBACK_SESSION_PLAYER_VERSION_MAX_LEN,
    )?;
    let capabilities_json = payload
        .capabilities
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    let grant_cache_key =
        live_playback_grant_cache_key(&stream_id, maybe_identity.as_ref(), device_id.as_deref());
    if let Some(cached) = state
        .live_response_cache
        .get_live_playback_grant(&grant_cache_key, LIVE_PLAYBACK_GRANT_CACHE_TTL)
        .await
    {
        return Ok(Json(cached));
    }
    let _coalesced = state
        .request_coalescer
        .acquire(&format!("live-playback-grant:{grant_cache_key}"))
        .await;
    if let Some(cached) = state
        .live_response_cache
        .get_live_playback_grant(&grant_cache_key, LIVE_PLAYBACK_GRANT_CACHE_TTL)
        .await
    {
        return Ok(Json(cached));
    }
    let target = fetch_live_stream_playback_target(&state.pool, &stream_id).await?;
    ensure_live_runtime_output_ready_for_playback(
        &state,
        &target.runtime_output,
        &target.playback_relative_path,
    )
    .await?;
    if let Some(existing_session) = fetch_reusable_live_playback_session(
        &state.pool,
        &stream_id,
        maybe_identity.as_ref(),
        device_id.as_deref(),
    )
    .await?
    {
        let (session, playback_token) = rotate_reusable_live_playback_session(
            &state.pool,
            existing_session,
            device_name.as_deref(),
            player_version.as_deref(),
            capabilities_json.as_deref(),
        )
        .await?;
        let grant = build_live_playback_grant(
            &state.pool,
            &target,
            maybe_identity.as_ref(),
            session,
            &playback_token,
        )
        .await?;
        state
            .live_response_cache
            .put_live_playback_grant(&grant_cache_key, grant.clone())
            .await;
        return Ok(Json(grant));
    }
    let now = Utc::now();
    let session_id = format!("pbs-{}", Uuid::new_v4().simple());
    let playback_token = format!("pbt_{}", Uuid::new_v4().simple());
    let expires_at = (now + chrono::Duration::hours(6)).to_rfc3339();
    let now_rfc3339 = now.to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO playback_sessions (
            id, auth_session_id, user_id, creator_id, asset_id, content_id, content_kind, token_hash,
            access_scope, created_at, expires_at, last_used_at,
            device_id, device_name, player_version, capabilities_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&session_id)
    .bind(
        maybe_identity
            .as_ref()
            .map(|identity| identity.session_id.clone()),
    )
    .bind(
        maybe_identity
            .as_ref()
            .map(|identity| identity.user_id.clone()),
    )
    .bind(Some(target.creator_id.clone()))
    .bind(&target.asset_id)
    .bind(&stream_id)
    .bind("live")
    .bind(hash_token(&playback_token))
    .bind("live")
    .bind(&now_rfc3339)
    .bind(&expires_at)
    .bind(&now_rfc3339)
    .bind(device_id.as_deref())
    .bind(device_name.as_deref())
    .bind(player_version.as_deref())
    .bind(capabilities_json.as_deref())
    .execute(&state.pool)
    .await?;

    let session = inserted_playback_session(
        session_id,
        stream_id,
        "live".to_string(),
        "live".to_string(),
        now_rfc3339,
        expires_at,
    );
    let grant = build_live_playback_grant(
        &state.pool,
        &target,
        maybe_identity.as_ref(),
        session,
        &playback_token,
    )
    .await?;
    state
        .live_response_cache
        .put_live_playback_grant(&grant_cache_key, grant.clone())
        .await;
    Ok(Json(grant))
}

async fn create_playback_session_for_content_id(
    state: SharedState,
    headers: HeaderMap,
    content_id: String,
) -> AppResult<Json<PlaybackGrant>> {
    let maybe_identity = optional_identity(&state.pool, &headers).await?;
    let target = fetch_upload_playback_target(&state.pool, &content_id).await?;
    let access =
        resolve_upload_playback_access(&state.pool, maybe_identity.as_ref(), &target).await?;
    let access_scope = access.access_scope.clone();
    let now = Utc::now();
    let session_id = format!("pbs-{}", Uuid::new_v4().simple());
    let playback_token = format!("pbt_{}", Uuid::new_v4().simple());
    let expires_at = (now + chrono::Duration::hours(6)).to_rfc3339();
    let now_rfc3339 = now.to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO playback_sessions (
            id, auth_session_id, user_id, creator_id, asset_id, content_id, content_kind, token_hash,
            access_scope, created_at, expires_at, last_used_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&session_id)
    .bind(
        maybe_identity
            .as_ref()
            .map(|identity| identity.session_id.clone()),
    )
    .bind(
        maybe_identity
            .as_ref()
            .map(|identity| identity.user_id.clone()),
    )
    .bind(Some(target.creator_id.clone()))
    .bind(&target.asset.id)
    .bind(&content_id)
    .bind(&target.asset.kind)
    .bind(hash_token(&playback_token))
    .bind(&access_scope)
    .bind(&now_rfc3339)
    .bind(&expires_at)
    .bind(&now_rfc3339)
    .execute(&state.pool)
    .await?;

    let session = inserted_playback_session(
        session_id,
        content_id,
        target.asset.kind.clone(),
        access_scope,
        now_rfc3339,
        expires_at,
    );
    Ok(Json(
        build_upload_playback_grant(
            &state.pool,
            &target,
            maybe_identity.as_ref(),
            session,
            &playback_token,
        )
        .await?,
    ))
}
