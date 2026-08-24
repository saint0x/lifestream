use super::*;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt, SeekFrom},
};
use tokio_util::io::ReaderStream;

const MAX_REWRITTEN_TEXT_TRACK_BYTES: u64 = 2 * 1024 * 1024;

fn rewrite_hls_manifest_reference_with_media_url(
    relative_reference: &str,
    manifest_dir: &FsPath,
    media_url: &dyn Fn(&str) -> String,
) -> String {
    let resolved = normalize_relative_storage_path(&manifest_dir.join(relative_reference));
    media_url(&resolved.to_string_lossy())
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

fn rewrite_hls_manifest_media_uri_line_with_media_url(
    line: &str,
    manifest_dir: &FsPath,
    media_url: &dyn Fn(&str) -> String,
) -> String {
    let Some(uri_start) = line.find("URI=\"") else {
        return line.to_string();
    };
    let value_start = uri_start + 5;
    let Some(value_end_offset) = line[value_start..].find('"') else {
        return line.to_string();
    };
    let value_end = value_start + value_end_offset;
    let rewritten_uri = rewrite_hls_manifest_reference_with_media_url(
        &line[value_start..value_end],
        manifest_dir,
        media_url,
    );
    format!(
        "{}URI=\"{}\"{}",
        &line[..uri_start],
        rewritten_uri,
        &line[value_end + 1..]
    )
}

fn rewrite_preview_vtt_body_with_media_url(
    body: &str,
    relative_path: &str,
    media_url: &dyn Fn(&str) -> String,
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
                    rewrite_hls_manifest_reference_with_media_url(
                        reference,
                        &manifest_dir,
                        media_url
                    ),
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
        return Ok((
            [
                (header::CONTENT_TYPE, content_type),
                (
                    header::CACHE_CONTROL,
                    cache_control_for_media(&relative_path),
                ),
            ],
            Body::from(body),
        )
            .into_response());
    }

    if relative_path.ends_with(".vtt") {
        if let Some(playback_token) = query.playback_token.as_deref() {
            let text = load_rewritable_text_track(&full_path, &relative_path, file_exists).await?;
            let media_url = |path: &str| state.storage.playback_media_url(path, playback_token);
            let rewritten =
                rewrite_preview_vtt_body_with_media_url(&text, &relative_path, &media_url)?;
            return Ok((
                [
                    (header::CONTENT_TYPE, content_type),
                    (
                        header::CACHE_CONTROL,
                        cache_control_for_media(&relative_path),
                    ),
                ],
                Body::from(rewritten),
            )
                .into_response());
        }
    }

    stream_media_file(&full_path, &relative_path, content_type, &headers).await
}

async fn load_rewritable_text_track(
    full_path: &FsPath,
    relative_path: &str,
    file_exists: bool,
) -> AppResult<String> {
    let metadata = tokio::fs::metadata(full_path).await.map_err(|error| {
        warn!(
            relative_path = %relative_path,
            full_path = %full_path.display(),
            file_exists,
            error = %error,
            "media vtt metadata read failed"
        );
        if error.kind() == std::io::ErrorKind::NotFound {
            AppError::NotFound
        } else {
            AppError::Io(error)
        }
    })?;
    if metadata.len() > MAX_REWRITTEN_TEXT_TRACK_BYTES {
        return Err(AppError::BadRequest(
            "text track is too large to rewrite inline".to_string(),
        ));
    }

    tokio::fs::read_to_string(full_path).await.map_err(|error| {
        warn!(
            relative_path = %relative_path,
            full_path = %full_path.display(),
            file_exists,
            error = %error,
            "media vtt read failed"
        );
        if error.kind() == std::io::ErrorKind::NotFound {
            AppError::NotFound
        } else {
            AppError::Io(error)
        }
    })
}

async fn stream_media_file(
    full_path: &FsPath,
    relative_path: &str,
    content_type: &'static str,
    headers: &HeaderMap,
) -> AppResult<Response> {
    let mut file = File::open(full_path).await.map_err(|error| {
        warn!(
            relative_path = %relative_path,
            full_path = %full_path.display(),
            error = %error,
            "media file open failed"
        );
        if error.kind() == std::io::ErrorKind::NotFound {
            AppError::NotFound
        } else {
            AppError::Io(error)
        }
    })?;
    let len = file.metadata().await?.len();
    let range = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| parse_single_byte_range(value, len));

    let mut response_headers = HeaderMap::new();
    response_headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    response_headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(cache_control_for_media(relative_path)),
    );

    let (status, body_len, stream): (StatusCode, u64, ReaderStream<tokio::io::Take<File>>) =
        if let Some((start, end)) = range {
            file.seek(SeekFrom::Start(start)).await?;
            let take_len = end.saturating_sub(start) + 1;
            response_headers.insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes {start}-{end}/{len}"))
                    .map_err(|error| AppError::Internal(error.to_string()))?,
            );
            (
                StatusCode::PARTIAL_CONTENT,
                take_len,
                ReaderStream::new(file.take(take_len)),
            )
        } else {
            (StatusCode::OK, len, ReaderStream::new(file.take(len)))
        };

    response_headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&body_len.to_string())
            .map_err(|error| AppError::Internal(error.to_string()))?,
    );

    let mut response = Body::from_stream(stream).into_response();
    *response.status_mut() = status;
    response.headers_mut().extend(response_headers);
    Ok(response)
}

