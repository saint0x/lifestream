use serde_json::Value;

use crate::obs::domain::{ActionConfirmationInput, RecordingInput};

use super::{ObsService, ObsServiceResult, RECORDING_MODES, require_one_of, require_text};

impl ObsService {
    pub async fn start_recording(
        &self,
        broadcast_id: &str,
        input: RecordingInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        require_one_of(&input.recording_mode, "recording_mode", RECORDING_MODES)?;
        Ok(self.store.start_recording(broadcast_id, input).await?)
    }

    pub async fn stop_recording(
        &self,
        broadcast_id: &str,
        input: ActionConfirmationInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        Ok(self.store.stop_recording(broadcast_id, input).await?)
    }

    pub async fn pause_recording(&self, broadcast_id: &str) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        Ok(self.store.pause_recording(broadcast_id).await?)
    }

    pub async fn resume_recording(&self, broadcast_id: &str) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        Ok(self.store.resume_recording(broadcast_id).await?)
    }

    pub async fn discard_recording(
        &self,
        broadcast_id: &str,
        input: ActionConfirmationInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        Ok(self.store.discard_recording(broadcast_id, input).await?)
    }
}
