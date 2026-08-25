use serde_json::Value;

use crate::obs::{
    domain::ReplayInput,
    replay_media::{ReplayClipRequest, ReplayMediaEngine},
};

use super::{ObsService, ObsServiceError, ObsServiceResult, require_text};

impl ObsService {
    pub async fn save_replay(
        &self,
        broadcast_id: &str,
        input: ReplayInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        if !(5..=300).contains(&input.duration_seconds) {
            return Err(ObsServiceError::Invalid {
                field: "duration_seconds",
                message: "must be between 5 and 300 seconds",
            });
        }
        let marker_id = self.store.next_id();
        let media_asset_id = format!("media_asset_replay_{marker_id}");
        let source = self.store.replay_media_source(broadcast_id).await?;
        let clip = ReplayMediaEngine
            .save_clip(ReplayClipRequest {
                marker_id: marker_id.clone(),
                broadcast_id: broadcast_id.to_string(),
                media_asset_id,
                duration_seconds: input.duration_seconds,
                sponsor_proof: input.sponsor_proof.unwrap_or(false),
                source,
            })
            .await?;
        Ok(self
            .store
            .save_replay_with_clip(broadcast_id, marker_id, input, clip)
            .await?)
    }
}
