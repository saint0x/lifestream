use serde_json::{Value, json};

#[derive(Debug, Clone, Copy)]
pub struct SourceContract {
    pub kind: &'static str,
    pub renderer: &'static str,
    pub permission_kind: &'static str,
    pub local_sync: &'static str,
    pub obs_kind: &'static str,
    pub required_settings: &'static [&'static str],
    pub requires_device: bool,
    pub requires_browser_url: bool,
    pub requires_media_asset: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct SourceFilterContract {
    pub kind: &'static str,
    pub renderer_stage: &'static str,
    pub obs_kind: &'static str,
    pub required_settings: &'static [&'static str],
}

pub const SOURCE_FILTER_CONTRACTS: &[SourceFilterContract] = &[
    filter_contract("color_correction", "video", "color_filter", &[]),
    filter_contract("chroma_key", "video", "chroma_key_filter", &["key_color"]),
    filter_contract("crop_pad", "layout", "crop_filter", &[]),
    filter_contract("scale_aspect", "layout", "scale_filter", &["mode"]),
    filter_contract("sharpness", "video", "sharpness_filter_v2", &["amount"]),
];

pub const SOURCE_CONTRACTS: &[SourceContract] = &[
    contract(
        "camera",
        "device_video",
        "camera",
        "media_stream",
        "av_capture_input",
        &[],
        true,
        false,
        false,
    ),
    contract(
        "microphone",
        "device_audio",
        "microphone",
        "media_stream",
        "coreaudio_input_capture",
        &[],
        true,
        false,
        false,
    ),
    contract(
        "desktop_audio",
        "system_audio",
        "desktop_audio",
        "loopback_audio",
        "coreaudio_output_capture",
        &[],
        true,
        false,
        false,
    ),
    contract(
        "system_audio",
        "system_audio",
        "system_audio",
        "screencapturekit_audio",
        "coreaudio_output_capture",
        &[],
        true,
        false,
        false,
    ),
    contract(
        "application_audio",
        "application_audio",
        "application_audio",
        "screencapturekit_application_audio",
        "coreaudio_application_capture",
        &[],
        true,
        false,
        false,
    ),
    contract(
        "screen_capture",
        "screen_video",
        "display",
        "display_stream",
        "display_capture",
        &[],
        true,
        false,
        false,
    ),
    contract(
        "display_capture",
        "screen_video",
        "display",
        "display_stream",
        "display_capture",
        &[],
        true,
        false,
        false,
    ),
    contract(
        "window_capture",
        "window_video",
        "window",
        "display_stream",
        "window_capture",
        &[],
        true,
        false,
        false,
    ),
    contract(
        "browser_capture",
        "browser_frame",
        "network",
        "browser_frame",
        "browser_source",
        &["width", "height"],
        false,
        true,
        false,
    ),
    contract(
        "media_file",
        "media_frame",
        "asset",
        "asset_file",
        "ffmpeg_source",
        &[],
        false,
        false,
        true,
    ),
    contract(
        "image",
        "image_frame",
        "asset",
        "asset_file",
        "image_source",
        &[],
        false,
        false,
        true,
    ),
    contract(
        "text",
        "text_overlay",
        "none",
        "inline",
        "text_ft2_source_v2",
        &["text"],
        false,
        false,
        false,
    ),
    contract(
        "lower_third",
        "lower_third",
        "none",
        "inline",
        "browser_source",
        &["headline"],
        false,
        false,
        false,
    ),
    contract(
        "branded_bumper",
        "branded_bumper",
        "asset",
        "campaign_asset",
        "browser_source",
        &["headline"],
        false,
        false,
        true,
    ),
    contract(
        "pinned_cta",
        "pinned_cta",
        "none",
        "runtime_feed",
        "browser_source",
        &["cta_text"],
        false,
        false,
        false,
    ),
    contract(
        "qr_code",
        "qr_code",
        "none",
        "inline",
        "browser_source",
        &["target_url"],
        false,
        false,
        false,
    ),
    contract(
        "promo_code",
        "promo_code",
        "none",
        "inline",
        "browser_source",
        &["promo_code"],
        false,
        false,
        false,
    ),
    contract(
        "sponsor_card",
        "sponsor_card",
        "campaign",
        "campaign_asset",
        "browser_source",
        &["promo_code"],
        false,
        false,
        true,
    ),
    contract(
        "countdown_timer",
        "countdown_timer",
        "none",
        "runtime_clock",
        "browser_source",
        &["seconds"],
        false,
        false,
        false,
    ),
    contract(
        "chat_overlay",
        "chat_overlay",
        "runtime",
        "runtime_feed",
        "browser_source",
        &[],
        false,
        false,
        false,
    ),
    contract(
        "alert_overlay",
        "alert_overlay",
        "runtime",
        "runtime_feed",
        "browser_source",
        &[],
        false,
        false,
        false,
    ),
    contract(
        "guest_feed",
        "guest_tile",
        "guest",
        "remote_media",
        "browser_source",
        &[],
        true,
        false,
        false,
    ),
    contract(
        "remote_contribution",
        "remote_tile",
        "guest",
        "remote_media",
        "browser_source",
        &[],
        true,
        false,
        false,
    ),
    contract(
        "vanta_video_asset",
        "vanta_video",
        "asset",
        "asset_file",
        "ffmpeg_source",
        &[],
        false,
        false,
        true,
    ),
    contract(
        "vanta_clip",
        "vanta_clip",
        "asset",
        "asset_file",
        "ffmpeg_source",
        &[],
        false,
        false,
        true,
    ),
    contract(
        "color_matte",
        "color_matte",
        "none",
        "inline",
        "color_source_v3",
        &["color"],
        false,
        false,
        false,
    ),
    contract(
        "safe_area_guide",
        "safe_area",
        "none",
        "inline",
        "browser_source",
        &[],
        false,
        false,
        false,
    ),
    contract(
        "scene_group",
        "scene_group",
        "scene",
        "scene_graph",
        "scene",
        &["scene_id"],
        false,
        false,
        false,
    ),
];

