use serde_json::Value;

use crate::obs::domain::AudioChannelPatch;

use super::{ObsService, ObsServiceError, ObsServiceResult, require_text};

impl ObsService {
    pub async fn patch_audio_channel(
        &self,
        channel_id: &str,
        input: AudioChannelPatch,
    ) -> ObsServiceResult<Value> {
        require_text(channel_id, "channel_id")?;
        if let Some(gain_db) = input.gain_db
            && !(-60.0..=24.0).contains(&gain_db)
        {
            return Err(ObsServiceError::Invalid {
                field: "gain_db",
                message: "must be between -60 and 24 dB",
            });
        }
        if let Some(delay_ms) = input.delay_ms
            && !(0..=5000).contains(&delay_ms)
        {
            return Err(ObsServiceError::Invalid {
                field: "delay_ms",
                message: "must be between 0 and 5000 ms",
            });
        }
        Ok(self.store.patch_audio_channel(channel_id, input).await?)
    }
}
