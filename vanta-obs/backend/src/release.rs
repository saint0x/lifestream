use axum::{Json, Router, extract::State, routing::get};
use serde_json::{Value, json};

use crate::{
    AppState, native::package::package_states, obs::vendor::validate_vendored_obs_approval,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/release/readiness", get(readiness))
}

async fn readiness(State(_state): State<AppState>) -> Json<Value> {
    Json(production_readiness_report())
}

pub fn production_readiness_report() -> Value {
    let packages = package_states();
    let package_blockers = packages
        .iter()
        .filter(|package| package.signing_required)
        .filter(|package| {
            package.platform == current_release_platform()
                || matches!(package.platform.as_str(), "macos" | "windows")
        })
        .flat_map(|package| {
            let mut blockers = Vec::new();
            if !package.artifact_present {
                blockers.push("missing_helper_binary");
            }
            if !package.installer_present {
                blockers.push("missing_installer");
            }
            if !package.helper_signature_verified {
                blockers.push("helper_signature_unverified");
            }
            if !package.installer_signature_verified {
                blockers.push("installer_signature_unverified");
            }
            if package.notarization_required && !package.notarization_verified {
                blockers.push("notarization_unverified");
            }
            if package.system_audio_validation_required && !package.system_audio_validation_verified
            {
                blockers.push("system_audio_validation_missing");
            }
            blockers.into_iter().map(|blocker| {
                json!({
                    "gate": blocker,
                    "package_id": package.package_id,
                    "helper_kind": package.helper_kind,
                    "platform": package.platform,
                    "diagnostics": package.diagnostics
                })
            })
        })
        .collect::<Vec<_>>();

    let vendor_track_requested = env_bool("VANTA_OBS_VENDOR_TRACK_REQUESTED");
    let vendor_evidence = json!({
        "gpl_legal_approval": env_bool("VANTA_OBS_VENDOR_GPL_APPROVED"),
        "open_source_distribution_posture": env_bool("VANTA_OBS_VENDOR_DISTRIBUTION_APPROVED"),
        "build_isolation_plan": env_bool("VANTA_OBS_VENDOR_BUILD_ISOLATION_APPROVED"),
        "upstream_patch_strategy": env_bool("VANTA_OBS_VENDOR_PATCH_STRATEGY_APPROVED"),
        "security_update_workflow": env_bool("VANTA_OBS_VENDOR_SECURITY_APPROVED"),
        "reproducible_macos_build": env_bool("VANTA_OBS_VENDOR_REPRO_MACOS_APPROVED"),
        "reproducible_windows_build": env_bool("VANTA_OBS_VENDOR_REPRO_WINDOWS_APPROVED"),
        "commercial_removal_plan": env_bool("VANTA_OBS_VENDOR_REMOVAL_PLAN_APPROVED")
    });
    let vendor_gate = if vendor_track_requested {
        match validate_vendored_obs_approval(&vendor_evidence) {
            Ok(approval) => json!({
                "status": "approved",
                "requested": true,
                "approval": approval
            }),
            Err(missing) => json!({
                "status": "blocked",
                "requested": true,
                "missing_approvals": missing
            }),
        }
    } else {
        json!({
            "status": "not_requested",
            "requested": false,
            "default_policy": "vendored_obs_blocked_without_explicit_approval"
        })
    };

    let mut blockers = package_blockers;
    if vendor_gate.get("status").and_then(Value::as_str) == Some("blocked") {
        blockers.push(json!({
            "gate": "vendored_obs_approval_missing",
            "details": vendor_gate
        }));
    }

    json!({
        "status": if blockers.is_empty() { "ready" } else { "blocked" },
        "release_kind": "vanta_obs_desktop_distribution",
        "native_packages": packages,
        "vendor_track": vendor_gate,
        "blockers": blockers
    })
}

fn env_bool(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn current_release_platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macos",
        "windows" => "windows",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::production_readiness_report;

    #[test]
    fn production_readiness_blocks_unsigned_distribution() {
        let report = production_readiness_report();
        assert_eq!(report["status"], "blocked");
        assert!(
            report["blockers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|blocker| blocker["gate"] == "installer_signature_unverified")
        );
        assert_eq!(report["vendor_track"]["status"], "not_requested");
    }
}
