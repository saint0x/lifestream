use serde_json::Value;

use crate::obs::domain::{
    EngagementAlertInput, EngagementPollInput, EngagementVoteInput, ScheduleSlotInput,
    ScheduleSlotPatch,
};

use super::{ObsService, ObsServiceError, ObsServiceResult, require_one_of, require_text};

const SCHEDULE_STATUSES: &[&str] = &["scheduled", "rescheduled", "cancelled", "completed"];
const POLL_KINDS: &[&str] = &["poll", "prediction"];
const ALERT_KINDS: &[&str] = &[
    "follow",
    "subscription",
    "tip",
    "raid",
    "sponsor",
    "milestone",
    "system",
];
const ALERT_SEVERITIES: &[&str] = &["info", "success", "warning", "critical"];

impl ObsService {
    pub async fn create_schedule_slot(
        &self,
        broadcast_id: &str,
        input: ScheduleSlotInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        require_text(&input.title, "title")?;
        require_text(&input.starts_at, "starts_at")?;
        require_duration(input.duration_minutes, "duration_minutes", 5, 24 * 60)?;
        Ok(self.store.create_schedule_slot(broadcast_id, input).await?)
    }

    pub async fn patch_schedule_slot(
        &self,
        slot_id: &str,
        input: ScheduleSlotPatch,
    ) -> ObsServiceResult<Value> {
        require_text(slot_id, "slot_id")?;
        if let Some(title) = input.title.as_deref() {
            require_text(title, "title")?;
        }
        if let Some(starts_at) = input.starts_at.as_deref() {
            require_text(starts_at, "starts_at")?;
        }
        if let Some(duration) = input.duration_minutes {
            require_duration(duration, "duration_minutes", 5, 24 * 60)?;
        }
        if let Some(status) = input.status.as_deref() {
            require_one_of(status, "status", SCHEDULE_STATUSES)?;
        }
        Ok(self.store.patch_schedule_slot(slot_id, input).await?)
    }

    pub async fn create_engagement_poll(
        &self,
        broadcast_id: &str,
        input: EngagementPollInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        require_one_of(&input.poll_kind, "poll_kind", POLL_KINDS)?;
        require_text(&input.question, "question")?;
        if !(2..=6).contains(&input.options.len()) {
            return Err(ObsServiceError::Invalid {
                field: "options",
                message: "must contain between two and six options",
            });
        }
        for option in &input.options {
            require_text(option, "options")?;
        }
        require_duration(input.duration_seconds, "duration_seconds", 10, 86_400)?;
        Ok(self
            .store
            .create_engagement_poll(broadcast_id, input)
            .await?)
    }

    pub async fn vote_engagement_poll(
        &self,
        poll_id: &str,
        input: EngagementVoteInput,
    ) -> ObsServiceResult<Value> {
        require_text(poll_id, "poll_id")?;
        require_text(&input.option_id, "option_id")?;
        require_text(&input.voter_id, "voter_id")?;
        Ok(self.store.vote_engagement_poll(poll_id, input).await?)
    }

    pub async fn close_engagement_poll(&self, poll_id: &str) -> ObsServiceResult<Value> {
        require_text(poll_id, "poll_id")?;
        Ok(self.store.close_engagement_poll(poll_id).await?)
    }

    pub async fn create_engagement_alert(
        &self,
        broadcast_id: &str,
        input: EngagementAlertInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        require_one_of(&input.alert_kind, "alert_kind", ALERT_KINDS)?;
        require_text(&input.title, "title")?;
        require_text(&input.message, "message")?;
        if let Some(severity) = input.severity.as_deref() {
            require_one_of(severity, "severity", ALERT_SEVERITIES)?;
        }
        if input.amount_cents.unwrap_or_default() < 0 {
            return Err(ObsServiceError::Invalid {
                field: "amount_cents",
                message: "must not be negative",
            });
        }
        Ok(self
            .store
            .create_engagement_alert(broadcast_id, input)
            .await?)
    }
}

fn require_duration(value: i64, field: &'static str, min: i64, max: i64) -> ObsServiceResult<()> {
    if value < min || value > max {
        return Err(ObsServiceError::Invalid {
            field,
            message: "is outside the supported range",
        });
    }
    Ok(())
}
