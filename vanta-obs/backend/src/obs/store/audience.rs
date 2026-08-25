use serde_json::{Value, json};

use crate::obs::domain::{AudienceTelemetryInput, RaidRedirectInput};

use super::{
    ObsStore, ObsStoreError,
    row::{now, num, short_id, text},
};

impl ObsStore {
    pub async fn ingest_audience_telemetry(
        &self,
        broadcast_id: &str,
        input: AudienceTelemetryInput,
    ) -> Result<Value, ObsStoreError> {
        self.row(
            "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
            &[broadcast_id],
        )
        .await?;
        let created_at = now();
        let discovery_source = input
            .discovery_source
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "direct".to_string());
        let discovery_score = input.discovery_score.unwrap_or_else(|| {
            if input.viewer_count >= 1000 {
                92.0
            } else if input.viewer_count >= 250 {
                74.0
            } else {
                52.0
            }
        });
        let details = input.details_json.unwrap_or_else(|| json!({}));
        let chat_messages = input.chat_messages_per_minute.unwrap_or_default();
        let tips = input.tips_cents.unwrap_or_default();
        let subscriptions = input.subscriptions.unwrap_or_default();
        let revenue = input.revenue_cents.unwrap_or(tips);
        sqlx::query(
            "INSERT INTO obs_audience_snapshots
            (id, broadcast_id, viewer_count, chat_messages_per_minute, tips_cents, subscriptions,
             revenue_cents, discovery_source, discovery_score, discovery_json, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(format!("audience_{}", short_id()))
        .bind(broadcast_id)
        .bind(input.viewer_count)
        .bind(chat_messages)
        .bind(tips)
        .bind(subscriptions)
        .bind(revenue)
        .bind(discovery_source)
        .bind(discovery_score)
        .bind(details.to_string())
        .bind(&created_at)
        .execute(&self.pool)
        .await?;
        self.add_event(
            Some(broadcast_id),
            "audience_telemetry",
            "Audience telemetry ingested",
        )
        .await?;
        self.dashboard().await
    }

    pub async fn schedule_raid_redirect(
        &self,
        broadcast_id: &str,
        input: RaidRedirectInput,
    ) -> Result<Value, ObsStoreError> {
        self.row(
            "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
            &[broadcast_id],
        )
        .await?;
        let created_at = now();
        let raid_id = format!("raid_{}", short_id());
        let viewer_count = input.viewer_count.unwrap_or_default();
        let execute_after_seconds = input.execute_after_seconds.unwrap_or(30);
        let redirect_url = input
            .redirect_url
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                format!(
                    "https://streamvanta.tv/{}/live",
                    input.target_channel_id.trim()
                )
            });
        let safety = raid_safety_json(
            "outbound",
            input.safety_json.unwrap_or_else(|| json!({})),
            viewer_count,
            execute_after_seconds,
        );
        sqlx::query(
            "INSERT INTO obs_raids
            (id, broadcast_id, direction, target_channel_id, target_channel_name, viewer_count,
             status, execute_after_seconds, redirect_url, safety_json, created_at, updated_at)
            VALUES (?, ?, 'outbound', ?, ?, ?, 'scheduled', ?, ?, ?, ?, ?)",
        )
        .bind(&raid_id)
        .bind(broadcast_id)
        .bind(input.target_channel_id.trim())
        .bind(input.target_channel_name.trim())
        .bind(viewer_count)
        .bind(execute_after_seconds)
        .bind(redirect_url)
        .bind(safety.to_string())
        .bind(&created_at)
        .bind(&created_at)
        .execute(&self.pool)
        .await?;
        self.add_event(
            Some(broadcast_id),
            "raid_redirect",
            "Outbound audience redirect scheduled",
        )
        .await?;
        self.dashboard().await
    }

    pub async fn record_inbound_raid(
        &self,
        broadcast_id: &str,
        input: RaidRedirectInput,
    ) -> Result<Value, ObsStoreError> {
        self.row(
            "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
            &[broadcast_id],
        )
        .await?;
        let created_at = now();
        let raid_id = format!("raid_{}", short_id());
        let viewer_count = input.viewer_count.unwrap_or_default();
        let safety = raid_safety_json(
            "inbound",
            input.safety_json.unwrap_or_else(|| json!({})),
            viewer_count,
            0,
        );
        sqlx::query(
            "INSERT INTO obs_raids
            (id, broadcast_id, direction, target_channel_id, target_channel_name, viewer_count,
             status, execute_after_seconds, redirect_url, safety_json, created_at, updated_at)
            VALUES (?, ?, 'inbound', ?, ?, ?, 'received', 0, ?, ?, ?, ?)",
        )
        .bind(&raid_id)
        .bind(broadcast_id)
        .bind(input.target_channel_id.trim())
        .bind(input.target_channel_name.trim())
        .bind(viewer_count)
        .bind(input.redirect_url.unwrap_or_default())
        .bind(safety.to_string())
        .bind(&created_at)
        .bind(&created_at)
        .execute(&self.pool)
        .await?;
        self.add_event(Some(broadcast_id), "raid_inbound", "Inbound raid recorded")
            .await?;
        self.dashboard().await
    }

    pub(super) async fn audience_state(&self, broadcast_id: &str) -> Result<Value, ObsStoreError> {
        let snapshots = self
            .list(
                "SELECT * FROM obs_audience_snapshots WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 60",
                &[broadcast_id],
            )
            .await?;
        let latest = snapshots.first().cloned().unwrap_or_else(|| {
            json!({
                "viewer_count": 0,
                "chat_messages_per_minute": 0,
                "tips_cents": 0,
                "subscriptions": 0,
                "revenue_cents": 0,
                "discovery_source": "pending",
                "discovery_score": 0.0,
                "discovery_json": {}
            })
        });
        let total = snapshots.len() as f64;
        let peak_viewers = snapshots
            .iter()
            .map(|snapshot| num(snapshot, "viewer_count"))
            .fold(0.0, f64::max) as i64;
        let average_viewers = if total > 0.0 {
            snapshots
                .iter()
                .map(|snapshot| num(snapshot, "viewer_count"))
                .sum::<f64>()
                / total
        } else {
            0.0
        };
        let revenue_cents = snapshots
            .iter()
            .map(|snapshot| num(snapshot, "revenue_cents"))
            .sum::<f64>() as i64;
        let tips_cents = snapshots
            .iter()
            .map(|snapshot| num(snapshot, "tips_cents"))
            .sum::<f64>() as i64;
        let subscriptions = snapshots
            .iter()
            .map(|snapshot| num(snapshot, "subscriptions"))
            .sum::<f64>() as i64;
        let broadcast = self
            .row(
                "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
                &[broadcast_id],
            )
            .await?;
        let runtime = self.runtime(broadcast_id).await?;
        let started_at = text(&runtime, "last_heartbeat_at");
        let raids = self
            .list(
                "SELECT * FROM obs_raids WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 10",
                &[broadcast_id],
            )
            .await?;
        let latest_outbound = raids
            .iter()
            .find(|raid| text(raid, "direction") == "outbound")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let latest_inbound = raids
            .iter()
            .find(|raid| text(raid, "direction") == "inbound")
            .cloned()
            .unwrap_or_else(|| json!({}));
        Ok(json!({
            "latest_snapshot": latest,
            "snapshots_json": snapshots,
            "raids_json": raids,
            "latest_outbound_raid": latest_outbound,
            "latest_inbound_raid": latest_inbound,
            "raid_count": raids.len(),
            "viewer_count": num(&latest, "viewer_count") as i64,
            "chat_messages_per_minute": num(&latest, "chat_messages_per_minute") as i64,
            "uptime_seconds": uptime_seconds(&broadcast, &runtime),
            "peak_viewers": peak_viewers,
            "average_viewers": (average_viewers * 10.0).round() / 10.0,
            "revenue_cents": revenue_cents,
            "tips_cents": tips_cents,
            "subscriptions": subscriptions,
            "discovery_source": text(&latest, "discovery_source"),
            "discovery_score": num(&latest, "discovery_score"),
            "started_at": started_at
        }))
    }
}

fn raid_safety_json(
    direction: &str,
    mut safety: Value,
    viewer_count: i64,
    execute_after_seconds: i64,
) -> Value {
    if safety.as_object().is_none() {
        safety = json!({});
    }
    let Some(object) = safety.as_object_mut() else {
        return safety;
    };
    object.insert("direction".to_string(), json!(direction));
    object.insert("viewer_count_checked".to_string(), json!(viewer_count >= 0));
    object.insert(
        "countdown_checked".to_string(),
        json!(direction == "inbound" || execute_after_seconds >= 5),
    );
    object.insert("moderation_handoff".to_string(), json!(true));
    object.insert(
        "chat_notice_required".to_string(),
        json!(direction == "outbound"),
    );
    safety
}

fn uptime_seconds(broadcast: &Value, runtime: &Value) -> i64 {
    if text(runtime, "stream_state") != "live" {
        return 0;
    }
    let created_at = text(broadcast, "created_at");
    chrono::DateTime::parse_from_rfc3339(&created_at)
        .map(|started| {
            (chrono::Utc::now() - started.with_timezone(&chrono::Utc))
                .num_seconds()
                .max(0)
        })
        .unwrap_or_default()
}
