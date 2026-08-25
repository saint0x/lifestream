use serde_json::Value;

use crate::obs::domain::{AudienceTelemetryInput, RaidRedirectInput};

use super::{ObsService, ObsServiceError, ObsServiceResult, require_text};

impl ObsService {
    pub async fn ingest_audience_telemetry(
        &self,
        broadcast_id: &str,
        input: AudienceTelemetryInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        if input.viewer_count < 0 {
            return Err(ObsServiceError::Invalid {
                field: "viewer_count",
                message: "must not be negative",
            });
        }
        if input.chat_messages_per_minute.unwrap_or_default() < 0 {
            return Err(ObsServiceError::Invalid {
                field: "chat_messages_per_minute",
                message: "must not be negative",
            });
        }
        if input.tips_cents.unwrap_or_default() < 0 {
            return Err(ObsServiceError::Invalid {
                field: "tips_cents",
                message: "must not be negative",
            });
        }
        if input.subscriptions.unwrap_or_default() < 0 {
            return Err(ObsServiceError::Invalid {
                field: "subscriptions",
                message: "must not be negative",
            });
        }
        if input.revenue_cents.unwrap_or_default() < 0 {
            return Err(ObsServiceError::Invalid {
                field: "revenue_cents",
                message: "must not be negative",
            });
        }
        if let Some(score) = input.discovery_score
            && !(0.0..=100.0).contains(&score)
        {
            return Err(ObsServiceError::Invalid {
                field: "discovery_score",
                message: "must be between 0 and 100",
            });
        }
        Ok(self
            .store
            .ingest_audience_telemetry(broadcast_id, input)
            .await?)
    }

    pub async fn schedule_raid_redirect(
        &self,
        broadcast_id: &str,
        input: RaidRedirectInput,
    ) -> ObsServiceResult<Value> {
        validate_raid_input(broadcast_id, &input, false)?;
        Ok(self
            .store
            .schedule_raid_redirect(broadcast_id, input)
            .await?)
    }

    pub async fn record_inbound_raid(
        &self,
        broadcast_id: &str,
        input: RaidRedirectInput,
    ) -> ObsServiceResult<Value> {
        validate_raid_input(broadcast_id, &input, true)?;
        Ok(self.store.record_inbound_raid(broadcast_id, input).await?)
    }
}

fn validate_raid_input(
    broadcast_id: &str,
    input: &RaidRedirectInput,
    inbound: bool,
) -> ObsServiceResult<()> {
    require_text(broadcast_id, "broadcast_id")?;
    require_text(&input.target_channel_id, "target_channel_id")?;
    require_text(&input.target_channel_name, "target_channel_name")?;
    if input.viewer_count.unwrap_or_default() < 0 {
        return Err(ObsServiceError::Invalid {
            field: "viewer_count",
            message: "must not be negative",
        });
    }
    let countdown = input
        .execute_after_seconds
        .unwrap_or(if inbound { 0 } else { 30 });
    if !inbound && !(5..=600).contains(&countdown) {
        return Err(ObsServiceError::Invalid {
            field: "execute_after_seconds",
            message: "must be between 5 and 600",
        });
    }
    if let Some(url) = input.redirect_url.as_deref()
        && !url.trim().is_empty()
        && !(url.starts_with("https://") || url.starts_with("vanta://"))
    {
        return Err(ObsServiceError::Invalid {
            field: "redirect_url",
            message: "must use https:// or vanta://",
        });
    }
    Ok(())
}
