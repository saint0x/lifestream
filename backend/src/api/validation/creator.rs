use super::*;

pub(crate) fn transition_creator_operational_status(
    current: &str,
    submit_requested: bool,
    terminal_approved: &str,
    terminal_blocked: &str,
) -> AppResult<String> {
    if current == terminal_approved {
        return Ok(current.to_string());
    }
    if current == terminal_blocked && submit_requested {
        return Ok("submitted".to_string());
    }
    if submit_requested {
        return Ok(match current {
            "pending" | "rejected" | "disabled" => "submitted".to_string(),
            "in_review" => "in_review".to_string(),
            "submitted" => "submitted".to_string(),
            other => other.to_string(),
        });
    }
    Ok(current.to_string())
}
