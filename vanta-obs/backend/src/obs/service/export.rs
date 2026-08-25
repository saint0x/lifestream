use serde_json::Value;

use crate::obs::export::{ObsExportInput, build_obs_export_package};

use super::{ObsService, ObsServiceResult, require_text};

impl ObsService {
    pub async fn export_obs_scene_collection(
        &self,
        input: ObsExportInput,
    ) -> ObsServiceResult<Value> {
        require_text(&input.collection_id, "collection_id")?;
        require_text(&input.label, "label")?;
        let bundle = self.store.collection_bundle(&input.collection_id).await?;
        let package = build_obs_export_package(input, bundle)?;
        Ok(self.store.create_obs_export_job(package).await?)
    }

    pub async fn export_jobs(&self) -> ObsServiceResult<Vec<Value>> {
        Ok(self.store.export_jobs().await?)
    }

    pub async fn export_job(&self, job_id: &str) -> ObsServiceResult<Value> {
        require_text(job_id, "job_id")?;
        Ok(self.store.export_job(job_id).await?)
    }
}