const fn contract(
    kind: &'static str,
    renderer: &'static str,
    permission_kind: &'static str,
    local_sync: &'static str,
    obs_kind: &'static str,
    required_settings: &'static [&'static str],
    requires_device: bool,
    requires_browser_url: bool,
    requires_media_asset: bool,
) -> SourceContract {
    SourceContract {
        kind,
        renderer,
        permission_kind,
        local_sync,
        obs_kind,
        required_settings,
        requires_device,
        requires_browser_url,
        requires_media_asset,
    }
}

const fn filter_contract(
    kind: &'static str,
    renderer_stage: &'static str,
    obs_kind: &'static str,
    required_settings: &'static [&'static str],
) -> SourceFilterContract {
    SourceFilterContract {
        kind,
        renderer_stage,
        obs_kind,
        required_settings,
    }
}

pub fn source_kinds() -> Vec<&'static str> {
    SOURCE_CONTRACTS
        .iter()
        .map(|contract| contract.kind)
        .collect()
}

pub fn contract_for(kind: &str) -> Option<SourceContract> {
    SOURCE_CONTRACTS
        .iter()
        .copied()
        .find(|contract| contract.kind == kind)
}

pub fn filter_contract_for(kind: &str) -> Option<SourceFilterContract> {
    SOURCE_FILTER_CONTRACTS
        .iter()
        .copied()
        .find(|contract| contract.kind == kind)
}

pub fn source_filter_summary(kind: &str) -> Option<Value> {
    filter_contract_for(kind).map(|contract| {
        json!({
            "kind": contract.kind,
            "renderer_stage": contract.renderer_stage,
            "obs_kind": contract.obs_kind,
            "required_settings": contract.required_settings
        })
    })
}

pub fn validate_source_filter(kind: &str, settings: &Value) -> SourceValidation {
    let Some(contract) = filter_contract_for(kind) else {
        return SourceValidation::blocked(vec![format!("{kind}_unsupported")]);
    };
    let errors = contract
        .required_settings
        .iter()
        .filter(|key| missing_setting(settings, key))
        .map(|key| format!("{key}_required"))
        .collect::<Vec<_>>();
    SourceValidation {
        status: if errors.is_empty() {
            "ready"
        } else {
            "blocked"
        },
        errors,
        warnings: Vec::new(),
    }
}

