use super::*;

pub(crate) async fn validate_playback_session_token_for_path(
    database: &crate::db::Database,
    playback_token: &str,
    relative_path: &str,
) -> AppResult<PlaybackSession> {
    let session =
        validate_playback_session_record_for_path(database, playback_token, relative_path).await?;
    Ok(playback_session_from_record(&session))
}

pub(crate) async fn creator_can_access_media_path(
    pool: &SqlitePool,
    creator_id: &str,
    relative_path: &str,
) -> AppResult<bool> {
    let rows = sqlx::query(
        r#"
        SELECT id, source_relative_path, poster_relative_path, playback_relative_path
        FROM media_assets
        WHERE creator_id = ?
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let asset = (
            row.get::<String, _>("id"),
            row.get::<String, _>("source_relative_path"),
            row.get::<Option<String>, _>("poster_relative_path"),
            row.get::<Option<String>, _>("playback_relative_path"),
        );
        let extra_paths: Vec<String> = fetch_media_asset_variants(pool, &asset.0)
            .await?
            .into_iter()
            .map(|variant| variant.relative_path)
            .chain(
                fetch_media_preview_track_rows(pool, &asset.0)
                    .await?
                    .into_iter()
                    .flat_map(|track| [track.image_relative_path, track.vtt_relative_path]),
            )
            .collect::<Vec<_>>();
        if path_allowed_for_paths(
            relative_path,
            &asset.1,
            asset.2.as_deref(),
            asset.3.as_deref(),
            &extra_paths,
        ) {
            return Ok(true);
        }
    }

    Ok(false)
}

pub(crate) fn playback_path_allowed_for_asset(asset: &MediaAsset, relative_path: &str) -> bool {
    path_allowed_for_paths(
        relative_path,
        "",
        asset.poster_path.as_deref(),
        asset.playback_path.as_deref(),
        &asset
            .variants
            .iter()
            .map(|variant| variant.relative_path.clone())
            .chain(
                asset
                    .preview_tracks
                    .iter()
                    .flat_map(|track| [track.image_path.clone(), track.vtt_path.clone()]),
            )
            .collect::<Vec<_>>(),
    )
}

pub(crate) fn path_allowed_for_paths(
    relative_path: &str,
    source_path: &str,
    poster_path: Option<&str>,
    playback_path: Option<&str>,
    extra_paths: &[String],
) -> bool {
    if relative_path == source_path {
        return true;
    }
    if poster_path.is_some_and(|path| relative_path == path) {
        return true;
    }
    if extra_paths.iter().any(|path| relative_path == path) {
        return true;
    }
    if let Some(playback_path) = playback_path {
        if relative_path == playback_path {
            return true;
        }
        if let Some(parent) = PathBuf::from(playback_path).parent() {
            let prefix = parent.to_string_lossy();
            if relative_path.starts_with(prefix.as_ref()) {
                return true;
            }
        }
    }
    false
}

pub(crate) async fn validate_playback_session(
    database: &crate::db::Database,
    session_id: &str,
    playback_token: &str,
) -> AppResult<PlaybackSession> {
    let session = validate_playback_session_record(database, session_id, playback_token).await?;
    Ok(playback_session_from_record(&session))
}
