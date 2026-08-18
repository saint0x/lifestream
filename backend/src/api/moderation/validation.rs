use super::*;

pub(crate) fn validate_creator_moderator_role(role: &str) -> AppResult<()> {
    match role {
        "mod" | "admin" => Ok(()),
        _ => Err(AppError::BadRequest(
            "unsupported moderator role".to_string(),
        )),
    }
}

pub(crate) fn validate_slow_mode_seconds(seconds: i64) -> AppResult<()> {
    if !(0..=300).contains(&seconds) {
        return Err(AppError::BadRequest(
            "slowModeSeconds must be between 0 and 300".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_auto_mod_level(level: &str) -> AppResult<()> {
    match level {
        "off" | "standard" | "strict" => Ok(()),
        _ => Err(AppError::BadRequest(
            "autoModLevel must be one of off, standard, or strict".to_string(),
        )),
    }
}

pub(crate) fn validate_live_moderation_action_type(action_type: &str) -> AppResult<()> {
    match action_type {
        "mute" | "ban" | "shadowban" => Ok(()),
        _ => Err(AppError::BadRequest(
            "unsupported live moderation action type".to_string(),
        )),
    }
}

pub(crate) fn validate_live_report_status(status: &str) -> AppResult<()> {
    match status {
        "open" | "reviewing" | "resolved" | "dismissed" => Ok(()),
        _ => Err(AppError::BadRequest(
            "unsupported live stream report status".to_string(),
        )),
    }
}

pub(crate) fn validate_creator_enforcement_scope(scope: &str) -> AppResult<()> {
    match scope.trim() {
        "live_streaming" | "uploads" | "collaboration" | "monetization" | "payouts" => Ok(()),
        other => Err(AppError::BadRequest(format!(
            "unsupported creator enforcement scope: {other}"
        ))),
    }
}