pub fn validate_source(
    kind: &str,
    device_id: Option<&str>,
    browser_url: Option<&str>,
    media_asset_id: Option<&str>,
    settings: &Value,
) -> SourceValidation {
    let Some(contract) = contract_for(kind) else {
        return SourceValidation::blocked(vec!["unsupported_source_kind".to_string()]);
    };
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if contract.requires_device && device_id.map(str::trim).unwrap_or_default().is_empty() {
        errors.push("device_id_required".to_string());
    }
    if contract.requires_browser_url {
        let url = browser_url.map(str::trim).unwrap_or_default();
        if !url.starts_with("https://") && !url.starts_with("http://") {
            errors.push("http_browser_url_required".to_string());
        }
    }
    if contract.requires_media_asset && media_asset_id.map(str::trim).unwrap_or_default().is_empty()
    {
        errors.push("media_asset_id_required".to_string());
    }
    for key in contract.required_settings {
        if missing_setting(settings, key) {
            errors.push(format!("{key}_required"));
        }
    }
    if matches!(
        contract.kind,
        "sponsor_card" | "pinned_cta" | "qr_code" | "promo_code"
    ) && missing_setting(settings, "tracking")
    {
        warnings.push("tracking_url_recommended".to_string());
    }

    SourceValidation {
        status: if errors.is_empty() {
            "ready"
        } else {
            "blocked"
        },
        errors,
        warnings,
    }
}

pub fn enriched_settings(
    kind: &str,
    device_id: Option<&str>,
    browser_url: Option<&str>,
    media_asset_id: Option<&str>,
    permission_state: &str,
    health_state: &str,
    settings: Value,
) -> Value {
    let validation = validate_source(kind, device_id, browser_url, media_asset_id, &settings);
    let contract = contract_for(kind);
    let mut root = settings.as_object().cloned().unwrap_or_default();
    root.insert(
        "vanta_source".to_string(),
        json!({
            "contract": contract.map(contract_json).unwrap_or_else(|| json!({ "kind": kind })),
            "validation": validation.to_json(),
            "permission": permission_json(contract, permission_state),
            "local_sync": sync_json(contract, permission_state, health_state)
        }),
    );
    Value::Object(root)
}

pub fn source_summary(
    kind: &str,
    device_id: Option<&str>,
    browser_url: Option<&str>,
    media_asset_id: Option<&str>,
    permission_state: &str,
    health_state: &str,
    settings: Value,
) -> Value {
    enriched_settings(
        kind,
        device_id,
        browser_url,
        media_asset_id,
        permission_state,
        health_state,
        settings,
    )
    .get("vanta_source")
    .cloned()
    .unwrap_or_else(|| json!({}))
}

fn contract_json(contract: SourceContract) -> Value {
    json!({
        "kind": contract.kind,
        "renderer": contract.renderer,
        "permission_kind": contract.permission_kind,
        "local_sync": contract.local_sync,
        "obs_kind": contract.obs_kind,
        "required_settings": contract.required_settings,
        "requires_device": contract.requires_device,
        "requires_browser_url": contract.requires_browser_url,
        "requires_media_asset": contract.requires_media_asset
    })
}

fn permission_json(contract: Option<SourceContract>, permission_state: &str) -> Value {
    json!({
        "state": permission_state,
        "kind": contract.map(|contract| contract.permission_kind).unwrap_or("unknown"),
        "required": contract
            .map(|contract| contract.permission_kind != "none")
            .unwrap_or(true)
    })
}

fn sync_json(
    contract: Option<SourceContract>,
    permission_state: &str,
    health_state: &str,
) -> Value {
    let transport = contract
        .map(|contract| contract.local_sync)
        .unwrap_or("unsupported");
    let blocked = permission_state == "denied"
        || permission_state == "unsupported"
        || health_state == "blocked";
    json!({
        "transport": transport,
        "status": if blocked { "blocked" } else if permission_state == "pending" { "pending" } else { "ready" },
        "permission_state": permission_state,
        "health_state": health_state
    })
}

fn missing_setting(settings: &Value, key: &str) -> bool {
    match settings.get(key) {
        None | Some(Value::Null) => true,
        Some(Value::String(value)) => value.trim().is_empty(),
        _ => false,
    }
}

#[derive(Debug, Clone)]
pub struct SourceValidation {
    pub status: &'static str,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl SourceValidation {
    fn blocked(errors: Vec<String>) -> Self {
        Self {
            status: "blocked",
            errors,
            warnings: Vec::new(),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "status": self.status,
            "errors": self.errors,
            "warnings": self.warnings
        })
    }
}
