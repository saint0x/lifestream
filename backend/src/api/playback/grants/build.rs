use super::*;
use crate::{
    config::StorageKind,
    models::{
        PlaybackAudioTrack, PlaybackCaptionTrack, PlaybackMediaAuthorization, PlaybackPreviewTrack,
    },
};

struct PlaybackGrantTracks {
    audio_tracks: Vec<PlaybackAudioTrack>,
    caption_tracks: Vec<PlaybackCaptionTrack>,
    preview_tracks: Vec<PlaybackPreviewTrack>,
}

pub(crate) async fn build_playback_grant_from_session_record(
    state: &SharedState,
    session_record: &PlaybackSessionRecord,
    playback_token: &str,
) -> AppResult<PlaybackGrant> {
    let session = playback_session_from_record(session_record);
    let maybe_identity = playback_identity_from_session_record(session_record);

    if session.content_kind == "live" {
        let target =
            fetch_live_stream_playback_target(state.db.try_sqlite_adapter()?, &session.content_id)
                .await?;
        return build_live_playback_grant(
            state,
            &target,
            maybe_identity.as_ref(),
            session,
            playback_token,
        )
        .await;
    }

    let target = fetch_upload_playback_target_for_database(&state.db, &session.content_id).await?;
    build_upload_playback_grant(
        state,
        &target,
        maybe_identity.as_ref(),
        session,
        playback_token,
    )
    .await
}

pub(crate) async fn build_live_playback_grant(
    state: &SharedState,
    target: &LivePlaybackTarget,
    maybe_identity: Option<&RequestIdentity>,
    session: PlaybackSession,
    playback_token: &str,
) -> AppResult<PlaybackGrant> {
    let manifest_url = state.storage.playback_manifest_url(
        &session.id,
        &target.playback_relative_path,
        playback_token,
    );
    let poster_url = target
        .poster_relative_path
        .as_ref()
        .map(|path| state.storage.playback_media_url(path, playback_token));
    let PlaybackGrantTracks {
        audio_tracks,
        caption_tracks,
        preview_tracks,
    } = build_playback_grant_tracks(
        state,
        maybe_identity,
        &target.asset.status,
        &target.asset.id,
        &target.asset.variants,
        target.asset.audio_codec.as_deref(),
        playback_token,
    )
    .await?;

    Ok(PlaybackGrant {
        media_authorization: playback_media_authorization(state, &session, playback_token),
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
    state: &SharedState,
    target: &UploadPlaybackTarget,
    maybe_identity: Option<&RequestIdentity>,
    session: PlaybackSession,
    playback_token: &str,
) -> AppResult<PlaybackGrant> {
    let manifest_relative_path = target.asset.playback_path.as_deref().unwrap_or_default();
    let manifest_url =
        state
            .storage
            .playback_manifest_url(&session.id, manifest_relative_path, playback_token);
    let poster_url = target
        .asset
        .poster_path
        .as_ref()
        .map(|path| state.storage.playback_media_url(path, playback_token));
    let PlaybackGrantTracks {
        audio_tracks,
        caption_tracks,
        preview_tracks,
    } = build_playback_grant_tracks(
        state,
        maybe_identity,
        &target.asset.status,
        &target.asset.id,
        &target.asset.variants,
        target.asset.audio_codec.as_deref(),
        playback_token,
    )
    .await?;

    Ok(PlaybackGrant {
        media_authorization: playback_media_authorization(state, &session, playback_token),
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

fn playback_media_authorization(
    state: &SharedState,
    session: &PlaybackSession,
    playback_token: &str,
) -> PlaybackMediaAuthorization {
    match state.storage.kind() {
        StorageKind::Local => PlaybackMediaAuthorization {
            strategy: "backendSessionToken".to_string(),
            manifest_authorization: "backend-manifest-token".to_string(),
            asset_authorization: "backend-media-token".to_string(),
            cache_strategy: "tokenized-backend-origin".to_string(),
            cdn_cookie_url: None,
            cdn_cookie_name: None,
            cdn_cookie_domain: None,
        },
        StorageKind::Object => PlaybackMediaAuthorization {
            strategy: "cdnSignedCookie".to_string(),
            manifest_authorization: "cdn-cookie".to_string(),
            asset_authorization: "cdn-cookie".to_string(),
            cache_strategy: "cacheable-cdn-origin".to_string(),
            cdn_cookie_url: Some(format!(
                "/api/v1/playback/sessions/{}/cdn-cookie?playbackToken={}",
                session.id, playback_token
            )),
            cdn_cookie_name: Some("VANTA_CDN_PLAYBACK".to_string()),
            cdn_cookie_domain: state
                .storage
                .cdn_cookie_domain()
                .map(std::borrow::ToOwned::to_owned),
        },
    }
}

async fn build_playback_grant_tracks(
    state: &SharedState,
    maybe_identity: Option<&RequestIdentity>,
    status: &str,
    asset_id: &str,
    variants: &[MediaAssetVariant],
    audio_codec: Option<&str>,
    playback_token: &str,
) -> AppResult<PlaybackGrantTracks> {
    let user_id = maybe_identity.map(|identity| identity.user_id.as_str());
    let (preferences, preview_track_rows) = tokio::try_join!(
        fetch_user_playback_preferences_for_database(&state.db, user_id),
        fetch_media_preview_track_rows_for_database(&state.db, asset_id),
    )?;
    let playback_media_url = |relative_path: &str| {
        state
            .storage
            .playback_media_url(relative_path, playback_token)
    };

    let audio_tracks = build_media_audio_tracks(
        status,
        asset_id,
        variants,
        audio_codec,
        Some(&playback_media_url),
        preferences.audio_language.as_deref(),
        preferences.prefer_dubbed,
    );
    let caption_tracks = build_media_caption_tracks(
        status,
        variants,
        Some(&playback_media_url),
        preferences.subtitle_language.as_deref(),
    );
    let preview_tracks =
        build_media_preview_tracks(status, &preview_track_rows, Some(&playback_media_url));

    Ok(PlaybackGrantTracks {
        audio_tracks,
        caption_tracks,
        preview_tracks,
    })
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
