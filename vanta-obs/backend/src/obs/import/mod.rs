use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use thiserror::Error;

use super::adapter::obs_kind_to_vanta_kind;

#[derive(Debug, Clone, Deserialize)]
pub struct ObsImportInput {
    pub label: String,
    pub collection_json: Value,
    pub allow_partial: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsImportPlan {
    pub label: String,
    pub collection_name: String,
    pub canvas_width: i64,
    pub canvas_height: i64,
    pub frame_rate: i64,
    pub scenes: Vec<ImportedScene>,
    pub sources: Vec<ImportedSource>,
    pub instances: Vec<ImportedInstance>,
    pub report: ObsImportReport,
    pub original_metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedScene {
    pub obs_name: String,
    pub vanta_name: String,
    pub order_index: i64,
    pub transition_kind: String,
    pub transition_duration_ms: i64,
    pub locked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedSource {
    pub obs_name: String,
    pub obs_kind: String,
    pub vanta_kind: String,
    pub display_name: String,
    pub settings: Value,
    pub original_metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedInstance {
    pub scene_name: String,
    pub source_name: String,
    pub order_index: i64,
    pub visible: bool,
    pub locked: bool,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub crop: Value,
    pub transform: Value,
    pub opacity: f64,
    pub original_metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsImportReport {
    pub status: String,
    pub imported_scene_count: usize,
    pub imported_source_count: usize,
    pub imported_instance_count: usize,
    pub warnings: Vec<ObsImportWarning>,
    pub omissions: Vec<ObsImportOmission>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsImportWarning {
    pub code: String,
    pub subject: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsImportOmission {
    pub code: String,
    pub subject: String,
    pub detail: String,
}

#[derive(Debug, Error)]
pub enum ObsImportError {
    #[error("invalid OBS collection: {0}")]
    Invalid(String),
    #[error("OBS collection has unsupported items and partial import is disabled")]
    PartialDisabled,
}

pub fn parse_obs_scene_collection(input: ObsImportInput) -> Result<ObsImportPlan, ObsImportError> {
    let root = input.collection_json;
    let allow_partial = input.allow_partial.unwrap_or(false);
    let collection_name = root
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| root.get("current_scene_collection").and_then(Value::as_str))
        .unwrap_or("Imported OBS Collection")
        .to_string();
    let canvas_width = root
        .pointer("/video/base_width")
        .and_then(Value::as_i64)
        .unwrap_or(1920);
    let canvas_height = root
        .pointer("/video/base_height")
        .and_then(Value::as_i64)
        .unwrap_or(1080);
    let frame_rate = root
        .pointer("/video/fps_num")
        .and_then(Value::as_i64)
        .unwrap_or(30);
    let scenes_json = root
        .get("scene_order")
        .and_then(Value::as_array)
        .ok_or_else(|| ObsImportError::Invalid("missing scene_order".to_string()))?;
    let sources_json = root
        .get("sources")
        .and_then(Value::as_array)
        .ok_or_else(|| ObsImportError::Invalid("missing sources".to_string()))?;

    let mut warnings = Vec::new();
    let mut omissions = Vec::new();
    let mut sources = Vec::new();
    let mut instances = Vec::new();
    let scene_order_names = scenes_json
        .iter()
        .filter_map(|item| {
            item.get("name")
                .and_then(Value::as_str)
                .or_else(|| item.as_str())
        })
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let referenced_container_names = referenced_scene_or_group_names(sources_json);
    let container_names = sources_json
        .iter()
        .filter(|source| matches!(text(source, "id").as_str(), "scene" | "group"))
        .map(|source| text(source, "name"))
        .collect::<HashSet<_>>();

    for source in sources_json {
        let name = text(source, "name");
        let kind = text(source, "id");
        if kind == "scene" || kind == "group" {
            collect_instances(&name, source, &mut instances, &mut warnings, &mut omissions);
            if referenced_container_names.contains(&name) {
                sources.push(ImportedSource {
                    obs_name: name.clone(),
                    obs_kind: kind.clone(),
                    vanta_kind: "scene_group".to_string(),
                    display_name: name.clone(),
                    settings: serde_json::json!({
                        "scene_id": Value::Null,
                        "obs_nested_scene_name": name,
                        "group_kind": if kind == "group" { "obs_group" } else { "obs_scene" },
                        "renderer": "nested_scene_graph"
                    }),
                    original_metadata: source.clone(),
                });
            }
            continue;
        }

        if let Some(vanta_kind) = obs_kind_to_vanta_kind(&kind) {
            collect_filter_warnings(&name, source, &mut warnings, &mut omissions);
            sources.push(ImportedSource {
                obs_name: name.clone(),
                obs_kind: kind,
                vanta_kind: vanta_kind.to_string(),
                display_name: name,
                settings: source
                    .get("settings")
                    .cloned()
                    .unwrap_or_else(|| Value::Object(Default::default())),
                original_metadata: source.clone(),
            });
        } else {
            omissions.push(omission(
                "unsupported_source_kind",
                &name,
                &format!("{kind} cannot be expressed as a Vanta OBS source"),
            ));
        }
    }

    let mut scenes = scene_order_names
        .iter()
        .enumerate()
        .map(|(index, name)| ImportedScene {
            obs_name: name.clone(),
            vanta_name: name.clone(),
            order_index: index as i64 + 1,
            transition_kind: map_transition(
                root.get("transition")
                    .and_then(Value::as_str)
                    .unwrap_or("fade"),
            ),
            transition_duration_ms: root
                .get("transition_duration")
                .and_then(Value::as_i64)
                .unwrap_or(300),
            locked: false,
        })
        .collect::<Vec<_>>();
    for name in referenced_container_names {
        if !scene_order_names.contains(&name) && container_names.contains(&name) {
            scenes.push(ImportedScene {
                obs_name: name.clone(),
                vanta_name: format!("Nested Group / {name}"),
                order_index: scenes.len() as i64 + 1,
                transition_kind: "cut".to_string(),
                transition_duration_ms: 0,
                locked: true,
            });
            warnings.push(warning(
                "obs_group_materialized_as_nested_scene",
                &name,
                "OBS group source was materialized as a locked nested Vanta scene so its child graph can round-trip",
            ));
        }
    }

    let supported_sources = sources
        .iter()
        .map(|source| source.obs_name.as_str())
        .collect::<HashSet<_>>();
    instances.retain(|instance| {
        if supported_sources.contains(instance.source_name.as_str()) {
            true
        } else {
            omissions.push(omission(
                "unsupported_scene_item_source",
                &instance.source_name,
                "scene item references a source that was not imported into Vanta OBS",
            ));
            false
        }
    });

    if !allow_partial && !omissions.is_empty() {
        return Err(ObsImportError::PartialDisabled);
    }

    let report = ObsImportReport {
        status: if omissions.is_empty() {
            "ready".to_string()
        } else {
            "partial".to_string()
        },
        imported_scene_count: scenes.len(),
        imported_source_count: sources.len(),
        imported_instance_count: instances.len(),
        warnings,
        omissions,
    };

    Ok(ObsImportPlan {
        label: input.label,
        collection_name,
        canvas_width,
        canvas_height,
        frame_rate,
        scenes,
        sources,
        instances,
        report,
        original_metadata: root,
    })
}

fn referenced_scene_or_group_names(sources_json: &[Value]) -> HashSet<String> {
    let container_names = sources_json
        .iter()
        .filter(|source| matches!(text(source, "id").as_str(), "scene" | "group"))
        .map(|source| text(source, "name"))
        .collect::<HashSet<_>>();
    let mut referenced = HashSet::new();
    for source in sources_json
        .iter()
        .filter(|source| matches!(text(source, "id").as_str(), "scene" | "group"))
    {
        let parent_name = text(source, "name");
        let Some(items) = source
            .get("settings")
            .and_then(|settings| settings.get("items"))
            .and_then(Value::as_array)
        else {
            continue;
        };
        for item in items {
            let child_name = text(item, "name");
            if child_name != parent_name && container_names.contains(&child_name) {
                referenced.insert(child_name);
            }
        }
    }
    referenced
}

fn collect_instances(
    scene_name: &str,
    source: &Value,
    instances: &mut Vec<ImportedInstance>,
    warnings: &mut Vec<ObsImportWarning>,
    omissions: &mut Vec<ObsImportOmission>,
) {
    let items = source
        .get("settings")
        .and_then(|settings| settings.get("items"))
        .and_then(Value::as_array);
    let Some(items) = items else {
        return;
    };
    for (index, item) in items.iter().enumerate() {
        let source_name = text(item, "name");
        let visible = item.get("visible").and_then(Value::as_bool).unwrap_or(true);
        let locked = item
            .get("locked")
            .and_then(Value::as_bool)
            .or_else(|| item.get("lock").and_then(Value::as_bool))
            .unwrap_or(false);
        let transform = item
            .get("pos")
            .cloned()
            .unwrap_or_else(|| Value::Object(Default::default()));
        let crop = item
            .get("crop")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({"top":0,"right":0,"bottom":0,"left":0}));
        let width = item
            .get("bounds")
            .and_then(|bounds| bounds.get("x"))
            .and_then(Value::as_f64)
            .or_else(|| {
                item.get("scale")
                    .and_then(|scale| scale.get("x"))
                    .and_then(Value::as_f64)
                    .map(|scale| 1920.0 * scale)
            })
            .unwrap_or(1920.0);
        let height = item
            .get("bounds")
            .and_then(|bounds| bounds.get("y"))
            .and_then(Value::as_f64)
            .or_else(|| {
                item.get("scale")
                    .and_then(|scale| scale.get("y"))
                    .and_then(Value::as_f64)
                    .map(|scale| 1080.0 * scale)
            })
            .unwrap_or(1080.0);
        if source_name.is_empty() {
            omissions.push(omission(
                "missing_scene_item_source",
                scene_name,
                "scene item did not include a source name",
            ));
            continue;
        }
        if item.get("blend_method").is_some() || item.get("blend_type").is_some() {
            warnings.push(warning(
                "unsupported_blend_mode",
                &source_name,
                "Vanta stores the original OBS blend metadata but does not apply arbitrary blend modes",
            ));
        }
        instances.push(ImportedInstance {
            scene_name: scene_name.to_string(),
            source_name,
            order_index: index as i64 + 1,
            visible,
            locked,
            x: transform.get("x").and_then(Value::as_f64).unwrap_or(0.0),
            y: transform.get("y").and_then(Value::as_f64).unwrap_or(0.0),
            width,
            height,
            crop,
            transform,
            opacity: 1.0,
            original_metadata: item.clone(),
        });
    }
}

fn collect_filter_warnings(
    source_name: &str,
    source: &Value,
    warnings: &mut Vec<ObsImportWarning>,
    omissions: &mut Vec<ObsImportOmission>,
) {
    let Some(filters) = source.get("filters").and_then(Value::as_array) else {
        return;
    };
    for filter in filters {
        let name = text(filter, "name");
        let kind = text(filter, "id");
        match kind.as_str() {
            "gain_filter" | "noise_suppress_filter" | "compressor_filter" | "limiter_filter" => {
                warnings.push(warning(
                    "audio_filter_preserved",
                    &name,
                    &format!("{kind} is preserved for later audio graph mapping"),
                ));
            }
            _ => omissions.push(omission(
                "unsupported_filter",
                source_name,
                &format!("{kind} filter is not imported into Vanta OBS"),
            )),
        }
    }
}

fn map_transition(kind: &str) -> String {
    match kind {
        "cut" | "fade" | "swipe" | "stinger" => kind.to_string(),
        "fade_to_color" => "dip_to_black".to_string(),
        _ => "fade".to_string(),
    }
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn warning(code: &str, subject: &str, detail: &str) -> ObsImportWarning {
    ObsImportWarning {
        code: code.to_string(),
        subject: subject.to_string(),
        detail: detail.to_string(),
    }
}

fn omission(code: &str, subject: &str, detail: &str) -> ObsImportOmission {
    ObsImportOmission {
        code: code.to_string(),
        subject: subject.to_string(),
        detail: detail.to_string(),
    }
}
