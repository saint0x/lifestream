use serde_json::Value;

use super::{ObsService, ObsServiceResult, require_text};

impl ObsService {
    pub async fn dashboard(&self) -> ObsServiceResult<Value> {
        Ok(self.store.dashboard().await?)
    }

    pub async fn collections(&self) -> ObsServiceResult<Vec<Value>> {
        Ok(self.store.collections().await?)
    }

    pub async fn collection_bundle(&self, collection_id: &str) -> ObsServiceResult<Value> {
        require_text(collection_id, "collection_id")?;
        Ok(self.store.collection_bundle(collection_id).await?)
    }
}
