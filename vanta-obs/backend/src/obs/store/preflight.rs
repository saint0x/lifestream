use super::{ObsStore, ObsStoreError, row::text};
use crate::native::package::fallback_plan;
use crate::obs::domain::{CheckResult, PreflightInput, PreflightResult};

impl ObsStore {
    pub(super) async fn evaluate_preflight(
        &self,
        input: &PreflightInput,
    ) -> Result<PreflightResult, ObsStoreError> {
        let scenes = self.scenes(&input.collection_id).await?;
        let sources = self.sources().await?;
        let audio = self.audio_channels(&input.broadcast_id).await?;
        let cues = self.cues(&input.broadcast_id).await?;
        let runtime = self.runtime(&input.broadcast_id).await?;
        let native_fallback = fallback_plan();
        let checks = vec![
            check(
                "camera",
                "Camera permission",
                sources.iter().any(|s| {
                    text(s, "source_kind") == "camera" && text(s, "permission_state") == "granted"
                }),
                "Primary camera is available",
            ),
            check(
                "microphone",
                "Microphone permission",
                sources.iter().any(|s| {
                    text(s, "source_kind") == "microphone"
                        && text(s, "permission_state") == "granted"
                }),
                "Host microphone has permission",
            ),
            check(
                "scene",
                "Program scene",
                scenes.iter().any(|scene| {
                    scene
                        .pointer("/scene_validation_json/status")
                        .and_then(serde_json::Value::as_str)
                        == Some("ready")
                        || scene
                            .pointer("/scene_validation_json/status")
                            .and_then(serde_json::Value::as_str)
                            == Some("warning")
                }),
                "Selected scene has visible video sources",
            ),
            check(
                "audio",
                "Audio meter",
                audio.iter().any(|c| {
                    c.pointer("/audio_graph_json/buses/program")
                        .and_then(serde_json::Value::as_bool)
                        == Some(true)
                        && c.pointer("/audio_graph_json/meter/silent")
                            .and_then(serde_json::Value::as_bool)
                            == Some(false)
                        && text(c, "channel_kind") == "microphone"
                }),
                "Program microphone channel is active",
            ),
            check(
                "runtime",
                "Runtime route",
                matches!(
                    text(&runtime, "runtime_state").as_str(),
                    "ready" | "healthy" | "program_updated"
                ),
                "Vanta ingest/runtime binding is ready",
            ),
            check(
                "sponsor",
                "Sponsor cues",
                cues.iter().all(|c| {
                    matches!(text(c, "status").as_str(), "ready" | "armed" | "shown_live")
                }),
                "Sold live inventory is ready",
            ),
        ];
        let blockers = checks
            .iter()
            .filter(|c| c.status == "blocked")
            .map(|c| c.label.clone())
            .collect::<Vec<_>>();
        let mut warnings = sources
            .iter()
            .filter(|s| text(s, "health_state") == "warning")
            .map(|s| format!("{} reports warning health", text(s, "display_name")))
            .collect::<Vec<_>>();
        warnings.extend(scenes.iter().filter_map(|scene| {
            let status = scene
                .pointer("/scene_validation_json/status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("blocked");
            if status == "warning" {
                Some(format!(
                    "{} has scene validation warnings",
                    text(scene, "name")
                ))
            } else {
                None
            }
        }));
        if native_fallback
            .get("native_ready")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        {
            warnings.push(
                "Native helpers are unavailable; browser preview plus external ingest fallback is ready"
                    .to_string(),
            );
        }
        Ok(PreflightResult {
            ready: blockers.is_empty(),
            checks,
            blockers,
            warnings,
        })
    }
}

fn check(key: &str, label: &str, pass: bool, detail: &str) -> CheckResult {
    CheckResult {
        key: key.to_string(),
        label: label.to_string(),
        status: if pass { "pass" } else { "blocked" }.to_string(),
        detail: detail.to_string(),
    }
}
