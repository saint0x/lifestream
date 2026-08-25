use serde_json::{Value, json};

use crate::obs::{
    domain::{
        CueInput, SponsorCampaignInput, SponsorInventoryInput, SponsorProofInput,
        SponsorReviewInput,
    },
    source::contract_for,
    sponsor_media::{SponsorProofMediaInput, capture_proof_media},
};

use super::{
    ObsStore, ObsStoreError,
    row::{now, short_id, text},
};

impl ObsStore {
    pub async fn attach_sponsor_campaign(
        &self,
        broadcast_id: &str,
        input: SponsorCampaignInput,
    ) -> Result<Value, ObsStoreError> {
        self.row(
            "SELECT * FROM obs_broadcast_profiles WHERE id = ?",
            &[broadcast_id],
        )
        .await?;
        let updated_at = now();
        sqlx::query("UPDATE obs_broadcast_profiles SET sponsor_campaign_id = ?, updated_at = ? WHERE id = ?")
            .bind(&input.campaign_id)
            .bind(&updated_at)
            .bind(broadcast_id)
            .execute(&self.pool)
            .await?;
        sqlx::query(
            "INSERT INTO obs_sponsor_campaigns
            (id, broadcast_id, campaign_id, advertiser, title, status, flight_json, claims_json, performance_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, 'attached', ?, ?, ?, ?, ?)",
        )
        .bind(format!("campaign_{}", short_id()))
        .bind(broadcast_id)
        .bind(input.campaign_id)
        .bind(input.advertiser)
        .bind(input.title)
        .bind(input.flight_json.unwrap_or_else(|| json!({"source":"vanta_backend"})).to_string())
        .bind(input.claims_json.unwrap_or_else(|| json!({"required":[],"prohibited":[]})).to_string())
        .bind(input.performance_json.unwrap_or_else(|| json!({"handoff":"pending"})).to_string())
        .bind(&updated_at)
        .bind(&updated_at)
        .execute(&self.pool)
        .await?;
        self.add_event(
            Some(broadcast_id),
            "sponsor_campaign",
            "Sponsor campaign attached",
        )
        .await?;
        self.dashboard().await
    }

