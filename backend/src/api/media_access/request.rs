use super::*;

pub(crate) fn rewrite_hls_manifest_reference(
    relative_reference: &str,
    manifest_dir: &FsPath,
    playback_token: &str,
) -> String {
    let resolved = normalize_relative_storage_path(&manifest_dir.join(relative_reference));
    format!(
        "/api/v1/media/{}?playbackToken={}",
        resolved.to_string_lossy(),
        playback_token
    )
}

pub(crate) fn normalize_relative_storage_path(path: &FsPath) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(segment) => normalized.push(segment),
            Component::RootDir | Component::Prefix(_) => {}
        }
    }
    normalized
}

pub(crate) fn rewrite_hls_manifest_media_uri_line(
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
    let rewritten_uri =
        rewrite_hls_manifest_reference(&line[value_start..value_end], manifest_dir, playback_token);
    format!(
        "{}URI=\"{}\"{}",
        &line[..uri_start],
        rewritten_uri,
        &line[value_end + 1..]
    )
}

pub(crate) fn rewrite_preview_vtt_body(
    body: &str,
    relative_path: &str,
    playback_token: &str,
) -> AppResult<String> {
    let manifest_dir = PathBuf::from(relative_path)
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| AppError::BadRequest("invalid preview vtt path".to_string()))?;
    Ok(body
        .lines()
        .map(|line| {
            if let Some((reference, suffix)) = line.split_once("#xywh=") {
                format!(
                    "{}#xywh={}",
                    rewrite_hls_manifest_reference(reference, &manifest_dir, playback_token),
                    suffix
                )
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

pub(crate) async fn serve_media_file(
    State(state): State<SharedState>,
    Path(path): Path<String>,
    headers: HeaderMap,
    Query(query): Query<PlaybackAccessQuery>,
) -> AppResult<Response> {
    let relative_path = sanitize_storage_key(&path)?;
    authorize_media_request(&state, &headers, &query, &relative_path).await?;
    let full_path = media_path_for_relative(&state, &relative_path);
    let file_exists = tokio::fs::try_exists(&full_path).await?;
    let bytes = tokio::fs::read(&full_path).await.map_err(|error| {
        warn!(
            relative_path = %relative_path,
            full_path = %full_path.display(),
            file_exists,
            error = %error,
            "media file read failed"
        );
        if error.kind() == std::io::ErrorKind::NotFound {
            AppError::NotFound
        } else {
            AppError::Io(error)
        }
    })?;

    let content_type = media_content_type(&relative_path);
    if relative_path.ends_with(".vtt") {
        if let Some(playback_token) = query.playback_token.as_deref() {
            let text =
                String::from_utf8(bytes).map_err(|error| AppError::Internal(error.to_string()))?;
            let rewritten = rewrite_preview_vtt_body(&text, &relative_path, playback_token)?;
            return Ok((
                [(header::CONTENT_TYPE, content_type)],
                Body::from(rewritten),
            )
                .into_response());
        }
    }
    Ok(([(header::CONTENT_TYPE, content_type)], Body::from(bytes)).into_response())
}

pub(crate) async fn authorize_media_request(
    state: &SharedState,
    headers: &HeaderMap,
    query: &PlaybackAccessQuery,
    relative_path: &str,
) -> AppResult<()> {
    if let Some(identity) = optional_identity(&state.pool, headers).await? {
        if let Some(creator_id) = identity.creator_id.as_deref() {
            if creator_can_access_media_path(&state.pool, creator_id, relative_path).await? {
                return Ok(());
            }
        }
    }

    let playback_token = query
        .playback_token
        .as_deref()
        .ok_or(AppError::Unauthorized)?;
    let session =
        validate_playback_session_token_for_path(&state.pool, playback_token, relative_path)
            .await?;
    if session.content_kind == "live" {
        return Ok(());
    }

    let target = fetch_upload_playback_target(&state.pool, &session.content_id).await?;
    if playback_path_allowed_for_asset(&target.asset, relative_path) {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}
