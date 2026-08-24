use super::*;
use crate::api::playauth::{LivePlaybackTarget, UploadPlaybackTarget};

mod build;
mod manifest;

pub(super) use build::{
    build_live_playback_grant, build_playback_grant_from_session_record,
    build_upload_playback_grant,
};

pub(crate) async fn get_playback_session(
    State(state): State<SharedState>,
    Path(session_id): Path<String>,
    Query(query): Query<PlaybackAccessQuery>,
) -> AppResult<Json<PlaybackGrant>> {
    let playback_token = query.playback_token.ok_or(AppError::Unauthorized)?;
    let session_record =
        validate_playback_session_record(&state.db, &session_id, &playback_token).await?;
    Ok(Json(
        build_playback_grant_from_session_record(&state, &session_record, &playback_token).await?,
    ))
}

pub(crate) async fn refresh_playback_session(
    State(state): State<SharedState>,
    Path(session_id): Path<String>,
    Query(query): Query<PlaybackAccessQuery>,
) -> AppResult<Json<PlaybackGrant>> {
    let playback_token = query.playback_token.ok_or(AppError::Unauthorized)?;
    let session_record =
        validate_playback_session_record(&state.db, &session_id, &playback_token).await?;
    let (refreshed_record, refreshed_token) =
        rotate_playback_session_token_for_refresh(&state.db, session_record, &playback_token)
            .await?;

    Ok(Json(
        build_playback_grant_from_session_record(&state, &refreshed_record, &refreshed_token)
            .await?,
    ))
}

pub(crate) async fn issue_playback_cdn_cookie(
    State(state): State<SharedState>,
    Path(session_id): Path<String>,
    Query(query): Query<PlaybackAccessQuery>,
) -> AppResult<Response> {
    let playback_token = query.playback_token.ok_or(AppError::Unauthorized)?;
    if state.storage.cdn_cookie_domain().is_none() {
        return Err(AppError::NotFound);
    }
    let session = validate_playback_session(&state.db, &session_id, &playback_token).await?;
    let expires_at = chrono::DateTime::parse_from_rfc3339(&session.expires_at)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let expires_ts = expires_at.timestamp();
    let signed_payload = format!("{}:{expires_ts}", session.id);
    let signature = crate::auth::hash_token_with_secret(
        &signed_payload,
        state.config_token_hash_secret().as_deref(),
    );
    let cookie_value = format!("v1.{}.{}.{}", session.id, expires_ts, signature);
    let max_age = (expires_at.with_timezone(&Utc) - Utc::now())
        .num_seconds()
        .max(0);
    let cookie = format!(
        "VANTA_CDN_PLAYBACK={cookie_value}; Domain={}; Path=/; Max-Age={max_age}; Secure; HttpOnly; SameSite=None",
        state
            .storage
            .cdn_cookie_domain()
            .expect("checked cdn cookie domain")
    );
    let cookie_header =
        HeaderValue::from_str(&cookie).map_err(|error| AppError::Internal(error.to_string()))?;
    Ok((
        StatusCode::NO_CONTENT,
        [
            (header::SET_COOKIE, cookie_header),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, no-store"),
            ),
        ],
    )
        .into_response())
}

pub(crate) async fn rotate_playback_session_token_for_refresh(
    database: &crate::db::Database,
    session_record: PlaybackSessionRecord,
    playback_token: &str,
) -> AppResult<(PlaybackSessionRecord, String)> {
    let refreshed_token = format!("pbt_{}", Uuid::new_v4().simple());
    let refreshed_at = Utc::now().to_rfc3339();
    let refreshed_expires_at = (Utc::now() + chrono::Duration::hours(6)).to_rfc3339();

    database
        .rotate_playback_session_token(
            &session_record.id,
            playback_token,
            &refreshed_token,
            &refreshed_at,
            &refreshed_expires_at,
        )
        .await?;

    Ok((
        PlaybackSessionRecord {
            expires_at: refreshed_expires_at,
            last_used_at: refreshed_at,
            ..session_record
        },
        refreshed_token,
    ))
}

pub(crate) use manifest::get_playback_manifest;
