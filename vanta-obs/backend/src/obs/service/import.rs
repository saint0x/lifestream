use serde_json::Value;

use crate::obs::import::{ObsImportInput, parse_obs_scene_collection};

use super::{ObsService, ObsServiceResult, require_text};

impl ObsService {
    pub async fn import_obs_scene_collection(
        &self,
        input: ObsImportInput,
    ) -> ObsServiceResult<Value> {
        require_text(&input.label, "label")?;
        let plan = parse_obs_scene_collection(input)?;
        Ok(self.store.apply_obs_import(plan).await?)
    }

    pub async fn import_reports(&self) -> ObsServiceResult<Vec<Value>> {
        Ok(self.store.import_reports().await?)
    }

    pub async fn import_report(&self, report_id: &str) -> ObsServiceResult<Value> {
        require_text(report_id, "report_id")?;
        Ok(self.store.import_report(report_id).await?)
    }
}
