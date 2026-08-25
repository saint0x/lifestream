use serde_json::json;
use vanta_obs_backend::{
    native::package::{
        build_all_platform_packages, build_current_platform_packages, package_states,
        verify_distribution_packages,
    },
    release::production_readiness_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("build") => {
            let reports = build_current_platform_packages()?;
            println!("{}", serde_json::to_string_pretty(&reports)?);
        }
        Some("build-all") => {
            let reports = build_all_platform_packages()?;
            println!("{}", serde_json::to_string_pretty(&reports)?);
        }
        Some("status") | None => {
            println!("{}", serde_json::to_string_pretty(&package_states())?);
        }
        Some("release-readiness") => {
            let strict = args.any(|arg| arg == "--strict");
            let report = production_readiness_report();
            let blocked = report.get("status").and_then(|value| value.as_str()) != Some("ready");
            println!("{}", serde_json::to_string_pretty(&report)?);
            if strict && blocked {
                std::process::exit(1);
            }
        }
        Some("verify-distribution") => {
            let strict = args.any(|arg| arg == "--strict");
            let reports = verify_distribution_packages()?;
            let blocked = reports.iter().any(|report| report.status != "ready");
            println!("{}", serde_json::to_string_pretty(&reports)?);
            if strict && blocked {
                std::process::exit(1);
            }
        }
        Some(command) => {
            eprintln!(
                "{}",
                json!({
                    "error": "unsupported command",
                    "command": command,
                    "usage": "vanta-native-package status | build | build-all | release-readiness [--strict] | verify-distribution [--strict]"
                })
            );
            std::process::exit(2);
        }
    }
    Ok(())
}
