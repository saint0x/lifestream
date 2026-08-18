use super::*;
use crate::models::{PlaybackAudioTrack, PlaybackCaptionTrack, PlaybackPreviewTrack};

pub(crate) async fn build_playback_grant_from_session_record(
    state: &SharedState,
    session_record: &PlaybackSessionRecord,
    playback_token: &str,
) -> AppResult<PlaybackGrant> {
    let session = playback_session_from_record(session_record);
    let maybe_identity = playback_identity_from_session_record(session_record);

    if session.content_kind == "live" {
        let target = fetch_live_stream_playback_target(&state.pool, &session.content_id).await?;
        return build_live_playback_grant(
            &state.pool,
            &target,
            maybe_identity.as_ref(),
            session,
            playback_token,
        )
        .await;
    }

    let target = fetch_upload_playback_target(&state.pool, &session.content_id).await?;
    build_upload_playback_grant(
        &state.pool,
        &target,
        maybe_identity.as_ref(),
        session,
        playback_token,
    )
    .await
}

pub(crate) async fn build_live_playback_grant(
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
    let audio_tracks = build_playback_audio_tracks(
        pool,
        maybe_identity,
        &target.asset.status,
        &target.asset.id,
        &target.asset.variants,
        target.asset.audio_codec.as_deref(),
        playback_token,
    )
    .await?;
    let caption_tracks = build_playback_caption_tracks(
        pool,
        maybe_identity,
        &target.asset.status,
        &target.asset.variants,
        playback_token,
    )
    .await?;
    let preview_tracks =
        build_playback_preview_tracks(pool, &target.asset.status, &target.asset.id, playback_token)
            .await?;

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

pub(crate) async fn build_upload_playback_grant(
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
    let audio_tracks = build_playback_audio_tracks(
        pool,
        maybe_identity,
        &target.asset.status,
        &target.asset.id,
        &target.asset.variants,
        target.asset.audio_codec.as_deref(),
        playback_token,
    )
    .await?;
    let caption_tracks = build_playback_caption_tracks(
        pool,
        maybe_identity,
        &target.asset.status,
        &target.asset.variants,
        playback_token,
    )
    .await?;
    let preview_tracks =
        build_playback_preview_tracks(pool, &target.asset.status, &target.asset.id, playback_token)
            .await?;

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

async fn build_playback_audio_tracks(
    pool: &SqlitePool,
    maybe_identity: Option<&RequestIdentity>,
    status: &str,
    asset_id: &str,
    variants: &[MediaAssetVariant],
    audio_codec: Option<&str>,
    playback_token: &str,
) -> AppResult<Vec<PlaybackAudioTrack>> {
    let (preferred_audio_language, prefer_dubbed) = fetch_user_audio_preferences(
        pool,
        maybe_identity.map(|identity| identity.user_id.as_str()),
    )
    .await?;
    Ok(build_media_audio_tracks(
        status,
        asset_id,
        variants,
        audio_codec,
        Some(playback_token),
        preferred_audio_language.as_deref(),
        prefer_dubbed,
    ))
}

async fn build_playback_caption_tracks(
    pool: &SqlitePool,
    maybe_identity: Option<&RequestIdentity>,
    status: &str,
    variants: &[MediaAssetVariant],
    playback_token: &str,
) -> AppResult<Vec<PlaybackCaptionTrack>> {
    let preferred_subtitle_language = fetch_user_subtitle_preference(
        pool,
        maybe_identity.map(|identity| identity.user_id.as_str()),
    )
    .await?;
    Ok(build_media_caption_tracks(
        status,
        variants,
        Some(playback_token),
        preferred_subtitle_language.as_deref(),
    ))
}

async fn build_playback_preview_tracks(
    pool: &SqlitePool,
    status: &str,
    asset_id: &str,
    playback_token: &str,
) -> AppResult<Vec<PlaybackPreviewTrack>> {
    let preview_track_rows = fetch_media_preview_track_rows(pool, asset_id).await?;
    Ok(build_media_preview_tracks(
        status,
        &preview_track_rows,
        Some(playback_token),
    ))
}

fn playback_identity_from_session_record(
    session_record: &PlaybackSessionRecord,
) -> Option<RequestIdentity> {
    session_record
        .user_id
        .as_deref()
        .map(|user_id| RequestIdentity {
            user_id: user_id.to_string(),
            session_id: session_record.auth_session_id.clone().unwrap_or_default(),
            creator_id: None,
            scopes: Vec::new(),
        })
}
