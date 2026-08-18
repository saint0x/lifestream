use super::*;

pub(crate) async fn check_database(pool: &SqlitePool) -> AppResult<bool> {
    let db_ok: i64 = sqlx::query("SELECT 1").fetch_one(pool).await?.get(0);
    Ok(db_ok == 1)
}

pub(crate) fn media_path_for_relative(
    state: &SharedState,
    relative_path: &str,
) -> std::path::PathBuf {
    state.media_root.join(relative_path)
}

pub(crate) async fn ensure_parent_dir(path: &std::path::Path) -> AppResult<()> {
    let parent = path.parent().ok_or_else(|| {
        AppError::BadRequest("invalid media path without parent directory".to_string())
    })?;
    tokio::fs::create_dir_all(parent).await?;
    Ok(())
}

pub(crate) async fn sha256_file(path: &std::path::Path) -> AppResult<String> {
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

pub(crate) fn sanitize_storage_key(input: &str) -> AppResult<String> {
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

pub(crate) fn parse_ffprobe_ratio(value: &str) -> Option<f64> {
    let (numerator, denominator) = value.split_once('/')?;
    let numerator = numerator.parse::<f64>().ok()?;
    let denominator = denominator.parse::<f64>().ok()?;
    if denominator == 0.0 {
        return None;
    }
    Some(numerator / denominator)
}

pub(crate) fn media_api_url(relative_path: &str) -> String {
    format!("/api/v1/media/{relative_path}")
}

pub(crate) fn sanitize_slug(input: &str) -> AppResult<String> {
    let slug = slugify(input);
    if slug.is_empty() {
        return Err(AppError::BadRequest("slug is required".to_string()));
    }
    if slug.len() > 120 {
        return Err(AppError::BadRequest("slug is too long".to_string()));
    }
    Ok(slug)
}

pub(crate) fn slugify(input: &str) -> String {
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

pub(crate) fn media_content_type(relative_path: &str) -> &'static str {
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
