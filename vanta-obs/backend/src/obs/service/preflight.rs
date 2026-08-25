use crate::obs::domain::{PreflightInput, PreflightResult};

use super::{ObsService, ObsServiceResult, require_text};

impl ObsService {
    pub async fn save_preflight(&self, input: PreflightInput) -> ObsServiceResult<PreflightResult> {
        require_text(&input.broadcast_id, "broadcast_id")?;
        require_text(&input.collection_id, "collection_id")?;
        Ok(self.store.save_preflight(input).await?)
    }
}
