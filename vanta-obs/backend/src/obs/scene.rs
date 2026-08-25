use serde_json::{Value, json};
use std::collections::HashMap;

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn int_bool(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_i64).unwrap_or_default() != 0
}

fn num(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or_default()
}

pub fn scene_validation(
    scene: &Value,
    instances: &[Value],
    sources: &[Value],
    canvas_width: f64,
    canvas_height: f64,
    role: &str,
) -> Value {
    let source_by_id = sources
        .iter()
        .map(|source| (text(source, "id"), source))
        .collect::<HashMap<_, _>>();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut visible_count = 0;
    let mut video_count = 0;
    let mut source_ids = Vec::new();

    for instance in instances {
        if !int_bool(instance, "visible") {
            continue;
        }
        visible_count += 1;
        let source_id = text(instance, "source_id");
        source_ids.push(source_id.clone());
        if num(instance, "width") <= 0.0 || num(instance, "height") <= 0.0 {
            errors.push(format!("{source_id}:invalid_bounds"));
        }
        if num(instance, "opacity") < 0.0 || num(instance, "opacity") > 1.0 {
            errors.push(format!("{source_id}:invalid_opacity"));
        }
        if num(instance, "x") + num(instance, "width") <= 0.0
            || num(instance, "y") + num(instance, "height") <= 0.0
            || num(instance, "x") >= canvas_width
            || num(instance, "y") >= canvas_height
        {
            warnings.push(format!("{source_id}:off_canvas"));
        }
        let Some(source) = source_by_id.get(&source_id) else {
            errors.push(format!("{source_id}:missing_source"));
            continue;
        };
        let renderer = source
            .pointer("/source_contract_json/renderer")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if renderer != "device_audio" {
            video_count += 1;
        }
        let validation_status = source
            .pointer("/source_validation_json/status")
            .and_then(Value::as_str)
            .unwrap_or("blocked");
        let sync_status = source
            .pointer("/source_sync_json/status")
            .and_then(Value::as_str)
            .unwrap_or("blocked");
        let permission_state = text(source, "permission_state");
        if validation_status == "blocked" || sync_status == "blocked" {
            errors.push(format!("{source_id}:source_blocked"));
        } else if validation_status == "warning"
            || sync_status == "pending"
            || permission_state == "pending"
        {
            warnings.push(format!("{source_id}:source_pending"));
        }
        if text(source, "health_state") == "warning" {
            warnings.push(format!("{source_id}:source_warning"));
        }
    }

    if visible_count == 0 {
        errors.push("no_visible_sources".to_string());
    }
    if video_count == 0 {
        warnings.push("no_visible_video_source".to_string());
    }
    let stored_state = text(scene, "validation_state");
    if stored_state != "ready" {
        warnings.push(format!("stored_state:{stored_state}"));
    }

    errors.sort();
    errors.dedup();
    warnings.sort();
    warnings.dedup();
    let status = if errors.is_empty() {
        if warnings.is_empty() {
            "ready"
        } else {
            "warning"
        }
    } else {
        "blocked"
    };
    json!({
        "status": status,
        "role": role,
        "visible_instances": visible_count,
        "video_instances": video_count,
        "source_ids": source_ids,
        "errors": errors,
        "warnings": warnings
    })
}
