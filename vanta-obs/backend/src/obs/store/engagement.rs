use serde_json::{Value, json};

use crate::obs::domain::{
    EngagementAlertInput, EngagementPollInput, EngagementVoteInput, ScheduleSlotInput,
    ScheduleSlotPatch,
};

use super::{
    ObsStore, ObsStoreError,
    row::{int, now, short_id, text},
};

impl ObsStore {
    pub async fn create_schedule_slot(
        &self,
        broadcast_id: &str,
        input: ScheduleSlotInput,
    ) -> Result<Value, ObsStoreError> {
        self.row(
            "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
            &[broadcast_id],
        )
        .await?;
        let created_at = now();
        sqlx::query(
            "INSERT INTO obs_schedule_slots
            (id, broadcast_id, title, starts_at, timezone, duration_minutes, status, reminder_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, 'scheduled', ?, ?, ?)",
        )
        .bind(format!("schedule_{}", short_id()))
        .bind(broadcast_id)
        .bind(input.title.trim())
        .bind(input.starts_at.trim())
        .bind(input.timezone.unwrap_or_else(|| "America/New_York".to_string()))
        .bind(input.duration_minutes)
        .bind(input.reminder_json.unwrap_or_else(|| json!({"notify_followers": true})).to_string())
        .bind(&created_at)
        .bind(&created_at)
        .execute(&self.pool)
        .await?;
        self.add_event(Some(broadcast_id), "schedule_slot", "Live schedule updated")
            .await?;
        self.dashboard().await
    }

