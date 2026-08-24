use super::grants::{build_live_playback_grant, build_upload_playback_grant};
use super::*;
use crate::api::control::ensure_live_runtime_output_ready_for_playback;
use crate::db::{
    NewPlaybackSession, PlaybackSessionMetadataUpdate, ReusableLivePlaybackSessionLookup,
};
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
    let maybe_identity = optional_identity(&state.db, &headers).await?;
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
    let target =
        fetch_live_stream_playback_target(state.db.try_sqlite_adapter()?, &stream_id).await?;
    ensure_live_runtime_output_ready_for_playback(
        &state,
        &target.runtime_output,
        &target.playback_relative_path,
    )
    .await?;
    let now = Utc::now();
    let now_rfc3339 = now.to_rfc3339();
    if let Some(existing_session) = state
        .db
        .find_reusable_live_playback_session(ReusableLivePlaybackSessionLookup {
            stream_id: &stream_id,
            auth_session_id: maybe_identity
                .as_ref()
                .map(|identity| identity.session_id.as_str()),
            device_id: device_id.as_deref(),
            now: &now_rfc3339,
        })
        .await?
    {
        let playback_token = format!("pbt_{}", Uuid::new_v4().simple());
        let refreshed_at = Utc::now().to_rfc3339();
        let refreshed_expires_at = (Utc::now() + chrono::Duration::hours(6)).to_rfc3339();
        let session = state
            .db
            .rotate_reusable_live_playback_session(
                existing_session,
                &playback_token,
                &refreshed_at,
                &refreshed_expires_at,
                PlaybackSessionMetadataUpdate {
                    device_name: device_name.as_deref(),
                    player_version: player_version.as_deref(),
                    capabilities_json: capabilities_json.as_deref(),
                },
            )
            .await?;
        let grant = build_live_playback_grant(
            &state,
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
        tracing::info!(
            stream_id,
            creator_id = %target.creator_id,
            asset_id = %target.asset_id,
            authenticated = maybe_identity.is_some(),
            "reused live playback session"
        );
        return Ok(Json(grant));
    }
    let session_id = format!("pbs-{}", Uuid::new_v4().simple());
    let playback_token = format!("pbt_{}", Uuid::new_v4().simple());
    let expires_at = (now + chrono::Duration::hours(6)).to_rfc3339();
    state
        .db
        .create_playback_session(NewPlaybackSession {
            id: &session_id,
            auth_session_id: maybe_identity
                .as_ref()
                .map(|identity| identity.session_id.as_str()),
            user_id: maybe_identity
                .as_ref()
                .map(|identity| identity.user_id.as_str()),
            creator_id: Some(&target.creator_id),
            asset_id: &target.asset_id,
            content_id: &stream_id,
            content_kind: "live",
            playback_token: &playback_token,
            access_scope: "live",
            created_at: &now_rfc3339,
            expires_at: &expires_at,
            device_id: device_id.as_deref(),
            device_name: device_name.as_deref(),
            player_version: player_version.as_deref(),
            capabilities_json: capabilities_json.as_deref(),
        })
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
        &state,
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
    tracing::info!(
        stream_id = %grant.session.content_id,
        creator_id = %target.creator_id,
        asset_id = %target.asset_id,
        authenticated = maybe_identity.is_some(),
        "created live playback session"
    );
    Ok(Json(grant))
}

async fn create_playback_session_for_content_id(
    state: SharedState,
    headers: HeaderMap,
    content_id: String,
) -> AppResult<Json<PlaybackGrant>> {
    let maybe_identity = optional_identity(&state.db, &headers).await?;
    let target = fetch_upload_playback_target(state.db.try_sqlite_adapter()?, &content_id).await?;
    let access = resolve_upload_playback_access(
        state.db.try_sqlite_adapter()?,
        maybe_identity.as_ref(),
        &target,
    )
    .await?;
    let access_scope = access.access_scope.clone();
    let now = Utc::now();
    let session_id = format!("pbs-{}", Uuid::new_v4().simple());
    let playback_token = format!("pbt_{}", Uuid::new_v4().simple());
    let expires_at = (now + chrono::Duration::hours(6)).to_rfc3339();
    let now_rfc3339 = now.to_rfc3339();

    state
        .db
        .create_playback_session(NewPlaybackSession {
            id: &session_id,
            auth_session_id: maybe_identity
                .as_ref()
                .map(|identity| identity.session_id.as_str()),
            user_id: maybe_identity
                .as_ref()
                .map(|identity| identity.user_id.as_str()),
            creator_id: Some(&target.creator_id),
            asset_id: &target.asset.id,
            content_id: &content_id,
            content_kind: &target.asset.kind,
            playback_token: &playback_token,
            access_scope: &access_scope,
            created_at: &now_rfc3339,
            expires_at: &expires_at,
            device_id: None,
            device_name: None,
            player_version: None,
            capabilities_json: None,
        })
        .await?;

    let session = inserted_playback_session(
        session_id,
        content_id,
        target.asset.kind.clone(),
        access_scope,
        now_rfc3339,
        expires_at,
    );
    let grant = build_upload_playback_grant(
        &state,
        &target,
        maybe_identity.as_ref(),
        session,
        &playback_token,
    )
    .await?;
    tracing::info!(
        content_id = %grant.session.content_id,
        creator_id = %target.creator_id,
        asset_id = %target.asset.id,
        access_scope = %grant.session.access_scope,
        authenticated = maybe_identity.is_some(),
        "created upload playback session"
    );
    Ok(Json(grant))
}
