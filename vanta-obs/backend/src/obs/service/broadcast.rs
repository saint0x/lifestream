use serde_json::Value;

use crate::obs::domain::{ActionConfirmationInput, BroadcastInput, BroadcastPatch};

use super::{
    ARCHIVE_POLICIES, LATENCY, ObsService, ObsServiceError, ObsServiceResult, RECORDING_POLICIES,
    VISIBILITY, require_one_of, require_text,
};

const CHAT_MODES: &[&str] = &[
    "open",
    "slow_mode",
    "subscriber_only",
    "follower_only",
    "subscriber_slow_mode",
];

impl ObsService {
    pub async fn create_broadcast(&self, input: BroadcastInput) -> ObsServiceResult<Value> {
        validate_broadcast(&input)?;
        Ok(self.store.create_broadcast(input).await?)
    }

    pub async fn start_broadcast(&self, broadcast_id: &str) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        Ok(self.store.start_broadcast(broadcast_id).await?)
    }

    pub async fn patch_broadcast(
        &self,
        broadcast_id: &str,
        input: BroadcastPatch,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        validate_broadcast_patch(&input)?;
        Ok(self.store.patch_broadcast(broadcast_id, input).await?)
    }

    pub async fn end_broadcast(
        &self,
        broadcast_id: &str,
        input: ActionConfirmationInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        Ok(self.store.end_broadcast(broadcast_id, input).await?)
    }
}

fn validate_broadcast(input: &BroadcastInput) -> ObsServiceResult<()> {
    require_text(&input.title, "title")?;
    require_text(&input.category, "category")?;
    require_one_of(&input.visibility, "visibility", VISIBILITY)?;
    require_one_of(&input.latency_profile, "latency_profile", LATENCY)?;
    require_one_of(
        &input.recording_policy,
        "recording_policy",
        RECORDING_POLICIES,
    )?;
    require_one_of(&input.archive_policy, "archive_policy", ARCHIVE_POLICIES)?;
    Ok(())
}

fn validate_broadcast_patch(input: &BroadcastPatch) -> ObsServiceResult<()> {
    if let Some(title) = input.title.as_deref() {
        require_text(title, "title")?;
    }
    if let Some(category) = input.category.as_deref() {
        require_text(category, "category")?;
    }
    if let Some(language) = input.language.as_deref() {
        require_text(language, "language")?;
    }
    if let Some(visibility) = input.visibility.as_deref() {
        require_one_of(visibility, "visibility", VISIBILITY)?;
    }
    if let Some(chat_mode) = input.chat_mode.as_deref() {
        require_one_of(chat_mode, "chat_mode", CHAT_MODES)?;
    }
    if let Some(tags) = input.tags.as_ref() {
        if tags.len() > 8 {
            return Err(ObsServiceError::Invalid {
                field: "tags",
                message: "supports up to 8 tags",
            });
        }
        if tags.iter().any(|tag| tag.trim().is_empty()) {
            return Err(ObsServiceError::Invalid {
                field: "tags",
                message: "must not contain empty values",
            });
        }
    }
    Ok(())
}
