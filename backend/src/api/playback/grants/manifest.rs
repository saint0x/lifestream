use super::*;

pub(crate) async fn get_playback_manifest(
    State(state): State<SharedState>,
    Path(session_id): Path<String>,
    Query(query): Query<PlaybackAccessQuery>,
) -> AppResult<Response> {
    let playback_token = query.playback_token.ok_or(AppError::Unauthorized)?;
    let session = validate_playback_session(&state.db, &session_id, &playback_token).await?;
    let manifest_relative_path = if session.content_kind == "live" {
        fetch_live_stream_playback_target(state.db.try_sqlite_adapter()?, &session.content_id)
            .await?
            .playback_relative_path
    } else {
        fetch_upload_playback_target_for_database(&state.db, &session.content_id)
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
                rewrite_hls_manifest_media_uri_line_with_storage(
                    &state,
                    line,
                    &manifest_dir,
                    &playback_token,
                )
            } else if line.starts_with('#') {
                line.to_string()
            } else {
                rewrite_hls_manifest_reference_with_storage(
                    &state,
                    line,
                    &manifest_dir,
                    &playback_token,
                )
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

fn rewrite_hls_manifest_reference_with_storage(
    state: &SharedState,
    relative_reference: &str,
    manifest_dir: &FsPath,
    playback_token: &str,
) -> String {
    let resolved = normalize_relative_storage_path(&manifest_dir.join(relative_reference));
    state
        .storage
        .playback_media_url(&resolved.to_string_lossy(), playback_token)
}

fn rewrite_hls_manifest_media_uri_line_with_storage(
    state: &SharedState,
    line: &str,
    manifest_dir: &FsPath,
    playback_token: &str,
) -> String {
    let Some(uri_start) = line.find("URI=\"") else {
        return line.to_string();
    };
    let value_start = uri_start + 5;
    let Some(value_end_offset) = line[value_start..].find('"') else {
        return line.to_string();
    };
    let value_end = value_start + value_end_offset;
    let rewritten_uri = rewrite_hls_manifest_reference_with_storage(
        state,
        &line[value_start..value_end],
        manifest_dir,
        playback_token,
    );
    format!(
        "{}URI=\"{}\"{}",
        &line[..uri_start],
        rewritten_uri,
        &line[value_end + 1..]
    )
}