fn parse_single_byte_range(value: &str, len: u64) -> Option<(u64, u64)> {
    let range = value.strip_prefix("bytes=")?;
    if range.contains(',') || len == 0 {
        return None;
    }
    let (start, end) = range.split_once('-')?;
    if start.is_empty() {
        let suffix = end.parse::<u64>().ok()?;
        if suffix == 0 {
            return None;
        }
        let start = len.saturating_sub(suffix);
        return Some((start, len - 1));
    }
    let start = start.parse::<u64>().ok()?;
    let end = if end.is_empty() {
        len - 1
    } else {
        end.parse::<u64>().ok()?.min(len - 1)
    };
    (start <= end && start < len).then_some((start, end))
}

fn cache_control_for_media(relative_path: &str) -> &'static str {
    if relative_path.ends_with(".m3u8") {
        if relative_path.contains("/live/") || relative_path.contains("live/") {
            "public, max-age=2, stale-while-revalidate=8"
        } else {
            "public, max-age=30, stale-while-revalidate=120"
        }
    } else if relative_path.ends_with(".vtt") {
        "public, max-age=3600, stale-while-revalidate=86400"
    } else if relative_path.ends_with(".ts")
        || relative_path.ends_with(".m4s")
        || relative_path.ends_with(".aac")
        || relative_path.ends_with(".mp4")
    {
        "public, max-age=31536000, immutable"
    } else if relative_path.ends_with(".jpg")
        || relative_path.ends_with(".jpeg")
        || relative_path.ends_with(".png")
    {
        "public, max-age=86400, stale-while-revalidate=604800"
    } else {
        "private, no-store"
    }
}

#[cfg(test)]
mod tests {
    use super::{cache_control_for_media, parse_single_byte_range};

    #[test]
    fn parses_single_byte_ranges() {
        assert_eq!(parse_single_byte_range("bytes=0-99", 1_000), Some((0, 99)));
        assert_eq!(
            parse_single_byte_range("bytes=950-", 1_000),
            Some((950, 999))
        );
        assert_eq!(
            parse_single_byte_range("bytes=-50", 1_000),
            Some((950, 999))
        );
        assert_eq!(parse_single_byte_range("bytes=100-50", 1_000), None);
        assert_eq!(parse_single_byte_range("items=0-1", 1_000), None);
    }

    #[test]
    fn assigns_cache_policy_by_media_class() {
        assert_eq!(
            cache_control_for_media("vod/film/segment-001.ts"),
            "public, max-age=31536000, immutable"
        );
        assert_eq!(
            cache_control_for_media("vod/film/thumb.vtt"),
            "public, max-age=3600, stale-while-revalidate=86400"
        );
        assert_eq!(
            cache_control_for_media("admin/report.bin"),
            "private, no-store"
        );
    }
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
            validate_playback_session_token_for_path(&state.db, playback_token, relative_path)
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
        let media_url = |path: &str| state.storage.playback_media_url(path, playback_token);
        Ok(raw
            .lines()
            .map(|line| {
                if line.is_empty() {
                    line.to_string()
                } else if line.starts_with("#EXT-X-MEDIA:") {
                    rewrite_hls_manifest_media_uri_line_with_media_url(
                        line,
                        &manifest_dir,
                        &media_url,
                    )
                } else if line.starts_with("#EXT-X-MAP:") || line.starts_with("#EXT-X-PART:") {
                    rewrite_hls_manifest_media_uri_line_with_media_url(
                        line,
                        &manifest_dir,
                        &media_url,
                    )
                } else if line.starts_with('#') {
                    line.to_string()
                } else {
                    rewrite_hls_manifest_reference_with_media_url(line, &manifest_dir, &media_url)
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

    let target =
        fetch_live_stream_playback_target(state.db.try_sqlite_adapter()?, &session.content_id)
            .await?;
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
    if let Some(identity) = optional_identity(&state.db, headers).await? {
        if let Some(creator_id) = identity.creator_id.as_deref() {
            if creator_can_access_media_path(
                state.db.try_sqlite_adapter()?,
                creator_id,
                relative_path,
            )
            .await?
            {
                return Ok(());
            }
        }
    }

    let playback_token = query
        .playback_token
        .as_deref()
        .ok_or(AppError::Unauthorized)?;
    let session =
        validate_playback_session_token_for_path(&state.db, playback_token, relative_path).await?;
    if session.content_kind == "live" {
        return Ok(());
    }

    let target = fetch_upload_playback_target_for_database(&state.db, &session.content_id).await?;
    if playback_path_allowed_for_asset(&target.asset, relative_path) {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}
