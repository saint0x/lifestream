use super::*;

pub(super) fn rewrite_hls_manifest_reference(
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

pub(super) fn normalize_relative_storage_path(path: &FsPath) -> PathBuf {
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

pub(super) fn rewrite_hls_manifest_media_uri_line(
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

pub(super) fn rewrite_preview_vtt_body(
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

pub(super) async fn serve_media_file(
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

pub(super) async fn authorize_media_request(
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

pub(super) async fn validate_playback_session_token_for_path(
    pool: &SqlitePool,
    playback_token: &str,
    relative_path: &str,
) -> AppResult<PlaybackSession> {
    let session =
        validate_playback_session_record_for_path(pool, playback_token, relative_path).await?;
    Ok(playback_session_from_record(&session))
}

pub(super) async fn creator_can_access_media_path(
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
        let extra_paths = fetch_media_asset_variants(pool, &asset.0)
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

pub(super) fn playback_path_allowed_for_asset(asset: &MediaAsset, relative_path: &str) -> bool {
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

pub(super) fn path_allowed_for_paths(
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

pub(super) async fn check_database(pool: &SqlitePool) -> AppResult<bool> {
    let db_ok: i64 = sqlx::query("SELECT 1").fetch_one(pool).await?.get(0);
    Ok(db_ok == 1)
}

pub(super) fn media_path_for_relative(
    state: &SharedState,
    relative_path: &str,
) -> std::path::PathBuf {
    state.media_root.join(relative_path)
}

pub(super) async fn ensure_parent_dir(path: &std::path::Path) -> AppResult<()> {
    let parent = path.parent().ok_or_else(|| {
        AppError::BadRequest("invalid media path without parent directory".to_string())
    })?;
    tokio::fs::create_dir_all(parent).await?;
    Ok(())
}

pub(super) async fn sha256_file(path: &std::path::Path) -> AppResult<String> {
    use tokio::io::AsyncReadExt;

    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        use sha2::Digest;
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub(super) fn sanitize_storage_key(input: &str) -> AppResult<String> {
    if input.is_empty() {
        return Err(AppError::BadRequest("storageKey is required".to_string()));
    }

    let mut normalized_parts = Vec::new();
    for component in std::path::Path::new(input).components() {
        match component {
            Component::Normal(part) => {
                let part = part
                    .to_str()
                    .ok_or_else(|| AppError::BadRequest("storageKey must be utf-8".to_string()))?;
                if part.trim().is_empty() {
                    return Err(AppError::BadRequest(
                        "storageKey contains an empty segment".to_string(),
                    ));
                }
                normalized_parts.push(part.to_string());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(AppError::BadRequest(
                    "storageKey must be a safe relative path".to_string(),
                ));
            }
        }
    }

    if normalized_parts.is_empty() {
        return Err(AppError::BadRequest(
            "storageKey must contain at least one path segment".to_string(),
        ));
    }

    Ok(normalized_parts.join("/"))
}

pub(super) fn require_upload_token(headers: &HeaderMap) -> AppResult<String> {
    let value = headers
        .get("x-upload-token")
        .ok_or(AppError::Unauthorized)?
        .to_str()
        .map_err(|_| AppError::Unauthorized)?
        .trim()
        .to_string();

    if value.is_empty() {
        return Err(AppError::Unauthorized);
    }

    Ok(value)
}

pub(super) fn require_ingest_token(headers: &HeaderMap) -> AppResult<String> {
    let value = headers
        .get("x-ingest-token")
        .ok_or(AppError::Unauthorized)?
        .to_str()
        .map_err(|_| AppError::Unauthorized)?
        .trim()
        .to_string();

    if value.is_empty() {
        return Err(AppError::Unauthorized);
    }

    Ok(value)
}

pub(super) async fn validate_upload_ingest_token(
    pool: &SqlitePool,
    creator_id: &str,
    job_id: &str,
    upload_token: &str,
) -> AppResult<()> {
    let token_hash = crate::auth::hash_token(upload_token);
    let exists = sqlx::query(
        "SELECT 1 FROM upload_job_ingest_sessions WHERE creator_id = ? AND job_id = ? AND upload_token_hash = ?",
    )
    .bind(creator_id)
    .bind(job_id)
    .bind(token_hash)
    .fetch_optional(pool)
    .await?
    .is_some();

    if exists {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}

pub(super) async fn fetch_playback_session_by_id(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<PlaybackSession> {
    let session = fetch_playback_session_record_by_id(pool, session_id).await?;
    Ok(playback_session_from_record(&session))
}

pub(super) async fn fetch_admin_playback_sessions(
    pool: &SqlitePool,
    creator_filter: Option<&str>,
    content_filter: Option<&str>,
    state_filter: Option<&str>,
    limit: i64,
) -> AppResult<Vec<AdminPlaybackSessionRecord>> {
    reconcile_playback_sessions_for_read(pool, creator_filter, content_filter, None).await?;
    let limit = limit.clamp(1, 250);
    let now = Utc::now().to_rfc3339();
    let rows = match state_filter {
        Some("active") | Some("valid") => {
            sqlx::query(
                r#"
                SELECT id
                FROM playback_sessions
                WHERE (?1 IS NULL OR creator_id = ?1)
                  AND (?2 IS NULL OR content_id = ?2)
                  AND expires_at > ?3
                ORDER BY created_at DESC
                LIMIT ?4
                "#,
            )
            .bind(creator_filter)
            .bind(content_filter)
            .bind(&now)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        Some("expired") | Some("invalid") => {
            sqlx::query(
                r#"
                SELECT id
                FROM playback_sessions
                WHERE (?1 IS NULL OR creator_id = ?1)
                  AND (?2 IS NULL OR content_id = ?2)
                  AND expires_at <= ?3
                ORDER BY created_at DESC
                LIMIT ?4
                "#,
            )
            .bind(creator_filter)
            .bind(content_filter)
            .bind(&now)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        Some(_) | None => {
            sqlx::query(
                r#"
                SELECT id
                FROM playback_sessions
                WHERE (?1 IS NULL OR creator_id = ?1)
                  AND (?2 IS NULL OR content_id = ?2)
                ORDER BY created_at DESC
                LIMIT ?3
                "#,
            )
            .bind(creator_filter)
            .bind(content_filter)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
    };

    let mut sessions = Vec::with_capacity(rows.len());
    for row in rows {
        let session_id: String = row.get("id");
        sessions.push(fetch_admin_playback_session_record(pool, &session_id).await?);
    }
    Ok(sessions)
}

pub(super) async fn fetch_admin_playback_session_record(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<AdminPlaybackSessionRecord> {
    reconcile_playback_sessions_for_read(pool, None, None, Some(session_id)).await?;
    let mut session = fetch_playback_session_record_by_id(pool, session_id).await?;
    let now = Utc::now().to_rfc3339();
    let mut active = session.expires_at > now;
    let valid_access = if active {
        validate_existing_playback_session_access(pool, &session, None).await?
    } else {
        false
    };
    if active && !valid_access {
        expire_playback_session_by_id(pool, session_id).await?;
        session = fetch_playback_session_record_by_id(pool, session_id).await?;
        active = false;
    }
    Ok(AdminPlaybackSessionRecord {
        session: playback_session_from_record(&session),
        user_id: session.user_id.clone(),
        creator_id: session.creator_id.clone(),
        asset_id: session.asset_id.clone(),
        active,
        valid_access,
    })
}

pub(super) async fn validate_playback_session(
    pool: &SqlitePool,
    session_id: &str,
    playback_token: &str,
) -> AppResult<PlaybackSession> {
    let session = validate_playback_session_record(pool, session_id, playback_token).await?;
    Ok(playback_session_from_record(&session))
}

pub(super) fn parse_ffprobe_ratio(value: &str) -> Option<f64> {
    let (numerator, denominator) = value.split_once('/')?;
    let numerator = numerator.parse::<f64>().ok()?;
    let denominator = denominator.parse::<f64>().ok()?;
    if denominator == 0.0 {
        return None;
    }
    Some(numerator / denominator)
}

pub(super) fn media_api_url(relative_path: &str) -> String {
    format!("/api/v1/media/{relative_path}")
}

pub(super) fn sanitize_slug(input: &str) -> AppResult<String> {
    let slug = slugify(input);
    if slug.is_empty() {
        return Err(AppError::BadRequest("slug is required".to_string()));
    }
    if slug.len() > 120 {
        return Err(AppError::BadRequest("slug is too long".to_string()));
    }
    Ok(slug)
}

pub(super) fn slugify(input: &str) -> String {
    let mut slug = String::with_capacity(input.len());
    let mut previous_dash = false;
    for character in input.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

pub(super) fn media_content_type(relative_path: &str) -> &'static str {
    match PathBuf::from(relative_path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
    {
        "m3u8" => "application/vnd.apple.mpegurl",
        "ts" => "video/mp2t",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "aac" => "audio/aac",
        "mp3" => "audio/mpeg",
        _ => "application/octet-stream",
    }
}
