use serde_json::Value;

use crate::obs::domain::HotkeyPatch;

use super::{ObsService, ObsServiceResult, require_text};

impl ObsService {
    pub async fn patch_hotkey(
        &self,
        hotkey_id: &str,
        input: HotkeyPatch,
    ) -> ObsServiceResult<Value> {
        require_text(hotkey_id, "hotkey_id")?;
        if let Some(binding) = &input.binding {
            require_text(binding, "binding")?;
        }
        Ok(self.store.patch_hotkey(hotkey_id, input).await?)
    }

    pub async fn trigger_hotkey(&self, hotkey_id: &str) -> ObsServiceResult<Value> {
        require_text(hotkey_id, "hotkey_id")?;
        Ok(self.store.trigger_hotkey(hotkey_id).await?)
    }
}
