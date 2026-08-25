use serde_json::{Value, json};

use crate::obs::domain::{
    EmergencyDisconnectInput, HotkeyPatch, RecordingInput, ReplayInput, bool_int,
};

use super::{
    ObsStore, ObsStoreError,
    row::{int, now, text},
};

impl ObsStore {
    pub(super) async fn hotkeys(&self) -> Result<Vec<Value>, ObsStoreError> {
        self.list(
            "SELECT * FROM obs_hotkeys ORDER BY scope ASC, action ASC, binding ASC",
            &[],
        )
        .await
    }

    pub async fn patch_hotkey(
        &self,
        hotkey_id: &str,
        input: HotkeyPatch,
    ) -> Result<Value, ObsStoreError> {
        let current = self
            .row("SELECT * FROM obs_hotkeys WHERE id = ?", &[hotkey_id])
            .await?;
        let binding = input.binding.unwrap_or_else(|| text(&current, "binding"));
        sqlx::query("UPDATE obs_hotkeys SET binding = ?, enabled = ?, updated_at = ? WHERE id = ?")
            .bind(binding)
            .bind(bool_int(
                input
                    .enabled
                    .unwrap_or_else(|| int(&current, "enabled") != 0),
            ))
            .bind(now())
            .bind(hotkey_id)
            .execute(&self.pool)
            .await?;
        self.row("SELECT * FROM obs_hotkeys WHERE id = ?", &[hotkey_id])
            .await
    }

    pub async fn trigger_hotkey(&self, hotkey_id: &str) -> Result<Value, ObsStoreError> {
        let hotkey = self
            .row("SELECT * FROM obs_hotkeys WHERE id = ?", &[hotkey_id])
            .await?;
        if int(&hotkey, "enabled") == 0 {
            return Ok(json!({
                "hotkey": hotkey,
                "status": "ignored",
                "reason": "disabled",
                "dashboard": self.dashboard().await?
            }));
        }

        let broadcast = self.active_broadcast().await?;
        let broadcast_id = text(&broadcast, "id");
        let action = text(&hotkey, "action");
        let result = match action.as_str() {
            "scene.send_program" => {
                let target_id = text(&hotkey, "target_id");
                self.send_to_program(&target_id).await?
            }
            "broadcast.start" => self.start_broadcast(&broadcast_id).await?,
            "recording.start" => {
                self.start_recording(
                    &broadcast_id,
                    RecordingInput {
                        recording_mode: "program_plus_isolated_audio".to_string(),
                        operator_id: Some("creator_vanta_originals".to_string()),
                        operator_role: Some("creator_owner".to_string()),
                        confirmation_text: None,
                        acknowledged_risks: None,
                    },
                )
                .await?
            }
            "replay.save_30" => {
                self.save_replay(
                    &broadcast_id,
                    ReplayInput {
                        duration_seconds: 30,
                        label: Some("Hotkey replay".to_string()),
                        sponsor_proof: Some(true),
                    },
                )
                .await?
            }
            "safety.hold" => {
                self.emergency_disconnect(
                    &broadcast_id,
                    EmergencyDisconnectInput {
                        reason: Some("Hotkey emergency hold".to_string()),
                        operator_id: Some("creator_vanta_originals".to_string()),
                        operator_role: Some("creator_owner".to_string()),
                        confirmation_text: None,
                        acknowledged_risks: None,
                    },
                )
                .await?
            }
            _ => {
                return Ok(json!({
                    "hotkey": hotkey,
                    "status": "ignored",
                    "reason": "unsupported_action",
                    "dashboard": self.dashboard().await?
                }));
            }
        };
        self.add_event(
            Some(&broadcast_id),
            "hotkey_trigger",
            &format!("{} executed from {}", action, text(&hotkey, "binding")),
        )
        .await?;
        Ok(json!({
            "hotkey": hotkey,
            "status": "executed",
            "result": result,
            "dashboard": self.dashboard().await?
        }))
    }
}
