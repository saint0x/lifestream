use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Deserialize)]
pub struct TimelinePatch {
    pub playhead_seconds: Option<f64>,
    pub selected_id: Option<String>,
    pub zoom: Option<f64>,
    pub safe_areas: Option<bool>,
    pub waveform: Option<bool>,
    pub edl_json: Option<Value>,
    pub change_summary: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RenderRequest {
    pub export_kind: String,
    pub target: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct PublishValidation {
    pub valid: bool,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
}

pub fn validate_publish(bundle: &Value) -> PublishValidation {
    let mut warnings = Vec::new();
    let mut blockers = Vec::new();
    let Some(ad_slots) = bundle.get("ad_slots").and_then(Value::as_array) else {
        blockers.push("timeline has no ad inventory track".to_string());
        return PublishValidation {
            valid: false,
            warnings,
            blockers,
        };
    };

    for slot in ad_slots {
        let label = slot
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or("ad slot");
        let status = slot
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("draft");
        let review = slot
            .get("review_status")
            .and_then(Value::as_str)
            .unwrap_or("not_required");
        let duration = slot
            .get("timeline_out_seconds")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            - slot
                .get("timeline_in_seconds")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
        let required = slot
            .get("required_duration_seconds")
            .and_then(Value::as_f64)
            .unwrap_or(duration);
        let has_asset = slot
            .get("selected_media_asset_id")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty());

        if !has_asset {
            blockers.push(format!("{label} needs linked creative or host-read proof"));
        }
        if duration + 0.25 < required {
            blockers.push(format!("{label} is shorter than the sold requirement"));
        }
        if matches!(
            status,
            "draft" | "needs_asset" | "needs_creator_recording" | "revision_requested"
        ) {
            blockers.push(format!("{label} is still {status}"));
        }
        if !matches!(review, "approved" | "not_required") {
            blockers.push(format!("{label} is awaiting required review"));
        }
        if matches!(status, "approved" | "locked") && review == "approved" {
            warnings.push(format!("{label} is render-safe"));
        }
    }

    PublishValidation {
        valid: blockers.is_empty(),
        warnings,
        blockers,
    }
}

pub fn render_plan(
    bundle: &Value,
    request: &RenderRequest,
    validation: PublishValidation,
) -> Value {
    let timeline = bundle.get("timeline").cloned().unwrap_or_else(|| json!({}));
    let assets = bundle.get("assets").cloned().unwrap_or_else(|| json!([]));
    let ad_slots = bundle.get("ad_slots").cloned().unwrap_or_else(|| json!([]));
    let versions = bundle.get("versions").cloned().unwrap_or_else(|| json!([]));
    let latest_version = versions.as_array().and_then(|items| items.last()).cloned();

    json!({
        "target": request.target,
        "export_kind": request.export_kind,
        "timeline_revision_id": latest_version.as_ref().and_then(|v| v.get("id")).cloned(),
        "timeline": timeline,
        "source_assets": assets,
        "ad_slot_outputs": ad_slots,
        "hls_variants": [
            { "height": 1080, "bitrate": 6200000 },
            { "height": 720, "bitrate": 3600000 },
            { "height": 480, "bitrate": 1600000 }
        ],
        "caption_outputs": ["vtt", "srt"],
        "thumbnail_outputs": ["poster", "timeline-strip"],
        "ffmpeg_filtergraph_or_equivalent": "edl->concat->loudnorm->overlay(ad_slots,graphics)->package",
        "validation_warnings": validation.warnings,
        "validation_blockers": validation.blockers
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_validation_blocks_incomplete_sold_inventory() {
        let bundle = json!({
            "ad_slots": [{
                "label": "Launch mid-roll",
                "status": "needs_asset",
                "review_status": "pending",
                "timeline_in_seconds": 10.0,
                "timeline_out_seconds": 25.0,
                "required_duration_seconds": 30.0
            }]
        });

        let validation = validate_publish(&bundle);

        assert!(!validation.valid);
        assert_eq!(validation.blockers.len(), 4);
        assert!(
            validation
                .blockers
                .iter()
                .any(|item| item.contains("needs linked creative"))
        );
        assert!(
            validation
                .blockers
                .iter()
                .any(|item| item.contains("shorter"))
        );
    }

    #[test]
    fn render_plan_preserves_reproducible_inputs() {
        let bundle = json!({
            "timeline": { "id": "timeline_1", "duration_seconds": 90.0 },
            "assets": [{ "media_asset_id": "media_1" }],
            "ad_slots": [{ "id": "slot_1", "status": "locked" }],
            "versions": [{ "id": "version_7", "version_number": 7 }]
        });
        let request = RenderRequest {
            export_kind: "final_vanta_master".to_string(),
            target: "hls-master".to_string(),
        };

        let plan = render_plan(
            &bundle,
            &request,
            PublishValidation {
                valid: true,
                warnings: vec!["slot_1 is render-safe".to_string()],
                blockers: vec![],
            },
        );

        assert_eq!(plan["timeline_revision_id"], "version_7");
        assert_eq!(plan["source_assets"][0]["media_asset_id"], "media_1");
        assert_eq!(plan["ad_slot_outputs"][0]["id"], "slot_1");
        assert_eq!(plan["caption_outputs"][0], "vtt");
    }
}