    pub async fn create_sponsor_inventory(
        &self,
        broadcast_id: &str,
        input: SponsorInventoryInput,
    ) -> Result<Value, ObsStoreError> {
        let campaign = self
            .row(
                "SELECT * FROM obs_sponsor_campaigns WHERE broadcast_id = ? AND campaign_id = ? ORDER BY created_at DESC LIMIT 1",
                &[broadcast_id, &input.campaign_id],
            )
            .await?;
        let source_kind = input.creative_kind.clone();
        let contract = contract_for(&source_kind).ok_or_else(|| {
            ObsStoreError::Invalid(format!("{source_kind} is not a Vanta source"))
        })?;
        let now = now();
        let source_id = format!("source_{}_{}", source_kind, short_id());
        let cue_kind = cue_kind_for(&source_kind);
        let settings = sponsor_settings(&input, &campaign);
        sqlx::query(
            "INSERT INTO obs_sources
            (id, creator_id, source_kind, display_name, device_id, media_asset_id, browser_url,
             default_settings_json, permission_state, health_state, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, ?, NULL, ?, NULL, ?, 'granted', 'good', ?, ?)",
        )
        .bind(&source_id)
        .bind(&source_kind)
        .bind(&input.label)
        .bind(if contract.requires_media_asset {
            Some(format!("asset_{}", input.campaign_id))
        } else {
            None
        })
        .bind(settings.to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        let cue = self
            .create_cue_for_broadcast(
                broadcast_id,
                CueInput {
                    cue_kind: cue_kind.to_string(),
                    label: input.label.clone(),
                    scheduled_at_seconds: Some(input.scheduled_at_seconds),
                    required_duration_seconds: Some(input.required_duration_seconds),
                    campaign_id: Some(input.campaign_id.clone()),
                    scene_id: input.scene_id.clone(),
                    source_id: Some(source_id.clone()),
                    requirements_json: Some(requirements_json(&input)),
                },
            )
            .await?;
        let inventory_id = format!("inventory_{}", short_id());
        sqlx::query(
            "INSERT INTO obs_sponsor_inventory
            (id, broadcast_id, campaign_id, creative_kind, label, source_kind, source_id, cue_id,
             scheduled_at_seconds, required_duration_seconds, status, requirements_json,
             renderer_json, proof_marker_id, review_status, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'scheduled', ?, ?, NULL, 'pending', ?, ?)",
        )
        .bind(&inventory_id)
        .bind(broadcast_id)
        .bind(&input.campaign_id)
        .bind(&input.creative_kind)
        .bind(&input.label)
        .bind(source_kind)
        .bind(source_id)
        .bind(text(&cue, "id"))
        .bind(input.scheduled_at_seconds)
        .bind(input.required_duration_seconds)
        .bind(requirements_json(&input).to_string())
        .bind(renderer_json(contract.renderer, cue_kind).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.add_event(
            Some(broadcast_id),
            "sponsor_inventory",
            "Sponsor inventory scheduled",
        )
        .await?;
        self.dashboard().await
    }

    pub async fn capture_sponsor_proof(
        &self,
        inventory_id: &str,
        input: SponsorProofInput,
    ) -> Result<Value, ObsStoreError> {
        let inventory = self
            .row(
                "SELECT * FROM obs_sponsor_inventory WHERE id = ?",
                &[inventory_id],
            )
            .await?;
        let broadcast_id = text(&inventory, "broadcast_id");
        let cue_id = text(&inventory, "cue_id");
        let cue = self.trigger_cue(&cue_id).await?;
        let proof_marker = text(&cue, "proof_marker_id");
        let now = now();
        let proof_id = format!("sponsor_proof_{}", short_id());
        let source_media_path = self.latest_proof_media_source(&broadcast_id).await?;
        let media = capture_proof_media(SponsorProofMediaInput {
            broadcast_id: &broadcast_id,
            inventory_id,
            cue_id: &cue_id,
            proof_id: &proof_id,
            proof_marker_id: &proof_marker,
            media_time_seconds: input.media_time_seconds,
            source_media_path: source_media_path.as_deref(),
        })
        .await?;
        self.persist_sponsor_proof_media_asset(
            &broadcast_id,
            &media.asset_id,
            &media.asset_dir,
            &media.manifest_path,
            &media.artifact_json,
            &media.validation_json,
            &now,
        )
        .await?;
        sqlx::query(
            "INSERT INTO obs_sponsor_proofs
            (id, broadcast_id, inventory_id, cue_id, proof_kind, status, media_time_seconds,
             artifact_json, review_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, 'captured', ?, ?, ?, ?, ?)",
        )
        .bind(&proof_id)
        .bind(&broadcast_id)
        .bind(inventory_id)
        .bind(&cue_id)
        .bind(input.proof_kind)
        .bind(input.media_time_seconds)
        .bind(media.artifact_json.to_string())
        .bind(json!({"status":"pending","reviewer_id":null,"notes":null}).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "UPDATE obs_sponsor_inventory SET status = 'proof_captured', proof_marker_id = ?, updated_at = ? WHERE id = ?",
        )
        .bind(proof_marker)
        .bind(&now)
        .bind(inventory_id)
        .execute(&self.pool)
        .await?;
        self.add_event(
            Some(&broadcast_id),
            "sponsor_proof",
            "Sponsor proof captured",
        )
        .await?;
        self.dashboard().await
    }

    async fn latest_proof_media_source(
        &self,
        broadcast_id: &str,
    ) -> Result<Option<String>, ObsStoreError> {
        if let Some(clip) = self
            .row_optional(
                "SELECT * FROM obs_replay_clip_drafts WHERE broadcast_id = ? AND status = 'queued' ORDER BY created_at DESC LIMIT 1",
                &[broadcast_id],
            )
            .await?
        {
            if let Some(path) = clip
                .get("output_path")
                .and_then(Value::as_str)
                .filter(|path| !path.is_empty())
            {
                return Ok(Some(path.to_string()));
            }
        }
        if let Some(recording) = self
            .row_optional(
                "SELECT * FROM obs_recording_jobs WHERE broadcast_id = ? AND status = 'packaging' ORDER BY created_at DESC LIMIT 1",
                &[broadcast_id],
            )
            .await?
            && let Some(segment) = recording
                .get("output_paths_json")
                .and_then(|paths| paths.get("segments"))
                .and_then(Value::as_array)
                .and_then(|segments| {
                    segments
                        .iter()
                        .find(|segment| segment.get("feed").and_then(Value::as_str) == Some("program"))
                })
            && let Some(path) = segment.get("path").and_then(Value::as_str)
        {
            return Ok(Some(path.to_string()));
        }
        Ok(None)
    }

    async fn persist_sponsor_proof_media_asset(
        &self,
        broadcast_id: &str,
        asset_id: &str,
        asset_dir: &std::path::Path,
        manifest_path: &std::path::Path,
        artifact_json: &Value,
        validation_json: &Value,
        now: &str,
    ) -> Result<(), ObsStoreError> {
        sqlx::query(
            "INSERT INTO vanta_media_assets
            (id, creator_id, broadcast_id, asset_kind, status, source_path, asset_path, manifest_path, metadata_json, validation_json, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, 'sponsor_proof', 'ready', ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET status = excluded.status, source_path = excluded.source_path, asset_path = excluded.asset_path, manifest_path = excluded.manifest_path, metadata_json = excluded.metadata_json, validation_json = excluded.validation_json, updated_at = excluded.updated_at",
        )
        .bind(asset_id)
        .bind(broadcast_id)
        .bind(text(artifact_json, "clip_path"))
        .bind(asset_dir.to_string_lossy().to_string())
        .bind(manifest_path.to_string_lossy().to_string())
        .bind(artifact_json.to_string())
        .bind(validation_json.to_string())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn review_sponsor_proof(
        &self,
        proof_id: &str,
        input: SponsorReviewInput,
    ) -> Result<Value, ObsStoreError> {
        let proof = self
            .row("SELECT * FROM obs_sponsor_proofs WHERE id = ?", &[proof_id])
            .await?;
        let broadcast_id = text(&proof, "broadcast_id");
        let inventory_id = text(&proof, "inventory_id");
        let now = now();
        let review = json!({
            "status": input.status,
            "reviewer_id": input.reviewer_id,
            "notes": input.notes
        });
        let status = text(&review, "status");
        sqlx::query(
            "UPDATE obs_sponsor_proofs SET status = 'reviewed', review_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(review.to_string())
        .bind(&now)
        .bind(proof_id)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "UPDATE obs_sponsor_inventory SET review_status = ?, updated_at = ? WHERE id = ?",
        )
        .bind(status)
        .bind(&now)
        .bind(&inventory_id)
        .execute(&self.pool)
        .await?;
        self.add_event(
            Some(&broadcast_id),
            "sponsor_review",
            "Sponsor proof reviewed",
        )
        .await?;
        self.dashboard().await
    }

    pub(super) async fn sponsor_state(&self, broadcast_id: &str) -> Result<Value, ObsStoreError> {
        let campaigns = self
            .list(
                "SELECT * FROM obs_sponsor_campaigns WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 8",
                &[broadcast_id],
            )
            .await?;
        let inventory = self
            .list(
                "SELECT * FROM obs_sponsor_inventory WHERE broadcast_id = ? ORDER BY scheduled_at_seconds ASC LIMIT 24",
                &[broadcast_id],
            )
            .await?;
        let mut proofs = self
            .list(
                "SELECT * FROM obs_sponsor_proofs WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 24",
                &[broadcast_id],
            )
            .await?;
        for proof in &mut proofs {
            let asset_id = proof
                .get("artifact_json")
                .and_then(|artifact| artifact.get("asset_id"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if asset_id.is_empty() {
                continue;
            }
            if let Some(asset) = self
                .row_optional(
                    "SELECT * FROM vanta_media_assets WHERE id = ?",
                    &[&asset_id],
                )
                .await?
                && let Some(object) = proof.as_object_mut()
            {
                object.insert("vanta_asset_json".to_string(), asset);
            }
        }
        let missed = inventory
            .iter()
            .filter(|item| text(item, "status") == "scheduled")
            .filter(|item| item["scheduled_at_seconds"].as_f64().unwrap_or_default() < 60.0)
            .cloned()
            .collect::<Vec<_>>();
        let approved_count = proofs
            .iter()
            .filter(|proof| {
                proof.pointer("/review_json/status").and_then(Value::as_str) == Some("approved")
            })
            .count();
        Ok(json!({
            "campaigns_json": campaigns,
            "inventory_json": inventory,
            "proofs_json": proofs,
            "missed_inventory_json": missed,
            "active_campaign": campaigns.first().cloned().unwrap_or(Value::Null),
            "next_inventory": inventory.first().cloned().unwrap_or(Value::Null),
            "proof_count": proofs.len(),
            "approved_proof_count": approved_count,
            "missed_count": missed.len(),
            "performance_handoff_json": campaigns.first().and_then(|campaign| campaign.get("performance_json")).cloned().unwrap_or_else(|| json!({"handoff":"pending"}))
        }))
    }
}

fn cue_kind_for(source_kind: &str) -> &'static str {
    match source_kind {
        "lower_third" => "lower_third",
        "branded_bumper" => "branded_bumper",
        "pinned_cta" => "pinned_cta",
        "qr_code" => "qr_code",
        "promo_code" => "promo_code",
        _ => "sponsor_read",
    }
}

fn sponsor_settings(input: &SponsorInventoryInput, campaign: &Value) -> Value {
    let mut settings = input
        .settings_json
        .clone()
        .unwrap_or_else(|| json!({}))
        .as_object()
        .cloned()
        .unwrap_or_default();
    settings.insert("campaign_id".to_string(), json!(input.campaign_id));
    settings.insert("advertiser".to_string(), campaign["advertiser"].clone());
    settings.insert("headline".to_string(), json!(input.label));
    settings
        .entry("promo_code".to_string())
        .or_insert_with(|| json!("VANTA20"));
    settings
        .entry("cta_text".to_string())
        .or_insert_with(|| json!("Open sponsor offer"));
    settings
        .entry("target_url".to_string())
        .or_insert_with(|| json!("https://streamvanta.tv/r/sponsor"));
    settings
        .entry("tracking".to_string())
        .or_insert_with(|| json!("streamvanta.tv/r/sponsor"));
    Value::Object(settings)
}

fn requirements_json(input: &SponsorInventoryInput) -> Value {
    json!({
        "required_claims": input.required_claims.clone().unwrap_or_default(),
        "prohibited_claims": input.prohibited_claims.clone().unwrap_or_default(),
        "duration_seconds": input.required_duration_seconds,
        "proof_required": true
    })
}

fn renderer_json(renderer: &str, cue_kind: &str) -> Value {
    json!({
        "renderer": renderer,
        "cue_kind": cue_kind,
        "clock_bound": true,
        "proof_marker": true
    })
}
