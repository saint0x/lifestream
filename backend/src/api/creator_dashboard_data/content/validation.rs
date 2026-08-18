use super::*;

pub(crate) fn validate_upload_visibility(visibility: &str) -> AppResult<()> {
    match visibility {
        "public" | "unlisted" | "private" => Ok(()),
        _ => Err(AppError::BadRequest(
            "unsupported upload visibility".to_string(),
        )),
    }
}

pub(crate) fn validate_upload_job_kind(kind: &str) -> AppResult<()> {
    match kind {
        "film" | "episode" | "clip" | "trailer" | "video" | "vod" | "live_archive" => Ok(()),
        other => Err(AppError::BadRequest(format!(
            "unsupported upload job kind: {other}"
        ))),
    }
}

pub(crate) fn validate_upload_job_source_type(source_type: &str) -> AppResult<()> {
    match source_type {
        "resumable-upload" | "live-archive" | "studio-export" => Ok(()),
        other => Err(AppError::BadRequest(format!(
            "unsupported upload job source type: {other}"
        ))),
    }
}

pub(crate) fn derive_upload_lifecycle_status(
    current_status: &str,
    visibility: &str,
    release_at: Option<&str>,
    now: &str,
) -> AppResult<String> {
    if current_status == "taken_down" {
        return Ok("taken_down".to_string());
    }
    match visibility {
        "private" => Ok("draft".to_string()),
        "public" | "unlisted" => {
            if release_at.is_some_and(|release_at| release_at > now) {
                Ok("scheduled".to_string())
            } else {
                Ok("published".to_string())
            }
        }
        _ => Err(AppError::BadRequest(
            "unsupported upload visibility".to_string(),
        )),
    }
}

pub(crate) fn validate_bulk_upload_action(upload: &Upload, action: &str) -> AppResult<()> {
    match action {
        "archive" => {
            if upload.status == "processing" {
                return Err(AppError::BadRequest(
                    "processing uploads cannot be archived".to_string(),
                ));
            }
            if upload.status == "taken_down" {
                return Err(AppError::BadRequest(
                    "taken-down uploads cannot be archived".to_string(),
                ));
            }
            Ok(())
        }
        "make_public" | "make_unlisted" => {
            if upload.status == "processing" || upload.status == "taken_down" {
                return Err(AppError::BadRequest(
                    "processing or taken-down uploads cannot change public visibility".to_string(),
                ));
            }
            Ok(())
        }
        "delete" => {
            if !matches!(upload.status.as_str(), "draft" | "archived" | "taken_down") {
                return Err(AppError::BadRequest(
                    "only draft, archived, or taken-down uploads can be deleted".to_string(),
                ));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
