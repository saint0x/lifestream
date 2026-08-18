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
    let content_type = media_content_type(&relative_path);
    if relative_path.ends_with(".m3u8") {
        let body =
            load_playback_manifest_body(&state, &query, &relative_path, &full_path, file_exists)
                .await?;
        return Ok(([(header::CONTENT_TYPE, content_type)], Body::from(body)).into_response());
    }

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

async fn load_playback_manifest_body(
    state: &SharedState,
    query: &PlaybackAccessQuery,
    relative_path: &str,
    full_path: &FsPath,
    file_exists: bool,
) -> AppResult<String> {
    let raw = if let Some(playback_token) = query.playback_token.as_deref() {
        let session =
            validate_playback_session_token_for_path(&state.pool, playback_token, relative_path)
                .await?;
        load_hls_manifest_with_optional_blocking_reload(
            state,
            query,
            &session,
            relative_path,
            full_path,
        )
        .await?
    } else {
        tokio::fs::read_to_string(full_path)
            .await
            .map_err(|error| {
                warn!(
                    relative_path = %relative_path,
                    full_path = %full_path.display(),
                    file_exists,
                    error = %error,
                    "media manifest read failed"
                );
                if error.kind() == std::io::ErrorKind::NotFound {
                    AppError::NotFound
                } else {
                    AppError::Io(error)
                }
            })?
    };

    if let Some(playback_token) = query.playback_token.as_deref() {
        let manifest_dir = PathBuf::from(relative_path)
            .parent()
            .map(PathBuf::from)
            .ok_or_else(|| AppError::BadRequest("invalid playback manifest path".to_string()))?;
        Ok(raw
            .lines()
            .map(|line| {
                if line.is_empty() {
                    line.to_string()
                } else if line.starts_with("#EXT-X-MEDIA:") {
                    rewrite_hls_manifest_media_uri_line(line, &manifest_dir, playback_token)
                } else if line.starts_with("#EXT-X-MAP:") || line.starts_with("#EXT-X-PART:") {
                    rewrite_hls_manifest_media_uri_line(line, &manifest_dir, playback_token)
                } else if line.starts_with('#') {
                    line.to_string()
                } else {
                    rewrite_hls_manifest_reference(line, &manifest_dir, playback_token)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n")
    } else {
        Ok(raw)
    }
}

async fn load_hls_manifest_with_optional_blocking_reload(
    state: &SharedState,
    query: &PlaybackAccessQuery,
    session: &PlaybackSession,
    relative_path: &str,
    full_path: &FsPath,
) -> AppResult<String> {
    let mut body = tokio::fs::read_to_string(full_path)
        .await
        .map_err(AppError::Io)?;
    if session.content_kind != "live" {
        return Ok(body);
    }

    let target = fetch_live_stream_playback_target(&state.pool, &session.content_id).await?;
    if !target.runtime_output.blocking_reload_enabled {
        return Ok(body);
    }
    if relative_path == target.playback_relative_path {
        return Ok(body);
    }
    let playback_manifest_path = PathBuf::from(&target.playback_relative_path);
    let Some(parent) = playback_manifest_path.parent() else {
        return Ok(body);
    };
    if !relative_path.starts_with(parent.to_string_lossy().as_ref()) {
        return Ok(body);
    }

    let requested = requested_live_manifest_cursor(query);
    let Some(requested) = requested else {
        return Ok(body);
    };
    let current = parse_live_manifest_cursor(&body);
    if !requested_manifest_cursor_is_ahead(requested, current) {
        return Ok(body);
    }

    let deadline = std::time::Instant::now()
        + std::time::Duration::from_millis(
            (target.runtime_output.target_segment_duration_sec.max(1) as u64 * 1_000).min(3_000),
        );
    while std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(125)).await;
        let next_body = tokio::fs::read_to_string(full_path)
            .await
            .map_err(AppError::Io)?;
        let next_cursor = parse_live_manifest_cursor(&next_body);
        body = next_body;
        if !requested_manifest_cursor_is_ahead(requested, next_cursor) {
            break;
        }
    }

    Ok(body)
}

fn requested_live_manifest_cursor(query: &PlaybackAccessQuery) -> Option<(i64, Option<i64>)> {
    query.hls_msn.map(|msn| (msn, query.hls_part))
}

fn requested_manifest_cursor_is_ahead(
    requested: (i64, Option<i64>),
    current: (i64, Option<i64>),
) -> bool {
    if requested.0 > current.0 {
        return true;
    }
    requested.0 == current.0 && requested.1.unwrap_or(0) > current.1.unwrap_or(0)
}

fn parse_live_manifest_cursor(body: &str) -> (i64, Option<i64>) {
    let mut media_sequence = 0_i64;
    let mut part_count = 0_i64;
    let mut saw_segment = false;

    for line in body.lines() {
        if let Some(value) = line.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
            media_sequence = value.trim().parse::<i64>().unwrap_or(0);
        } else if line.starts_with("#EXT-X-PART:") {
            part_count += 1;
        } else if line.starts_with("#EXTINF:") {
            saw_segment = true;
        }
    }

    let part = if part_count > 0 {
        Some(part_count - 1)
    } else if saw_segment {
        Some(0)
    } else {
        None
    };
    (media_sequence, part)
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
