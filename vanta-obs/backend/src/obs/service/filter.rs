use serde_json::Value;

use crate::obs::domain::{SourceFilterInput, SourceFilterPatch};

use super::{ObsService, ObsServiceResult, require_text};

impl ObsService {
    pub async fn create_source_filter(
        &self,
        source_id: &str,
        input: SourceFilterInput,
    ) -> ObsServiceResult<Value> {
        require_text(source_id, "source_id")?;
        require_text(&input.filter_kind, "filter_kind")?;
        require_text(&input.label, "label")?;
        Ok(self.store.create_source_filter(source_id, input).await?)
    }

    pub async fn patch_source_filter(
        &self,
        filter_id: &str,
        input: SourceFilterPatch,
    ) -> ObsServiceResult<Value> {
        require_text(filter_id, "filter_id")?;
        if let Some(label) = &input.label {
            require_text(label, "label")?;
        }
        Ok(self.store.patch_source_filter(filter_id, input).await?)
    }

    pub async fn disable_source_filter(&self, filter_id: &str) -> ObsServiceResult<Value> {
        require_text(filter_id, "filter_id")?;
        Ok(self.store.disable_source_filter(filter_id).await?)
    }
}
