use super::*;
use crate::api::playback_authority::{LivePlaybackTarget, UploadPlaybackTarget};

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

pub(crate) async fn get_playback_manifest(
    State(state): State<SharedState>,
    Path(session_id): Path<String>,
    Query(query): Query<PlaybackAccessQuery>,
) -> AppResult<Response> {
    let playback_token = query.playback_token.ok_or(AppError::Unauthorized)?;
    let session = validate_playback_session(&state.pool, &session_id, &playback_token).await?;
    let manifest_relative_path = if session.content_kind == "live" {
        fetch_live_stream_playback_target(&state.pool, &session.content_id)
            .await?
            .playback_relative_path
    } else {
        fetch_upload_playback_target(&state.pool, &session.content_id)
            .await?
            .asset
            .playback_path
            .clone()
            .ok_or_else(|| AppError::BadRequest("playback manifest unavailable".to_string()))?
    };
    let manifest_path = media_path_for_relative(&state, &manifest_relative_path);
    let manifest_body = tokio::fs::read_to_string(&manifest_path).await?;
    let manifest_dir = PathBuf::from(&manifest_relative_path)
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| AppError::BadRequest("invalid playback manifest path".to_string()))?;

    let rewritten = manifest_body
        .lines()
        .map(|line| {
            if line.is_empty() {
                line.to_string()
            } else if line.starts_with("#EXT-X-MEDIA:") {
                rewrite_hls_manifest_media_uri_line(line, &manifest_dir, &playback_token)
            } else if line.starts_with('#') {
                line.to_string()
            } else {
                rewrite_hls_manifest_reference(line, &manifest_dir, &playback_token)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok((
        [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
        Body::from(format!("{rewritten}\n")),
    )
        .into_response())
}

pub(super) async fn build_live_playback_grant(
    pool: &SqlitePool,
    target: &LivePlaybackTarget,
    maybe_identity: Option<&RequestIdentity>,
    session: PlaybackSession,
    playback_token: &str,
) -> AppResult<PlaybackGrant> {
    let manifest_url = format!(
        "/api/v1/playback/sessions/{}/manifest?playbackToken={}",
        session.id, playback_token
    );
    let poster_url = target
        .poster_relative_path
        .as_ref()
        .map(|path| format!("/api/v1/media/{path}?playbackToken={playback_token}"));
    let preferred_subtitle_language = fetch_user_subtitle_preference(
        pool,
        maybe_identity
            .as_ref()
            .map(|identity| identity.user_id.as_str()),
    )
    .await?;
    let (preferred_audio_language, prefer_dubbed) = fetch_user_audio_preferences(
        pool,
        maybe_identity
            .as_ref()
            .map(|identity| identity.user_id.as_str()),
    )
    .await?;
    let audio_tracks = build_media_audio_tracks(
        &target.asset.status,
        &target.asset.id,
        &target.asset.variants,
        target.asset.audio_codec.as_deref(),
        Some(playback_token),
        preferred_audio_language.as_deref(),
        prefer_dubbed,
    );
    let caption_tracks = build_media_caption_tracks(
        &target.asset.status,
        &target.asset.variants,
        Some(playback_token),
        preferred_subtitle_language.as_deref(),
    );
    let preview_track_rows = fetch_media_preview_track_rows(pool, &target.asset.id).await?;
    let preview_tracks = build_media_preview_tracks(
        &target.asset.status,
        &preview_track_rows,
        Some(playback_token),
    );

    Ok(PlaybackGrant {
        session,
        playback_token: playback_token.to_string(),
        manifest_url,
        poster_url,
        content_title: target.title.clone(),
        content_kind: "live".to_string(),
        visibility: "public".to_string(),
        access_policy: "free".to_string(),
        access_tier_id: None,
        price_cents: None,
        currency: None,
        rental_window_hours: None,
        default_audio_track_id: default_audio_track_id(&audio_tracks),
        default_caption_track_id: default_caption_track_id(&caption_tracks),
        default_preview_track_id: default_preview_track_id(&preview_tracks),
        audio_tracks,
        caption_tracks,
        preview_tracks,
    })
}

pub(super) async fn build_upload_playback_grant(
    pool: &SqlitePool,
    target: &UploadPlaybackTarget,
    maybe_identity: Option<&RequestIdentity>,
    session: PlaybackSession,
    playback_token: &str,
) -> AppResult<PlaybackGrant> {
    let manifest_url = format!(
        "/api/v1/playback/sessions/{}/manifest?playbackToken={}",
        session.id, playback_token
    );
    let poster_url = target
        .asset
        .poster_path
        .as_ref()
        .map(|path| format!("/api/v1/media/{path}?playbackToken={playback_token}"));
    let preferred_subtitle_language = fetch_user_subtitle_preference(
        pool,
        maybe_identity
            .as_ref()
            .map(|identity| identity.user_id.as_str()),
    )
    .await?;
    let (preferred_audio_language, prefer_dubbed) = fetch_user_audio_preferences(
        pool,
        maybe_identity
            .as_ref()
            .map(|identity| identity.user_id.as_str()),
    )
    .await?;
    let audio_tracks = build_media_audio_tracks(
        &target.asset.status,
        &target.asset.id,
        &target.asset.variants,
        target.asset.audio_codec.as_deref(),
        Some(playback_token),
        preferred_audio_language.as_deref(),
        prefer_dubbed,
    );
    let caption_tracks = build_media_caption_tracks(
        &target.asset.status,
        &target.asset.variants,
        Some(playback_token),
        preferred_subtitle_language.as_deref(),
    );
    let preview_track_rows = fetch_media_preview_track_rows(pool, &target.asset.id).await?;
    let preview_tracks = build_media_preview_tracks(
        &target.asset.status,
        &preview_track_rows,
        Some(playback_token),
    );

    Ok(PlaybackGrant {
        session,
        playback_token: playback_token.to_string(),
        manifest_url,
        poster_url,
        content_title: target.asset.title.clone(),
        content_kind: target.asset.kind.clone(),
        visibility: target.asset.visibility.clone(),
        access_policy: target.upload.access_policy.clone(),
        access_tier_id: target.upload.access_tier_id.clone(),
        price_cents: target.upload.price_cents,
        currency: target.upload.currency.clone(),
        rental_window_hours: target.upload.rental_window_hours,
        default_audio_track_id: default_audio_track_id(&audio_tracks),
        default_caption_track_id: default_caption_track_id(&caption_tracks),
        default_preview_track_id: default_preview_track_id(&preview_tracks),
        audio_tracks,
        caption_tracks,
        preview_tracks,
    })
}

async fn build_playback_grant_from_session_record(
    state: &SharedState,
    session_record: &PlaybackSessionRecord,
    playback_token: &str,
) -> AppResult<PlaybackGrant> {
    let session = playback_session_from_record(session_record);
    if session.content_kind == "live" {
        let target = fetch_live_stream_playback_target(&state.pool, &session.content_id).await?;
        return build_live_playback_grant(
            &state.pool,
            &target,
            session_record
                .user_id
                .as_deref()
                .map(|user_id| RequestIdentity {
                    user_id: user_id.to_string(),
                    session_id: session_record.auth_session_id.clone().unwrap_or_default(),
                    creator_id: None,
                    scopes: Vec::new(),
                })
                .as_ref(),
            session,
            playback_token,
        )
        .await;
    }

    let target = fetch_upload_playback_target(&state.pool, &session.content_id).await?;
    build_upload_playback_grant(
        &state.pool,
        &target,
        session_record
            .user_id
            .as_deref()
            .map(|user_id| RequestIdentity {
                user_id: user_id.to_string(),
                session_id: session_record.auth_session_id.clone().unwrap_or_default(),
                creator_id: None,
                scopes: Vec::new(),
            })
            .as_ref(),
        session,
        playback_token,
    )
    .await
}
