use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativePackageManifest {
    pub package_id: String,
    pub helper_kind: String,
    pub platform: String,
    pub display_name: String,
    pub binary_path: String,
    pub install_path: String,
    pub transports: Vec<String>,
    pub signing: NativePackageSigning,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativePackageSigning {
    pub required: bool,
    pub identity_env: String,
    pub notarization_required: bool,
    pub entitlement_profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativePackageState {
    pub package_id: String,
    pub helper_kind: String,
    pub platform: String,
    pub display_name: String,
    pub binary_path: String,
    pub install_path: String,
    pub transports: Vec<String>,
    pub permissions: Vec<String>,
    pub signing_required: bool,
    pub notarization_required: bool,
    pub entitlement_profile: String,
    pub signing_identity_configured: bool,
    pub artifact_present: bool,
    pub status: String,
    pub diagnostics: Vec<String>,
    pub build_manifest_path: String,
    pub helper_signature_verified: bool,
    pub installer_present: bool,
    pub installer_signature_verified: bool,
    pub notarization_verified: bool,
    pub system_audio_validation_required: bool,
    pub system_audio_validation_verified: bool,
    pub system_audio_validation_artifact: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NativePackageBuildReport {
    pub package_id: String,
    pub helper_kind: String,
    pub platform: String,
    pub binary_path: String,
    pub install_path: String,
    pub build_manifest_path: String,
    pub helper_signed: bool,
    pub helper_signing_identity: String,
    pub helper_sha256: String,
    pub installer_created: bool,
    pub installer_signed: bool,
    pub notarization_required: bool,
    pub notarization_verified: bool,
    pub system_audio_validation_required: bool,
    pub system_audio_validation_verified: bool,
    pub system_audio_validation_artifact: String,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NativePackageVerificationReport {
    pub package_id: String,
    pub helper_kind: String,
    pub platform: String,
    pub binary_path: String,
    pub install_path: String,
    pub build_manifest_path: String,
    pub artifact_present: bool,
    pub installer_present: bool,
    pub helper_sha256_matches_manifest: bool,
    pub helper_production_signature_verified: bool,
    pub installer_production_signature_verified: bool,
    pub notarization_verified: bool,
    pub system_audio_validation_required: bool,
    pub system_audio_validation_verified: bool,
    pub status: String,
    pub diagnostics: Vec<String>,
}

pub fn package_states() -> Vec<NativePackageState> {
    manifests()
        .into_iter()
        .map(|manifest| state_for_manifest(&manifest))
        .collect()
}

pub fn verify_distribution_packages()
-> Result<Vec<NativePackageVerificationReport>, NativePackageBuildError> {
    manifests()
        .into_iter()
        .map(|manifest| verify_distribution_package(&manifest))
        .collect()
}

pub fn build_current_platform_packages()
-> Result<Vec<NativePackageBuildReport>, NativePackageBuildError> {
    let platform = current_platform();
    build_platform_packages(platform)
}

pub fn build_all_platform_packages()
-> Result<Vec<NativePackageBuildReport>, NativePackageBuildError> {
    manifests()
        .into_iter()
        .map(|manifest| build_package(&manifest))
        .collect::<Result<Vec<_>, _>>()
}

pub fn build_platform_packages(
    platform: &str,
) -> Result<Vec<NativePackageBuildReport>, NativePackageBuildError> {
    manifests()
        .into_iter()
        .filter(|manifest| manifest.platform == platform)
        .map(|manifest| build_package(&manifest))
        .collect::<Result<Vec<_>, _>>()
}

pub fn current_platform_package(helper_kind: &str) -> Option<NativePackageState> {
    let platform = current_platform();
    package_states()
        .into_iter()
        .find(|package| package.helper_kind == helper_kind && package.platform == platform)
}

pub fn package_health(helper_kind: &str) -> Value {
    current_platform_package(helper_kind)
        .map(|package| {
            json!({
                "package_id": package.package_id,
                "platform": package.platform,
                "status": package.status,
                "artifact_present": package.artifact_present,
                "installer_present": package.installer_present,
                "signing_required": package.signing_required,
                "signing_identity_configured": package.signing_identity_configured,
                "helper_signature_verified": package.helper_signature_verified,
                "installer_signature_verified": package.installer_signature_verified,
                "notarization_verified": package.notarization_verified,
                "system_audio_validation_required": package.system_audio_validation_required,
                "system_audio_validation_verified": package.system_audio_validation_verified,
                "system_audio_validation_artifact": package.system_audio_validation_artifact,
                "build_manifest_path": package.build_manifest_path,
                "transports": package.transports,
                "permissions": package.permissions,
                "diagnostics": package.diagnostics
            })
        })
        .unwrap_or_else(|| {
            json!({
                "status": "unsupported_platform",
                "platform": current_platform(),
                "helper_kind": helper_kind
            })
        })
}

pub fn fallback_plan() -> Value {
    let platform = current_platform();
    let current_packages = package_states()
        .into_iter()
        .filter(|package| package.platform == platform)
        .collect::<Vec<_>>();
    let blocked = current_packages
        .iter()
        .filter(|package| package.status != "ready")
        .map(|package| {
            json!({
                "helper_kind": package.helper_kind,
                "package_id": package.package_id,
                "status": package.status,
                "diagnostics": package.diagnostics
            })
        })
        .collect::<Vec<_>>();
    let native_ready = blocked.is_empty() && !current_packages.is_empty();

    json!({
        "status": if native_ready { "native_ready" } else { "browser_preview_external_ingest" },
        "native_ready": native_ready,
        "platform": platform,
        "browser_preview": {
            "available": true,
            "mode": "canvas_capture_stream",
            "scope": "local preview/program composition"
        },
        "external_ingest": {
            "available": true,
            "protocols": ["rtmp", "srt", "webrtc"],
            "steps": [
                "Run Vanta Studio browser preview for scenes, cues, moderation, sponsor proof, and runtime control.",
                "Use the Runtime panel stream key and ingest URL in OBS, Streamlabs, or a hardware encoder.",
                "Keep Vanta runtime open so archive, replay markers, sponsor proof, and live ops state stay authoritative."
            ]
        },
        "blocked_helpers": blocked
    })
}

fn state_for_manifest(manifest: &NativePackageManifest) -> NativePackageState {
    let binary_path = expand_root(&manifest.binary_path);
    let install_path = expand_root(&manifest.install_path);
    let build_manifest_path = build_manifest_path(manifest);
    let artifact_present = Path::new(&binary_path).is_file();
    let installer_present = Path::new(&install_path).is_file();
    let build_manifest = read_build_manifest(&build_manifest_path);
    let helper_signature_verified = build_manifest
        .as_ref()
        .and_then(|value| value.get("helper_signed"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let installer_signature_verified = build_manifest
        .as_ref()
        .and_then(|value| value.get("installer_signed"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let notarization_verified = build_manifest
        .as_ref()
        .and_then(|value| value.get("notarization_verified"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let system_audio_validation_required = requires_system_audio_validation(manifest);
    let system_audio_validation_reported = build_manifest
        .as_ref()
        .and_then(|value| value.get("system_audio_validation_verified"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let system_audio_validation_artifact = build_manifest
        .as_ref()
        .and_then(|value| value.get("system_audio_validation_artifact"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let system_audio_validation_verified = system_audio_validation_reported
        && validation_artifact_is_readable(&system_audio_validation_artifact);
    let signing_identity_configured = !manifest.signing.identity_env.is_empty()
        && env::var(&manifest.signing.identity_env).is_ok();
    let mut diagnostics = Vec::new();

    if !artifact_present {
        diagnostics.push(format!("missing helper binary at {binary_path}"));
    }
    if artifact_present && manifest.signing.required && !helper_signature_verified {
        diagnostics.push(format!(
            "helper binary at {binary_path} has no verified signature manifest"
        ));
    }
    if !installer_present {
        diagnostics.push(format!("missing helper installer at {install_path}"));
    }
    if installer_present && manifest.signing.required && !installer_signature_verified {
        diagnostics.push(format!(
            "installer at {install_path} has no verified production signature"
        ));
    }
    if manifest.signing.required && !signing_identity_configured {
        diagnostics.push(format!(
            "missing signing identity env {}",
            manifest.signing.identity_env
        ));
    }
    if manifest.signing.notarization_required && !notarization_verified {
        diagnostics.push("notarization has not been verified".to_string());
    }
    if manifest.transports.is_empty() {
        diagnostics.push("at least one helper transport is required".to_string());
    }
    if manifest.permissions.is_empty() {
        diagnostics.push("permission manifest is empty".to_string());
    }
    if requires_system_audio_validation(manifest) {
        if !manifest
            .permissions
            .iter()
            .any(|permission| permission == "screen-recording")
        {
            diagnostics.push("audio helper must declare screen-recording permission for ScreenCaptureKit system audio".to_string());
        }
        if !manifest
            .permissions
            .iter()
            .any(|permission| permission == "system-audio")
        {
            diagnostics.push(
                "audio helper must declare system-audio permission for native system audio"
                    .to_string(),
            );
        }
        if !system_audio_validation_verified {
            diagnostics.push("signed audio helper has no verified permission-granted ScreenCaptureKit system-audio artifact".to_string());
        }
        if system_audio_validation_reported && !system_audio_validation_verified {
            diagnostics.push(format!(
                "system audio validation artifact is missing or empty at {system_audio_validation_artifact}"
            ));
        }
    }

    let status = if manifest.platform != current_platform() {
        "available_for_other_platform"
    } else if !artifact_present || !installer_present {
        "missing_artifact"
    } else if manifest.signing.required
        && (!signing_identity_configured || !installer_signature_verified)
    {
        "missing_signing_identity"
    } else if manifest.signing.notarization_required && !notarization_verified {
        "missing_notarization"
    } else if system_audio_validation_required && !system_audio_validation_verified {
        "missing_system_audio_validation"
    } else {
        "ready"
    };

    NativePackageState {
        package_id: manifest.package_id.clone(),
        helper_kind: manifest.helper_kind.clone(),
        platform: manifest.platform.clone(),
        display_name: manifest.display_name.clone(),
        binary_path,
        install_path,
        transports: manifest.transports.clone(),
        permissions: manifest.permissions.clone(),
        signing_required: manifest.signing.required,
        notarization_required: manifest.signing.notarization_required,
        entitlement_profile: manifest.signing.entitlement_profile.clone(),
        signing_identity_configured,
        artifact_present,
        status: status.to_string(),
        diagnostics,
        build_manifest_path,
        helper_signature_verified,
        installer_present,
        installer_signature_verified,
        notarization_verified,
        system_audio_validation_required,
        system_audio_validation_verified,
        system_audio_validation_artifact,
    }
}

pub fn manifests() -> Vec<NativePackageManifest> {
    [
        include_str!("../../../native/capture/macos/package.json"),
        include_str!("../../../native/capture/windows/package.json"),
        include_str!("../../../native/encode/macos/package.json"),
        include_str!("../../../native/encode/windows/package.json"),
        include_str!("../../../native/replay/macos/package.json"),
        include_str!("../../../native/replay/windows/package.json"),
        include_str!("../../../native/audio/macos/package.json"),
        include_str!("../../../native/audio/windows/package.json"),
    ]
    .into_iter()
    .map(|raw| serde_json::from_str(raw).expect("native package manifests must be valid json"))
    .collect()
}

pub fn expand_root(path: &str) -> String {
    path.replace(
        "$VANTA_OBS_ROOT",
        env!("CARGO_MANIFEST_DIR").trim_end_matches("/backend"),
    )
}

fn requires_system_audio_validation(manifest: &NativePackageManifest) -> bool {
    manifest.helper_kind == "audio" && manifest.platform == "macos"
}

fn system_audio_validation_artifact(
    manifest: &NativePackageManifest,
    diagnostics: &mut Vec<String>,
) -> Result<(bool, String), NativePackageBuildError> {
    if !requires_system_audio_validation(manifest) {
        return Ok((false, String::new()));
    }

    let artifact = env::var("VANTA_SYSTEM_AUDIO_VALIDATION_ARTIFACT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_default();
    if artifact.is_empty() {
        diagnostics.push("set VANTA_SYSTEM_AUDIO_VALIDATION_ARTIFACT to a permission-granted ScreenCaptureKit system-audio M4A produced by the signed audio helper".to_string());
        return Ok((false, String::new()));
    }
    if !validation_artifact_is_readable(&artifact) {
        diagnostics.push(format!(
            "system audio validation artifact does not exist or is empty at {artifact}"
        ));
        return Ok((false, artifact));
    }
    Ok((true, artifact))
}

fn validation_artifact_is_readable(path: &str) -> bool {
    if path.trim().is_empty() {
        return false;
    }
    fs::metadata(Path::new(path))
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

fn verify_distribution_package(
    manifest: &NativePackageManifest,
) -> Result<NativePackageVerificationReport, NativePackageBuildError> {
    let binary_path = PathBuf::from(expand_root(&manifest.binary_path));
    let install_path = PathBuf::from(expand_root(&manifest.install_path));
    let build_manifest_path = PathBuf::from(build_manifest_path(manifest));
    let build_manifest = read_build_manifest(&build_manifest_path.to_string_lossy());
    let mut diagnostics = Vec::new();
    let artifact_present = binary_path.is_file();
    let installer_present = install_path.is_file();

    let expected_sha256 = build_manifest
        .as_ref()
        .and_then(|value| value.get("helper_sha256"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let helper_sha256_matches_manifest = if artifact_present && !expected_sha256.is_empty() {
        sha256_file(&binary_path)? == expected_sha256
    } else {
        false
    };
    if !helper_sha256_matches_manifest {
        diagnostics.push("helper artifact SHA-256 does not match build manifest".to_string());
    }

    let helper_manifest_signed = build_manifest
        .as_ref()
        .and_then(|value| value.get("helper_signed"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let helper_manifest_identity = build_manifest
        .as_ref()
        .and_then(|value| value.get("helper_signing_identity"))
        .and_then(Value::as_str)
        .unwrap_or("-");
    let helper_tool_verified = verify_platform_signature(
        &binary_path,
        manifest.platform.as_str(),
        "helper",
        &mut diagnostics,
    );
    let helper_production_signature_verified =
        helper_manifest_signed && helper_manifest_identity != "-" && helper_tool_verified;
    if !helper_production_signature_verified {
        diagnostics.push("helper artifact has no verified production signature".to_string());
    }

    let installer_manifest_signed = build_manifest
        .as_ref()
        .and_then(|value| value.get("installer_signed"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let installer_tool_verified = verify_installer_signature(
        &install_path,
        manifest.platform.as_str(),
        "installer",
        &mut diagnostics,
    );
    let installer_production_signature_verified =
        installer_manifest_signed && installer_tool_verified;
    if !installer_production_signature_verified {
        diagnostics.push("installer artifact has no verified production signature".to_string());
    }

    let notarization_reported = build_manifest
        .as_ref()
        .and_then(|value| value.get("notarization_verified"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let notarization_verified = if manifest.signing.notarization_required {
        notarization_reported && verify_macos_notarization_staple(&install_path, &mut diagnostics)
    } else {
        false
    };
    if manifest.signing.notarization_required && !notarization_verified {
        diagnostics.push("macOS installer has no verified notarization staple".to_string());
    }

    let system_audio_validation_required = requires_system_audio_validation(manifest);
    let system_audio_artifact = build_manifest
        .as_ref()
        .and_then(|value| value.get("system_audio_validation_artifact"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let system_audio_validation_reported = build_manifest
        .as_ref()
        .and_then(|value| value.get("system_audio_validation_verified"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let system_audio_validation_verified = !system_audio_validation_required
        || (system_audio_validation_reported
            && validation_artifact_is_readable(system_audio_artifact));
    if system_audio_validation_required && !system_audio_validation_verified {
        diagnostics.push(
            "signed audio helper has no verified ScreenCaptureKit system-audio artifact"
                .to_string(),
        );
    }

    if !artifact_present {
        diagnostics.push(format!(
            "missing helper artifact at {}",
            binary_path.display()
        ));
    }
    if !installer_present {
        diagnostics.push(format!(
            "missing installer artifact at {}",
            install_path.display()
        ));
    }

    let ready = artifact_present
        && installer_present
        && helper_sha256_matches_manifest
        && helper_production_signature_verified
        && installer_production_signature_verified
        && (!manifest.signing.notarization_required || notarization_verified)
        && system_audio_validation_verified;

    Ok(NativePackageVerificationReport {
        package_id: manifest.package_id.clone(),
        helper_kind: manifest.helper_kind.clone(),
        platform: manifest.platform.clone(),
        binary_path: binary_path.to_string_lossy().to_string(),
        install_path: install_path.to_string_lossy().to_string(),
        build_manifest_path: build_manifest_path.to_string_lossy().to_string(),
        artifact_present,
        installer_present,
        helper_sha256_matches_manifest,
        helper_production_signature_verified,
        installer_production_signature_verified,
        notarization_verified,
        system_audio_validation_required,
        system_audio_validation_verified,
        status: if ready { "ready" } else { "blocked" }.to_string(),
        diagnostics,
    })
}

fn build_package(
    manifest: &NativePackageManifest,
) -> Result<NativePackageBuildReport, NativePackageBuildError> {
    let source_binary = helper_source_binary()?;
    let binary_path = PathBuf::from(expand_root(&manifest.binary_path));
    let install_path = PathBuf::from(expand_root(&manifest.install_path));
    let manifest_path = PathBuf::from(build_manifest_path(manifest));
    let mut diagnostics = Vec::new();
    fs::create_dir_all(parent(&binary_path)?)?;
    fs::copy(&source_binary, &binary_path)?;

    let signing_identity =
        env::var(&manifest.signing.identity_env).unwrap_or_else(|_| "-".to_string());
    let helper_signed =
        sign_helper_binary(&binary_path, manifest, &signing_identity, &mut diagnostics)?;
    let helper_sha256 = sha256_file(&binary_path)?;

    let (installer_created, installer_signed) = build_installer(
        &binary_path,
        &install_path,
        manifest,
        &signing_identity,
        &mut diagnostics,
    )?;
    let notarization_verified = notarize_installer_if_required(
        &install_path,
        manifest,
        installer_created && installer_signed,
        &mut diagnostics,
    )?;
    let system_audio_validation_required = requires_system_audio_validation(manifest);
    let (system_audio_validation_verified, system_audio_validation_artifact) =
        system_audio_validation_artifact(manifest, &mut diagnostics)?;
    let report = NativePackageBuildReport {
        package_id: manifest.package_id.clone(),
        helper_kind: manifest.helper_kind.clone(),
        platform: manifest.platform.clone(),
        binary_path: binary_path.to_string_lossy().to_string(),
        install_path: install_path.to_string_lossy().to_string(),
        build_manifest_path: manifest_path.to_string_lossy().to_string(),
        helper_signed,
        helper_signing_identity: signing_identity,
        helper_sha256,
        installer_created,
        installer_signed,
        notarization_required: manifest.signing.notarization_required,
        notarization_verified,
        system_audio_validation_required,
        system_audio_validation_verified,
        system_audio_validation_artifact,
        diagnostics,
    };
    fs::create_dir_all(parent(&manifest_path)?)?;
    fs::write(&manifest_path, serde_json::to_vec_pretty(&report)?)?;
    Ok(report)
}

fn helper_source_binary() -> Result<PathBuf, NativePackageBuildError> {
    if let Some(path) = env::var("VANTA_NATIVE_HELPER_BINARY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
    {
        if path.is_file() {
            return Ok(path);
        }
        return Err(NativePackageBuildError::Command(format!(
            "VANTA_NATIVE_HELPER_BINARY points at missing file {}",
            path.display()
        )));
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest_dir
            .join("target")
            .join("debug")
            .join("vanta-native-helper"),
        manifest_dir
            .join("target")
            .join(format!("{}-apple-darwin", env::consts::ARCH))
            .join("debug")
            .join("vanta-native-helper"),
    ];
    for path in candidates {
        if path.is_file() {
            return Ok(path);
        }
    }
    Err(NativePackageBuildError::Command(
        "helper source binary does not exist; run `cargo build --bin vanta-native-helper` first or set VANTA_NATIVE_HELPER_BINARY".to_string(),
    ))
}

fn sign_macos_binary(
    binary_path: &Path,
    manifest: &NativePackageManifest,
    signing_identity: &str,
    diagnostics: &mut Vec<String>,
) -> Result<bool, NativePackageBuildError> {
    let mut command = Command::new("codesign");
    command
        .arg("--force")
        .arg("--timestamp=none")
        .arg("--sign")
        .arg(signing_identity);
    let entitlements = entitlement_path(manifest);
    if entitlements.is_file() {
        command.arg("--entitlements").arg(entitlements);
    }
    command.arg(binary_path);
    run_command(command, "codesign helper")?;
    let verified = Command::new("codesign")
        .arg("--verify")
        .arg("--strict")
        .arg(binary_path)
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if signing_identity == "-" {
        diagnostics.push("helper binary was ad-hoc signed for local verification; set VANTA_MACOS_DEVELOPER_ID for production signing".to_string());
    }
    Ok(verified)
}

fn verify_platform_signature(
    artifact_path: &Path,
    platform: &str,
    artifact_kind: &str,
    diagnostics: &mut Vec<String>,
) -> bool {
    if !artifact_path.is_file() {
        return false;
    }
    match platform {
        "macos" => {
            let verified = Command::new("codesign")
                .arg("--verify")
                .arg("--strict")
                .arg(artifact_path)
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false);
            if !verified {
                diagnostics.push(format!(
                    "macOS {artifact_kind} signature verification failed"
                ));
            }
            verified
        }
        "windows" => verify_windows_artifact_signature(artifact_path, artifact_kind, diagnostics),
        _ => false,
    }
}

fn verify_installer_signature(
    installer_path: &Path,
    platform: &str,
    artifact_kind: &str,
    diagnostics: &mut Vec<String>,
) -> bool {
    if !installer_path.is_file() {
        return false;
    }
    match platform {
        "macos" => {
            let verified = Command::new("pkgutil")
                .arg("--check-signature")
                .arg(installer_path)
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false);
            if !verified {
                diagnostics.push(format!(
                    "macOS {artifact_kind} package signature verification failed"
                ));
            }
            verified
        }
        "windows" => verify_windows_artifact_signature(installer_path, artifact_kind, diagnostics),
        _ => false,
    }
}

fn verify_windows_artifact_signature(
    artifact_path: &Path,
    artifact_kind: &str,
    diagnostics: &mut Vec<String>,
) -> bool {
    let signtool = env::var("VANTA_WINDOWS_SIGNTOOL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "signtool".to_string());
    let verified = Command::new(&signtool)
        .arg("verify")
        .arg("/pa")
        .arg(artifact_path)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    if !verified {
        diagnostics.push(format!(
            "Windows {artifact_kind} Authenticode verification failed"
        ));
    }
    verified
}

fn verify_macos_notarization_staple(installer_path: &Path, diagnostics: &mut Vec<String>) -> bool {
    if !installer_path.is_file() {
        return false;
    }
    let verified = Command::new("xcrun")
        .arg("stapler")
        .arg("validate")
        .arg(installer_path)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    if !verified {
        diagnostics.push("macOS notarization staple verification failed".to_string());
    }
    verified
}

fn sign_helper_binary(
    binary_path: &Path,
    manifest: &NativePackageManifest,
    signing_identity: &str,
    diagnostics: &mut Vec<String>,
) -> Result<bool, NativePackageBuildError> {
    match manifest.platform.as_str() {
        "macos" => sign_macos_binary(binary_path, manifest, signing_identity, diagnostics),
        "windows" => sign_windows_binary(binary_path, manifest, signing_identity, diagnostics),
        platform => Err(NativePackageBuildError::Command(format!(
            "unsupported native helper signing platform {platform}"
        ))),
    }
}

fn sign_windows_binary(
    binary_path: &Path,
    _manifest: &NativePackageManifest,
    signing_identity: &str,
    diagnostics: &mut Vec<String>,
) -> Result<bool, NativePackageBuildError> {
    if signing_identity == "-" {
        diagnostics.push("Windows helper was staged unsigned; set VANTA_WINDOWS_SIGNING_CERT and run on a Windows signing host with signtool for production Authenticode signing".to_string());
        return Ok(false);
    }
    sign_windows_artifact(binary_path, signing_identity, "helper", diagnostics)
}

fn sign_windows_artifact(
    artifact_path: &Path,
    signing_identity: &str,
    artifact_kind: &str,
    diagnostics: &mut Vec<String>,
) -> Result<bool, NativePackageBuildError> {
    let signtool = env::var("VANTA_WINDOWS_SIGNTOOL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "signtool".to_string());
    let mut command = Command::new(&signtool);
    command
        .arg("sign")
        .arg("/fd")
        .arg("SHA256")
        .arg("/a")
        .arg("/f")
        .arg(signing_identity);
    if let Some(password) = env::var("VANTA_WINDOWS_SIGNING_CERT_PASSWORD")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        command.arg("/p").arg(password);
    }
    if let Some(timestamp_url) = env::var("VANTA_WINDOWS_TIMESTAMP_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        command
            .arg("/tr")
            .arg(timestamp_url)
            .arg("/td")
            .arg("SHA256");
    }
    command.arg(artifact_path);
    let status = command.status();
    match status {
        Ok(status) if status.success() => {
            let verified = Command::new(&signtool)
                .arg("verify")
                .arg("/pa")
                .arg(artifact_path)
                .status()
                .map(|status| status.success())
                .unwrap_or(false);
            if !verified {
                diagnostics.push(format!(
                    "signtool signing completed for Windows {artifact_kind} but Authenticode verification failed"
                ));
            }
            Ok(verified)
        }
        Ok(status) => {
            diagnostics.push(format!(
                "Windows {artifact_kind} Authenticode signing failed with status {status}"
            ));
            Ok(false)
        }
        Err(error) => {
            diagnostics.push(format!(
                "Windows {artifact_kind} Authenticode signing skipped because signtool is unavailable: {error}"
            ));
            Ok(false)
        }
    }
}

fn build_installer(
    binary_path: &Path,
    install_path: &Path,
    manifest: &NativePackageManifest,
    signing_identity: &str,
    diagnostics: &mut Vec<String>,
) -> Result<(bool, bool), NativePackageBuildError> {
    match manifest.platform.as_str() {
        "macos" => {
            let created = build_macos_installer(
                binary_path,
                install_path,
                manifest,
                signing_identity,
                diagnostics,
            )?;
            Ok((created, created && signing_identity != "-"))
        }
        "windows" => build_windows_msix(
            binary_path,
            install_path,
            manifest,
            signing_identity,
            diagnostics,
        ),
        platform => Err(NativePackageBuildError::Command(format!(
            "unsupported native installer platform {platform}"
        ))),
    }
}

fn build_macos_installer(
    binary_path: &Path,
    install_path: &Path,
    manifest: &NativePackageManifest,
    signing_identity: &str,
    diagnostics: &mut Vec<String>,
) -> Result<bool, NativePackageBuildError> {
    let staging = PathBuf::from(expand_root(&format!(
        "$VANTA_OBS_ROOT/native/{}/{}/stage/usr/local/lib/vanta-obs/helpers",
        manifest.helper_kind, manifest.platform
    )));
    if let Some(root) = staging
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
    {
        if root.exists() {
            fs::remove_dir_all(root)?;
        }
    }
    fs::create_dir_all(&staging)?;
    fs::copy(
        binary_path,
        staging.join(binary_path.file_name().unwrap_or_default()),
    )?;
    fs::create_dir_all(parent(install_path)?)?;
    let mut command = Command::new("pkgbuild");
    command
        .arg("--root")
        .arg(staging.ancestors().nth(5).unwrap_or(&staging))
        .arg("--identifier")
        .arg(format!(
            "com.vanta.obs.native.{}.helper",
            manifest.helper_kind
        ))
        .arg("--version")
        .arg(env!("CARGO_PKG_VERSION"))
        .arg("--install-location")
        .arg("/")
        .arg(install_path);
    if signing_identity != "-" {
        command.arg("--sign").arg(signing_identity);
    } else {
        diagnostics.push(
            "installer package was built unsigned; pkg signing requires VANTA_MACOS_DEVELOPER_ID"
                .to_string(),
        );
    }
    run_command(command, "pkgbuild helper installer")?;
    Ok(install_path.is_file())
}

fn build_windows_msix(
    binary_path: &Path,
    install_path: &Path,
    manifest: &NativePackageManifest,
    signing_identity: &str,
    diagnostics: &mut Vec<String>,
) -> Result<(bool, bool), NativePackageBuildError> {
    let staging = PathBuf::from(expand_root(&format!(
        "$VANTA_OBS_ROOT/native/{}/{}/stage/VantaOBS/helpers",
        manifest.helper_kind, manifest.platform
    )));
    let stage_root = PathBuf::from(expand_root(&format!(
        "$VANTA_OBS_ROOT/native/{}/{}/stage",
        manifest.helper_kind, manifest.platform
    )));
    if stage_root.exists() {
        fs::remove_dir_all(&stage_root)?;
    }
    fs::create_dir_all(&staging)?;
    let staged_binary = staging.join(binary_path.file_name().unwrap_or_default());
    fs::copy(binary_path, &staged_binary)?;
    fs::create_dir_all(parent(install_path)?)?;
    let staged_sha256 = sha256_file(&staged_binary)?;
    let package_payload = json!({
        "format": "vanta-msix-staging-manifest",
        "package_id": manifest.package_id,
        "helper_kind": manifest.helper_kind,
        "platform": manifest.platform,
        "display_name": manifest.display_name,
        "binary_name": staged_binary.file_name().and_then(|value| value.to_str()).unwrap_or("vanta-native-helper.exe"),
        "binary_sha256": staged_sha256,
        "transports": manifest.transports,
        "permissions": manifest.permissions,
        "signing": {
            "required": manifest.signing.required,
            "identity_env": manifest.signing.identity_env,
            "authenticode_cert_configured": signing_identity != "-",
            "signed": false
        }
    });
    fs::write(install_path, serde_json::to_vec_pretty(&package_payload)?)?;
    let installer_signed = if signing_identity == "-" {
        diagnostics.push("Windows MSIX staging artifact was created unsigned; set VANTA_WINDOWS_SIGNING_CERT and run on a Windows signing host with signtool for production Authenticode signing".to_string());
        false
    } else {
        sign_windows_artifact(
            install_path,
            signing_identity,
            "MSIX installer",
            diagnostics,
        )?
    };
    Ok((install_path.is_file(), installer_signed))
}

fn notarize_installer_if_required(
    install_path: &Path,
    manifest: &NativePackageManifest,
    installer_signed: bool,
    diagnostics: &mut Vec<String>,
) -> Result<bool, NativePackageBuildError> {
    if !manifest.signing.notarization_required {
        return Ok(false);
    }
    if manifest.platform != "macos" {
        diagnostics.push("notarization is only supported for macOS helper installers".to_string());
        return Ok(false);
    }
    if !installer_signed {
        diagnostics.push(
            "macOS notarization skipped because the installer is not production-signed".to_string(),
        );
        return Ok(false);
    }
    let Some(command) = notarytool_submit_command(install_path, diagnostics) else {
        diagnostics.push("macOS notarization skipped; configure VANTA_MACOS_NOTARY_PROFILE or VANTA_MACOS_NOTARY_APPLE_ID, VANTA_MACOS_NOTARY_PASSWORD, and VANTA_MACOS_NOTARY_TEAM_ID".to_string());
        return Ok(false);
    };
    if let Err(error) = run_command(command, "xcrun notarytool submit") {
        diagnostics.push(format!("macOS notarization failed: {error}"));
        return Ok(false);
    }
    let mut staple = Command::new("xcrun");
    staple.arg("stapler").arg("staple").arg(install_path);
    if let Err(error) = run_command(staple, "xcrun stapler staple") {
        diagnostics.push(format!("macOS notarization stapling failed: {error}"));
        return Ok(false);
    }
    let mut validate = Command::new("xcrun");
    validate.arg("stapler").arg("validate").arg(install_path);
    if let Err(error) = run_command(validate, "xcrun stapler validate") {
        diagnostics.push(format!(
            "macOS notarization staple validation failed: {error}"
        ));
        return Ok(false);
    }
    Ok(true)
}

fn notarytool_submit_command(
    install_path: &Path,
    diagnostics: &mut Vec<String>,
) -> Option<Command> {
    let mut command = Command::new("xcrun");
    command
        .arg("notarytool")
        .arg("submit")
        .arg(install_path)
        .arg("--wait");
    if let Some(profile) = env::var("VANTA_MACOS_NOTARY_PROFILE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        command.arg("--keychain-profile").arg(profile);
        return Some(command);
    }
    let apple_id = env::var("VANTA_MACOS_NOTARY_APPLE_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let password = env::var("VANTA_MACOS_NOTARY_PASSWORD")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let team_id = env::var("VANTA_MACOS_NOTARY_TEAM_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    match (apple_id, password, team_id) {
        (Some(apple_id), Some(password), Some(team_id)) => {
            command
                .arg("--apple-id")
                .arg(apple_id)
                .arg("--password")
                .arg(password)
                .arg("--team-id")
                .arg(team_id);
            Some(command)
        }
        _ => {
            diagnostics.push("notarytool credentials are incomplete".to_string());
            None
        }
    }
}

fn build_manifest_path(manifest: &NativePackageManifest) -> String {
    expand_root(&format!(
        "$VANTA_OBS_ROOT/native/{}/{}/package/build-manifest.json",
        manifest.helper_kind, manifest.platform
    ))
}

fn read_build_manifest(path: &str) -> Option<Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
}

fn entitlement_path(manifest: &NativePackageManifest) -> PathBuf {
    PathBuf::from(expand_root(&format!(
        "$VANTA_OBS_ROOT/native/{}/{}/{}",
        manifest.helper_kind, manifest.platform, manifest.signing.entitlement_profile
    )))
}

fn sha256_file(path: &Path) -> Result<String, NativePackageBuildError> {
    let bytes = fs::read(path)?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{digest:x}"))
}

fn parent(path: &Path) -> Result<&Path, NativePackageBuildError> {
    path.parent().ok_or_else(|| {
        NativePackageBuildError::Command(format!("path {} has no parent", path.display()))
    })
}

fn run_command(mut command: Command, label: &str) -> Result<(), NativePackageBuildError> {
    let output = command.output().map_err(|error| {
        NativePackageBuildError::Command(format!("{label} could not start: {error}"))
    })?;
    if output.status.success() {
        return Ok(());
    }
    Err(NativePackageBuildError::Command(format!(
        "{label} failed with status {}: {}{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )))
}

#[derive(Debug, thiserror::Error)]
pub enum NativePackageBuildError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("native package build failed: {0}")]
    Command(String),
}

fn current_platform() -> &'static str {
    match env::consts::OS {
        "macos" => "macos",
        "windows" => "windows",
        "linux" => "linux",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::{current_platform_package, package_states};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn validates_platform_specific_native_helper_manifests() {
        let packages = package_states();
        assert_eq!(packages.len(), 8);
        for kind in ["capture", "encode", "replay", "audio"] {
            assert!(packages.iter().any(|package| {
                package.helper_kind == kind
                    && package.platform == "macos"
                    && package.signing_required
                    && package.notarization_required
            }));
            assert!(packages.iter().any(|package| {
                package.helper_kind == kind
                    && package.platform == "windows"
                    && package.signing_required
            }));
        }

        let capture = current_platform_package("capture").unwrap();
        assert_eq!(capture.helper_kind, "capture");
        assert!(
            capture
                .transports
                .iter()
                .any(|transport| transport == "stdio")
        );
        assert!(!capture.permissions.is_empty());
        assert!(!capture.build_manifest_path.is_empty());
        assert_ne!(capture.status, "ready");

        let audio = packages
            .iter()
            .find(|package| package.helper_kind == "audio" && package.platform == "macos")
            .unwrap();
        assert!(audio.system_audio_validation_required);
        assert!(
            audio
                .permissions
                .iter()
                .any(|permission| permission == "screen-recording")
        );
        assert!(
            audio
                .permissions
                .iter()
                .any(|permission| permission == "system-audio")
        );
        assert!(
            audio
                .permissions
                .iter()
                .any(|permission| permission == "application-audio")
        );
        assert_ne!(audio.status, "ready");
    }

    #[test]
    fn macos_notarization_requires_signed_installer_and_credentials() {
        let mut diagnostics = Vec::new();
        let manifest = package_states()
            .into_iter()
            .find(|package| package.helper_kind == "capture" && package.platform == "macos")
            .unwrap();
        assert!(manifest.notarization_required);
        assert!(
            manifest
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains("notarization"))
        );
        let result = super::notarize_installer_if_required(
            std::path::Path::new(&manifest.install_path),
            &super::manifests()
                .into_iter()
                .find(|candidate| {
                    candidate.helper_kind == "capture" && candidate.platform == "macos"
                })
                .unwrap(),
            false,
            &mut diagnostics,
        )
        .unwrap();
        assert!(!result);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains("not production-signed"))
        );
    }

    #[test]
    fn windows_msix_stays_unsigned_without_authenticode_certificate() {
        let root = std::env::temp_dir().join(format!(
            "vanta-obs-msix-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let binary_path = root.join("vanta-native-helper.exe");
        let install_path = root.join("vanta-native-helper.msix");
        fs::write(&binary_path, b"test helper").unwrap();

        let manifest = super::manifests()
            .into_iter()
            .find(|candidate| candidate.helper_kind == "capture" && candidate.platform == "windows")
            .unwrap();
        let mut diagnostics = Vec::new();
        let (created, signed) = super::build_windows_msix(
            &binary_path,
            &install_path,
            &manifest,
            "-",
            &mut diagnostics,
        )
        .unwrap();

        assert!(created);
        assert!(!signed);
        assert!(install_path.is_file());
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains("created unsigned"))
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn distribution_verifier_blocks_non_production_signed_artifacts() {
        let reports = super::verify_distribution_packages().unwrap();
        assert_eq!(reports.len(), 8);
        assert!(reports.iter().any(|report| {
            report.platform == "macos"
                && report.helper_kind == "capture"
                && report.status == "blocked"
                && !report.helper_production_signature_verified
                && !report.installer_production_signature_verified
        }));
        assert!(reports.iter().any(|report| {
            report.helper_kind == "audio"
                && report.platform == "macos"
                && report.system_audio_validation_required
                && !report.system_audio_validation_verified
        }));
    }
}
