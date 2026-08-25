use serde_json::Value;

use crate::obs::domain::{
    BlockedTermInput, ModerationQueueInput, ModerationResolveInput, ModeratorInput,
    PinnedMessageInput,
};

use super::{ObsService, ObsServiceResult, require_one_of, require_text};

const MODERATOR_ROLES: &[&str] = &["owner", "producer", "moderator"];
const BLOCKED_TERM_ACTIONS: &[&str] = &["hold", "hide", "ban"];
const MODERATION_STATUSES: &[&str] = &["approved", "hidden", "banned"];

impl ObsService {
    pub async fn add_moderator(
        &self,
        broadcast_id: &str,
        input: ModeratorInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        require_text(&input.user_id, "user_id")?;
        require_text(&input.display_name, "display_name")?;
        require_one_of(&input.role, "role", MODERATOR_ROLES)?;
        Ok(self.store.add_moderator(broadcast_id, input).await?)
    }

    pub async fn add_blocked_term(
        &self,
        broadcast_id: &str,
        input: BlockedTermInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        require_text(&input.term, "term")?;
        if let Some(action) = input.action.as_deref() {
            require_one_of(action, "action", BLOCKED_TERM_ACTIONS)?;
        }
        Ok(self.store.add_blocked_term(broadcast_id, input).await?)
    }

    pub async fn enqueue_moderation(
        &self,
        broadcast_id: &str,
        input: ModerationQueueInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        require_text(&input.author_id, "author_id")?;
        require_text(&input.author_name, "author_name")?;
        require_text(&input.message, "message")?;
        Ok(self.store.enqueue_moderation(broadcast_id, input).await?)
    }

    pub async fn resolve_moderation(
        &self,
        item_id: &str,
        input: ModerationResolveInput,
    ) -> ObsServiceResult<Value> {
        require_text(item_id, "item_id")?;
        require_one_of(&input.status, "status", MODERATION_STATUSES)?;
        Ok(self.store.resolve_moderation(item_id, input).await?)
    }

    pub async fn pin_message(
        &self,
        broadcast_id: &str,
        input: PinnedMessageInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        require_text(&input.author_name, "author_name")?;
        require_text(&input.message, "message")?;
        Ok(self.store.pin_message(broadcast_id, input).await?)
    }

    pub async fn unpin_message(&self, message_id: &str) -> ObsServiceResult<Value> {
        require_text(message_id, "message_id")?;
        Ok(self.store.unpin_message(message_id).await?)
    }
}
