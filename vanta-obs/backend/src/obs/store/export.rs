use crate::obs::export::ObsExportPackage;

use super::{
    ObsStore, ObsStoreError,
    row::{id, now},
};

impl ObsStore {
    pub async fn create_obs_export_job(
        &self,
        package: ObsExportPackage,
    ) -> Result<serde_json::Value, ObsStoreError> {
        let job_id = id();
        let now = now();
        sqlx::query(
            "INSERT INTO obs_export_jobs
            (id, creator_id, collection_id, label, status, scene_collection_json, asset_manifest_json, warnings_json, setup_instructions_json, created_at)
            VALUES (?, 'creator_vanta_originals', ?, ?, 'ready', ?, ?, ?, ?, ?)",
        )
        .bind(&job_id)
        .bind(&package.collection_id)
        .bind(&package.label)
        .bind(package.scene_collection_json.to_string())
        .bind(serde_json::to_string(&package.asset_manifest)?)
        .bind(serde_json::to_string(&package.warnings)?)
        .bind(serde_json::to_string(&package.setup_instructions)?)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.export_job(&job_id).await
    }

    pub async fn export_jobs(&self) -> Result<Vec<serde_json::Value>, ObsStoreError> {
        self.list(
            "SELECT * FROM obs_export_jobs ORDER BY created_at DESC",
            &[],
        )
        .await
    }

    pub async fn export_job(&self, job_id: &str) -> Result<serde_json::Value, ObsStoreError> {
        self.row("SELECT * FROM obs_export_jobs WHERE id = ?", &[job_id])
            .await
    }
}
