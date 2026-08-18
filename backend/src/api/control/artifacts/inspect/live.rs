use super::*;

pub(super) async fn archive_artifact_exists(
    state: &SharedState,
    relative_path: &str,
) -> AppResult<bool> {
    tokio::fs::try_exists(media_path_for_relative(state, relative_path))
        .await
        .map_err(AppError::Io)
}

pub(super) async fn validate_manifest_artifact(
    state: &SharedState,
    relative_path: &str,
) -> AppResult<Option<String>> {
    let manifest_path = media_path_for_relative(state, relative_path);
    let metadata = match tokio::fs::metadata(&manifest_path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Some(format!(
                "runtime manifest {relative_path} does not exist on disk"
            )));
        }
        Err(error) => return Err(AppError::Io(error)),
    };
    if !metadata.is_file() {
        return Ok(Some(format!(
            "runtime manifest {relative_path} is not a regular file"
        )));
    }
    if metadata.len() == 0 {
        return Ok(Some(format!("runtime manifest {relative_path} is empty")));
    }
    let body = tokio::fs::read_to_string(&manifest_path)
        .await
        .map_err(AppError::Io)?;
    let trimmed = body.trim_start();
    if !trimmed.starts_with("#EXTM3U") {
        return Ok(Some(format!(
            "runtime manifest {relative_path} is not a valid HLS playlist"
        )));
    }
    Ok(None)
}

pub(super) async fn validate_archive_artifact(
    state: &SharedState,
    relative_path: &str,
) -> AppResult<Option<String>> {
    let archive_path = media_path_for_relative(state, relative_path);
    let metadata = match tokio::fs::metadata(&archive_path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Some(format!(
                "runtime archive {relative_path} does not exist on disk"
            )));
        }
        Err(error) => return Err(AppError::Io(error)),
    };
    if !metadata.is_file() {
        return Ok(Some(format!(
            "runtime archive {relative_path} is not a regular file"
        )));
    }
    if metadata.len() == 0 {
        return Ok(Some(format!("runtime archive {relative_path} is empty")));
    }
    Ok(None)
}
