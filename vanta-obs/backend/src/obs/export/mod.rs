use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use super::adapter::{ObsExportTarget, vanta_kind_to_obs_export};

#[derive(Debug, Clone, Deserialize)]
pub struct ObsExportInput {
    pub collection_id: String,
    pub label: String,
    pub include_setup_instructions: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsExportPackage {
    pub label: String,
    pub collection_id: String,
    pub collection_name: String,
    pub scene_collection_json: Value,
    pub asset_manifest: ObsAssetManifest,
    pub warnings: Vec<ObsExportWarning>,
    pub setup_instructions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsAssetManifest {
    pub bundle_id: String,
    pub assets: Vec<ObsAssetManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsAssetManifestEntry {
    pub source_name: String,
    pub source_kind: String,
    pub obs_kind: String,
    pub bundle_path: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsExportWarning {
    pub code: String,
    pub subject: String,
    pub detail: String,
}

#[derive(Debug, Error)]
pub enum ObsExportError {
    #[error("invalid Vanta collection bundle: {0}")]
    Invalid(String),
}

pub fn build_obs_export_package(
    input: ObsExportInput,
    bundle: Value,
) -> Result<ObsExportPackage, ObsExportError> {
    let collection = bundle
        .get("collection")
        .ok_or_else(|| ObsExportError::Invalid("missing collection".to_string()))?;
    let scenes = bundle
        .get("scenes")
        .and_then(Value::as_array)
        .ok_or_else(|| ObsExportError::Invalid("missing scenes".to_string()))?;
    let sources = bundle
        .get("sources")
        .and_then(Value::as_array)
        .ok_or_else(|| ObsExportError::Invalid("missing sources".to_string()))?;
    let instances = bundle
        .get("instances")
        .and_then(Value::as_array)
        .ok_or_else(|| ObsExportError::Invalid("missing instances".to_string()))?;

    let collection_name = text(collection, "name");
    let mut warnings = Vec::new();
    let mut manifest_assets = Vec::new();
    let mut obs_sources = Vec::new();

    for scene in scenes {
        obs_sources.push(json!({
            "name": text(scene, "name"),
            "id": "scene",
            "settings": {
                "items": scene_items(scene, sources, instances)
            }
        }));
    }

    for source in sources {
        if let Some(exported) = export_source(
            source,
            scenes,
            sources,
            instances,
            &mut warnings,
            &mut manifest_assets,
        ) {
            obs_sources.push(exported);
        }
    }

    let transition = transition_kind(
        scenes
            .first()
            .map(|scene| text(scene, "transition_kind"))
            .unwrap_or_else(|| "fade".to_string())
            .as_str(),
    );
    let scene_collection_json = json!({
        "name": collection_name,
        "current_scene_collection": collection_name,
        "transition": transition,
        "transition_duration": scenes.first().map(|scene| int(scene, "transition_duration_ms")).unwrap_or(300),
        "video": {
            "base_width": int(collection, "canvas_width"),
            "base_height": int(collection, "canvas_height"),
            "fps_num": int(collection, "frame_rate"),
            "fps_den": 1
        },
        "scene_order": scenes.iter().map(|scene| json!({"name": text(scene, "name")})).collect::<Vec<_>>(),
        "sources": obs_sources,
        "vanta_export": {
            "collection_id": input.collection_id,
            "generated_by": "vanta-obs",
            "warnings": warnings
        }
    });
    let setup_instructions = if input.include_setup_instructions.unwrap_or(true) {
        vec![
            "Import the generated OBS scene collection JSON from OBS Studio.".to_string(),
            "Place bundled assets at the paths listed in asset_manifest before opening the collection.".to_string(),
            "Review Vanta export warnings for runtime features OBS cannot reproduce natively.".to_string(),
        ]
    } else {
        Vec::new()
    };

    Ok(ObsExportPackage {
        label: input.label,
        collection_id: input.collection_id,
        collection_name,
        scene_collection_json,
        asset_manifest: ObsAssetManifest {
            bundle_id: format!("obs-export-{}", short_label(collection)),
            assets: manifest_assets,
        },
        warnings,
        setup_instructions,
    })
}

fn scene_items(scene: &Value, sources: &[Value], instances: &[Value]) -> Vec<Value> {
    instances
        .iter()
        .filter(|instance| text(instance, "scene_id") == text(scene, "id"))
        .filter_map(|instance| {
            let source = sources
                .iter()
                .find(|source| text(source, "id") == text(instance, "source_id"))?;
            Some(json!({
                "name": text(source, "display_name"),
                "visible": int(instance, "visible") != 0,
                "locked": int(instance, "locked") != 0,
                "pos": {
                    "x": num(instance, "x"),
                    "y": num(instance, "y")
                },
                "bounds": {
                    "x": num(instance, "width"),
                    "y": num(instance, "height")
                },
                "crop": instance.get("crop_json").cloned().unwrap_or_else(|| json!({"top":0,"right":0,"bottom":0,"left":0})),
                "scale": {"x": 1.0, "y": 1.0},
                "rot": instance
                    .get("transform_json")
                    .and_then(|transform| transform.get("rotation"))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0),
                "opacity": num(instance, "opacity")
            }))
        })
        .collect()
}

fn export_source(
    source: &Value,
    scenes: &[Value],
    sources: &[Value],
    instances: &[Value],
    warnings: &mut Vec<ObsExportWarning>,
    assets: &mut Vec<ObsAssetManifestEntry>,
) -> Option<Value> {
    let name = text(source, "display_name");
    let kind = text(source, "source_kind");
    let stored_settings = source
        .get("default_settings_json")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let settings = stored_settings
        .get("settings")
        .cloned()
        .unwrap_or_else(|| stored_settings.clone());
    let target = match vanta_kind_to_obs_export(&kind) {
        Some(ObsExportTarget::Source(target)) => target,
        Some(ObsExportTarget::Omit(notice)) => {
            warnings.push(warning(notice.code, &name, notice.detail));
            return None;
        }
        None => {
            warnings.push(warning(
                "unsupported_vanta_source_kind",
                &name,
                &format!("{kind} cannot be represented in OBS scene collection JSON"),
            ));
            return None;
        }
    };
    if let Some(notice) = target.notice {
        warnings.push(warning(notice.code, &name, notice.detail));
    }
    if let Some(folder) = target.asset_folder {
        assets.push(asset_entry(&name, &kind, target.obs_kind, folder));
    }
    let obs_settings = obs_settings(
        target.obs_kind,
        &kind,
        &name,
        source,
        &settings,
        scenes,
        sources,
        instances,
    );

    Some(json!({
        "name": name,
        "id": target.obs_kind,
        "settings": obs_settings,
        "vanta_source": {
            "id": text(source, "id"),
            "kind": kind,
            "settings": settings
        }
    }))
}

fn obs_settings(
    obs_kind: &str,
    kind: &str,
    name: &str,
    source: &Value,
    settings: &Value,
    scenes: &[Value],
    sources: &[Value],
    instances: &[Value],
) -> Value {
    match obs_kind {
        "av_capture_input" | "coreaudio_input_capture" => {
            json!({"device": text(source, "device_id")})
        }
        "display_capture" => json!({"display": text(source, "device_id")}),
        "window_capture" => json!({"window": text(source, "device_id")}),
        "browser_source" if kind == "browser_capture" => browser_settings(source, settings),
        "browser_source" => vanta_overlay_settings(kind, name, settings),
        "ffmpeg_source" if kind == "media_file" => {
            json!({"local_file": bundle_path(name, "media")})
        }
        "ffmpeg_source" => json!({"local_file": bundle_path(name, "vanta-media")}),
        "image_source" => json!({"file": bundle_path(name, "images")}),
        "text_ft2_source" => {
            json!({"text": settings.get("text").and_then(Value::as_str).unwrap_or(name)})
        }
        "color_source" => {
            json!({"color": settings.get("color").cloned().unwrap_or_else(|| json!(4278190080u64))})
        }
        "group" => {
            let scene_id = settings.get("scene_id").and_then(Value::as_str);
            let items = scene_id
                .and_then(|scene_id| scenes.iter().find(|scene| text(scene, "id") == scene_id))
                .map(|scene| scene_items(scene, sources, instances))
                .unwrap_or_default();
            json!({
                "items": items,
                "vanta_nested_scene_id": scene_id,
                "vanta_group_name": name
            })
        }
        _ => json!({}),
    }
}

fn browser_settings(source: &Value, settings: &Value) -> Value {
    json!({
        "url": text(source, "browser_url"),
        "width": settings.get("width").and_then(Value::as_i64).unwrap_or(1280),
        "height": settings.get("height").and_then(Value::as_i64).unwrap_or(720)
    })
}

fn vanta_overlay_settings(kind: &str, name: &str, settings: &Value) -> Value {
    json!({
        "url": format!("https://streamvanta.tv/obs-overlay/{}/{}", kind, slug(name)),
        "width": settings.get("width").and_then(Value::as_i64).unwrap_or(1280),
        "height": settings.get("height").and_then(Value::as_i64).unwrap_or(720)
    })
}

fn asset_entry(
    name: &str,
    source_kind: &str,
    obs_kind: &str,
    folder: &str,
) -> ObsAssetManifestEntry {
    ObsAssetManifestEntry {
        source_name: name.to_string(),
        source_kind: source_kind.to_string(),
        obs_kind: obs_kind.to_string(),
        bundle_path: bundle_path(name, folder),
        required: true,
    }
}

fn bundle_path(name: &str, folder: &str) -> String {
    format!("assets/{}/{}.bin", folder, slug(name))
}

fn transition_kind(kind: &str) -> &'static str {
    match kind {
        "cut" => "cut",
        "dip_to_black" => "fade_to_color",
        "swipe" => "swipe",
        "stinger" => "stinger",
        _ => "fade",
    }
}

fn warning(code: &str, subject: &str, detail: &str) -> ObsExportWarning {
    ObsExportWarning {
        code: code.to_string(),
        subject: subject.to_string(),
        detail: detail.to_string(),
    }
}

fn short_label(collection: &Value) -> String {
    slug(&text(collection, "name"))
}

fn slug(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn int(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap_or_default()
}

fn num(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or_default()
}
