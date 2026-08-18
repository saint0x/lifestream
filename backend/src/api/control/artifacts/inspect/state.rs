pub(super) fn artifact_state_label(
    status: &str,
    valid: bool,
    invalid: bool,
    persisted_relative_path: Option<&str>,
    expected_relative_path: Option<&str>,
) -> String {
    if valid {
        return "valid".to_string();
    }
    if invalid {
        return "invalid".to_string();
    }
    if matches!(status, "ready" | "complete" | "finalizing") && persisted_relative_path.is_none() {
        return "missing".to_string();
    }
    if persisted_relative_path.is_some() && expected_relative_path != persisted_relative_path {
        return "drifted".to_string();
    }
    "pending".to_string()
}

pub(super) fn declared_artifact_state_label(
    status: &str,
    persisted_relative_path: Option<&str>,
    expected_relative_path: Option<&str>,
) -> String {
    if !matches!(status, "ready" | "complete" | "finalizing") {
        return "pending".to_string();
    }
    match persisted_relative_path {
        None => "missing".to_string(),
        Some(path) if Some(path) != expected_relative_path => "drifted".to_string(),
        Some(_) => "declared".to_string(),
    }
}

pub(super) fn declared_artifact_issue(
    status: &str,
    persisted_relative_path: Option<&str>,
    expected_relative_path: Option<&str>,
    artifact_kind: &str,
) -> Option<String> {
    if !matches!(status, "ready" | "complete" | "finalizing") {
        return None;
    }
    match persisted_relative_path {
        None => Some(format!(
            "{artifact_kind} is required for the current runtime state but no persisted path is present"
        )),
        Some(path) if Some(path) != expected_relative_path => Some(format!(
            "{artifact_kind} path {path} does not match the backend-owned runtime path {}",
            expected_relative_path.unwrap_or_default()
        )),
        Some(_) => None,
    }
}