    pub async fn patch_schedule_slot(
        &self,
        slot_id: &str,
        input: ScheduleSlotPatch,
    ) -> Result<Value, ObsStoreError> {
        let current = self
            .row("SELECT * FROM obs_schedule_slots WHERE id = ?", &[slot_id])
            .await?;
        let broadcast_id = text(&current, "broadcast_id");
        sqlx::query(
            "UPDATE obs_schedule_slots SET title = ?, starts_at = ?, timezone = ?,
             duration_minutes = ?, status = ?, reminder_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(input.title.unwrap_or_else(|| text(&current, "title")))
        .bind(
            input
                .starts_at
                .unwrap_or_else(|| text(&current, "starts_at")),
        )
        .bind(input.timezone.unwrap_or_else(|| text(&current, "timezone")))
        .bind(
            input
                .duration_minutes
                .unwrap_or_else(|| int(&current, "duration_minutes")),
        )
        .bind(input.status.unwrap_or_else(|| text(&current, "status")))
        .bind(
            input
                .reminder_json
                .unwrap_or_else(|| current["reminder_json"].clone())
                .to_string(),
        )
        .bind(now())
        .bind(slot_id)
        .execute(&self.pool)
        .await?;
        self.add_event(
            Some(&broadcast_id),
            "schedule_slot",
            "Live schedule changed",
        )
        .await?;
        self.dashboard().await
    }

    pub async fn create_engagement_poll(
        &self,
        broadcast_id: &str,
        input: EngagementPollInput,
    ) -> Result<Value, ObsStoreError> {
        self.row(
            "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
            &[broadcast_id],
        )
        .await?;
        let created_at = now();
        let options = input
            .options
            .iter()
            .enumerate()
            .map(|(index, label)| {
                json!({
                    "id": format!("option_{}", index + 1),
                    "label": label.trim(),
                    "votes": 0,
                    "percent": 0
                })
            })
            .collect::<Vec<_>>();
        sqlx::query(
            "INSERT INTO obs_engagement_polls
            (id, broadcast_id, poll_kind, question, options_json, status, duration_seconds,
             opened_at, closed_at, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, 'open', ?, ?, NULL, ?, ?)",
        )
        .bind(format!("poll_{}", short_id()))
        .bind(broadcast_id)
        .bind(input.poll_kind)
        .bind(input.question.trim())
        .bind(json!(options).to_string())
        .bind(input.duration_seconds)
        .bind(&created_at)
        .bind(&created_at)
        .bind(&created_at)
        .execute(&self.pool)
        .await?;
        self.add_event(Some(broadcast_id), "engagement_poll", "Live poll opened")
            .await?;
        self.dashboard().await
    }

    pub async fn vote_engagement_poll(
        &self,
        poll_id: &str,
        input: EngagementVoteInput,
    ) -> Result<Value, ObsStoreError> {
        let poll = self
            .row(
                "SELECT * FROM obs_engagement_polls WHERE id = ?",
                &[poll_id],
            )
            .await?;
        if text(&poll, "status") != "open" {
            return Err(ObsStoreError::Invalid("poll is not open".to_string()));
        }
        let options = poll_options(&poll);
        if !options
            .iter()
            .any(|option| text(option, "id") == input.option_id)
        {
            return Err(ObsStoreError::Invalid(
                "poll option is not part of this poll".to_string(),
            ));
        }
        let broadcast_id = text(&poll, "broadcast_id");
        let created_at = now();
        sqlx::query(
            "INSERT INTO obs_engagement_votes (id, poll_id, broadcast_id, option_id, voter_id, created_at)
            VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(format!("vote_{}", short_id()))
        .bind(poll_id)
        .bind(&broadcast_id)
        .bind(input.option_id)
        .bind(input.voter_id)
        .bind(&created_at)
        .execute(&self.pool)
        .await?;
        self.add_event(
            Some(&broadcast_id),
            "engagement_vote",
            "Live poll vote received",
        )
        .await?;
        self.dashboard().await
    }

    pub async fn close_engagement_poll(&self, poll_id: &str) -> Result<Value, ObsStoreError> {
        let poll = self
            .row(
                "SELECT * FROM obs_engagement_polls WHERE id = ?",
                &[poll_id],
            )
            .await?;
        let broadcast_id = text(&poll, "broadcast_id");
        let updated_at = now();
        sqlx::query(
            "UPDATE obs_engagement_polls SET status = 'closed', closed_at = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&updated_at)
        .bind(&updated_at)
        .bind(poll_id)
        .execute(&self.pool)
        .await?;
        self.add_event(Some(&broadcast_id), "engagement_poll", "Live poll closed")
            .await?;
        self.dashboard().await
    }

    pub async fn create_engagement_alert(
        &self,
        broadcast_id: &str,
        input: EngagementAlertInput,
    ) -> Result<Value, ObsStoreError> {
        self.row(
            "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
            &[broadcast_id],
        )
        .await?;
        let created_at = now();
        sqlx::query(
            "INSERT INTO obs_alert_events
            (id, broadcast_id, alert_kind, title, message, severity, source_user, amount_cents,
             status, metadata_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'ready', ?, ?, ?)",
        )
        .bind(format!("alert_{}", short_id()))
        .bind(broadcast_id)
        .bind(input.alert_kind)
        .bind(input.title.trim())
        .bind(input.message.trim())
        .bind(input.severity.unwrap_or_else(|| "info".to_string()))
        .bind(input.source_user)
        .bind(input.amount_cents.unwrap_or_default())
        .bind(input.metadata_json.unwrap_or_else(|| json!({})).to_string())
        .bind(&created_at)
        .bind(&created_at)
        .execute(&self.pool)
        .await?;
        self.add_event(Some(broadcast_id), "engagement_alert", "Live alert queued")
            .await?;
        self.dashboard().await
    }

    pub(super) async fn engagement_state(
        &self,
        broadcast_id: &str,
    ) -> Result<Value, ObsStoreError> {
        let schedule = self
            .list(
                "SELECT * FROM obs_schedule_slots WHERE broadcast_id = ? ORDER BY starts_at ASC LIMIT 8",
                &[broadcast_id],
            )
            .await?;
        let polls = self
            .list(
                "SELECT * FROM obs_engagement_polls WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 8",
                &[broadcast_id],
            )
            .await?;
        let mut enriched_polls = Vec::new();
        for poll in polls {
            enriched_polls.push(self.enrich_poll(poll).await?);
        }
        let alerts = self
            .list(
                "SELECT * FROM obs_alert_events WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 12",
                &[broadcast_id],
            )
            .await?;
        let active_poll = enriched_polls
            .iter()
            .find(|poll| text(poll, "status") == "open")
            .cloned()
            .unwrap_or(Value::Null);
        let next_slot = schedule
            .iter()
            .find(|slot| {
                text(slot, "status") == "scheduled" || text(slot, "status") == "rescheduled"
            })
            .cloned()
            .unwrap_or(Value::Null);
        Ok(json!({
            "schedule_json": schedule,
            "next_slot": next_slot,
            "polls_json": enriched_polls,
            "active_poll": active_poll,
            "alerts_json": alerts,
            "ready_alert_count": alerts.iter().filter(|alert| text(alert, "status") == "ready").count(),
            "has_active_poll": !active_poll.is_null(),
            "schedule_count": schedule.len()
        }))
    }

    async fn enrich_poll(&self, mut poll: Value) -> Result<Value, ObsStoreError> {
        let poll_id = text(&poll, "id");
        let votes = self
            .list(
                "SELECT * FROM obs_engagement_votes WHERE poll_id = ? ORDER BY created_at ASC",
                &[&poll_id],
            )
            .await?;
        let options = poll_options(&poll);
        let total_votes = votes.len() as i64;
        let enriched_options = options
            .into_iter()
            .map(|mut option| {
                let option_id = text(&option, "id");
                let count = votes
                    .iter()
                    .filter(|vote| text(vote, "option_id") == option_id)
                    .count() as i64;
                let percent = if total_votes > 0 {
                    ((count as f64 / total_votes as f64) * 100.0).round() as i64
                } else {
                    0
                };
                if let Some(object) = option.as_object_mut() {
                    object.insert("votes".to_string(), json!(count));
                    object.insert("percent".to_string(), json!(percent));
                }
                option
            })
            .collect::<Vec<_>>();
        let is_prediction = text(&poll, "poll_kind") == "prediction";
        if let Some(object) = poll.as_object_mut() {
            object.insert("options_json".to_string(), json!(enriched_options));
            object.insert("votes_json".to_string(), json!(votes));
            object.insert("total_votes".to_string(), json!(total_votes));
            object.insert("is_prediction".to_string(), json!(is_prediction));
        }
        Ok(poll)
    }
}

fn poll_options(poll: &Value) -> Vec<Value> {
    poll.get("options_json")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}
