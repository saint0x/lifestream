use serde_json::{Value, json};

const REQUIRED_APPROVALS: [&str; 8] = [
    "gpl_legal_approval",
    "open_source_distribution_posture",
    "build_isolation_plan",
    "upstream_patch_strategy",
    "security_update_workflow",
    "reproducible_macos_build",
    "reproducible_windows_build",
    "commercial_removal_plan",
];

pub fn vendored_obs_policy() -> Value {
    json!({
        "status": "blocked_without_explicit_approval",
        "default_product_shell": "vanta_native_web_studio",
        "allowed_before_approval": [
            "obs_websocket_interop",
            "obs_scene_collection_import_export",
            "lightweight_vanta_obs_companion_plugin",
            "implementation_study_without_source_copy"
        ],
        "blocked_before_approval": [
            "vendored_obs_studio_source",
            "linked_libobs_runtime",
            "qt_obs_shell_embedding",
            "generic_obs_plugin_host"
        ],
        "required_approvals": REQUIRED_APPROVALS,
        "value_filter": {
            "must_support": [
                "vanta_live_studio",
                "sponsor_proof",
                "guest_collaboration",
                "recording_replay_archive",
                "publishing_handoff",
                "live_ops_recovery"
            ],
            "reject": [
                "parity_for_its_own_sake",
                "novelty_filters",
                "generic_settings_sprawl",
                "duplicated_obs_ui_shell"
            ]
        }
    })
}

pub fn validate_vendored_obs_approval(evidence: &Value) -> Result<Value, Vec<String>> {
    let missing = REQUIRED_APPROVALS
        .iter()
        .filter(|approval| {
            !evidence
                .get(**approval)
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .map(|approval| approval.to_string())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(missing);
    }
    Ok(json!({
        "status": "approved_for_isolated_vendor_track",
        "allowed_scope": "isolated_optional_obs_vendor_track",
        "product_shell": "vanta_native_web_studio",
        "required_boundary": "no_vanta_core_dependency_on_qt_obs_shell",
        "removal_plan_required": true
    }))
}

#[cfg(test)]
mod tests {
    use super::{REQUIRED_APPROVALS, validate_vendored_obs_approval, vendored_obs_policy};
    use serde_json::json;

    #[test]
    fn blocks_vendored_obs_until_all_approval_evidence_exists() {
        let policy = vendored_obs_policy();
        assert_eq!(policy["status"], "blocked_without_explicit_approval");
        assert!(
            policy["blocked_before_approval"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item == "linked_libobs_runtime")
        );

        let denied = validate_vendored_obs_approval(&json!({
            "gpl_legal_approval": true,
            "open_source_distribution_posture": true
        }))
        .unwrap_err();
        assert!(denied.contains(&"build_isolation_plan".to_string()));
        assert!(denied.contains(&"commercial_removal_plan".to_string()));

        let complete = REQUIRED_APPROVALS
            .iter()
            .map(|approval| (approval.to_string(), json!(true)))
            .collect::<serde_json::Map<_, _>>();
        let approved = validate_vendored_obs_approval(&json!(complete)).unwrap();
        assert_eq!(approved["status"], "approved_for_isolated_vendor_track");
        assert_eq!(approved["product_shell"], "vanta_native_web_studio");
    }
}
