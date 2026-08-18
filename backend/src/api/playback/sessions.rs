use super::grants::{build_live_playback_grant, build_upload_playback_grant};
use super::*;
use crate::api::control::ensure_live_runtime_output_ready_for_playback;

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
) -> AppResult<Json<PlaybackGrant>> {
    let maybe_identity = optional_identity(&state.pool, &headers).await?;
    let target = fetch_live_stream_playback_target(&state.pool, &stream_id).await?;
    ensure_live_runtime_output_ready_for_playback(
        &state,
        &target.runtime_output,
        &target.playback_relative_path,
    )
    .await?;
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
    .bind(&target.asset_id)
    .bind(&stream_id)
    .bind("live")
    .bind(hash_token(&playback_token))
    .bind("live")
    .bind(&now_rfc3339)
    .bind(&expires_at)
    .bind(&now_rfc3339)
    .execute(&state.pool)
    .await?;

    let session = fetch_playback_session_by_id(&state.pool, &session_id).await?;
    Ok(Json(
        build_live_playback_grant(
            &state.pool,
            &target,
            maybe_identity.as_ref(),
            session,
            &playback_token,
        )
        .await?,
    ))
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
    .bind(access.access_scope)
    .bind(&now_rfc3339)
    .bind(&expires_at)
    .bind(&now_rfc3339)
    .execute(&state.pool)
    .await?;

    let session = fetch_playback_session_by_id(&state.pool, &session_id).await?;
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
