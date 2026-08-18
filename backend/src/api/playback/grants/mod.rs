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
        validate_playback_session_record(&state.pool, &session_id, &playback_token).await?;
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
        validate_playback_session_record(&state.pool, &session_id, &playback_token).await?;
    let (refreshed_record, refreshed_token) =
        rotate_playback_session_token_for_refresh(&state.pool, session_record, &playback_token)
            .await?;

    Ok(Json(
        build_playback_grant_from_session_record(&state, &refreshed_record, &refreshed_token)
            .await?,
    ))
}

pub(crate) async fn rotate_playback_session_token_for_refresh(
    pool: &SqlitePool,
    session_record: PlaybackSessionRecord,
    playback_token: &str,
) -> AppResult<(PlaybackSessionRecord, String)> {
    let refreshed_token = format!("pbt_{}", Uuid::new_v4().simple());
    let refreshed_at = Utc::now().to_rfc3339();
    let refreshed_expires_at = (Utc::now() + chrono::Duration::hours(6)).to_rfc3339();

    let update = sqlx::query(
        r#"
        UPDATE playback_sessions
        SET token_hash = ?, expires_at = ?, last_used_at = ?
        WHERE id = ? AND token_hash = ? AND expires_at > ?
        "#,
    )
    .bind(hash_token(&refreshed_token))
    .bind(&refreshed_expires_at)
    .bind(&refreshed_at)
    .bind(&session_record.id)
    .bind(hash_token(playback_token))
    .bind(&refreshed_at)
    .execute(pool)
    .await?;

    if update.rows_affected() != 1 {
        return Err(AppError::Unauthorized);
    }

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
