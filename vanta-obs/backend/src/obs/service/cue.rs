use serde_json::Value;

use crate::obs::domain::CueInput;

use super::{CUE_KINDS, ObsService, ObsServiceResult, require_one_of, require_text};

impl ObsService {
    pub async fn create_cue(&self, broadcast_id: &str, input: CueInput) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        require_one_of(&input.cue_kind, "cue_kind", CUE_KINDS)?;
        require_text(&input.label, "label")?;
        Ok(self.store.create_cue(broadcast_id, input).await?)
    }

    pub async fn trigger_cue(&self, cue_id: &str) -> ObsServiceResult<Value> {
        require_text(cue_id, "cue_id")?;
        Ok(self.store.trigger_cue(cue_id).await?)
    }
}
