use serde_json::Value;

use crate::obs::domain::{
    SponsorCampaignInput, SponsorInventoryInput, SponsorProofInput, SponsorReviewInput,
};

use super::{ObsService, ObsServiceError, ObsServiceResult, require_one_of, require_text};

const CREATIVE_KINDS: &[&str] = &[
    "sponsor_card",
    "lower_third",
    "branded_bumper",
    "pinned_cta",
    "qr_code",
    "promo_code",
];
const PROOF_KINDS: &[&str] = &["marker", "clip", "screenshot", "media_segment"];
const REVIEW_STATUSES: &[&str] = &["pending", "approved", "rejected", "needs_revision"];

impl ObsService {
    pub async fn attach_sponsor_campaign(
        &self,
        broadcast_id: &str,
        input: SponsorCampaignInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        require_text(&input.campaign_id, "campaign_id")?;
        require_text(&input.advertiser, "advertiser")?;
        require_text(&input.title, "title")?;
        Ok(self
            .store
            .attach_sponsor_campaign(broadcast_id, input)
            .await?)
    }

    pub async fn create_sponsor_inventory(
        &self,
        broadcast_id: &str,
        input: SponsorInventoryInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        require_text(&input.campaign_id, "campaign_id")?;
        require_one_of(&input.creative_kind, "creative_kind", CREATIVE_KINDS)?;
        require_text(&input.label, "label")?;
        if input.scheduled_at_seconds < 0.0 {
            return Err(ObsServiceError::Invalid {
                field: "scheduled_at_seconds",
                message: "must not be negative",
            });
        }
        if input.required_duration_seconds <= 0.0 || input.required_duration_seconds > 600.0 {
            return Err(ObsServiceError::Invalid {
                field: "required_duration_seconds",
                message: "is outside the supported range",
            });
        }
        Ok(self
            .store
            .create_sponsor_inventory(broadcast_id, input)
            .await?)
    }

    pub async fn capture_sponsor_proof(
        &self,
        inventory_id: &str,
        input: SponsorProofInput,
    ) -> ObsServiceResult<Value> {
        require_text(inventory_id, "inventory_id")?;
        require_one_of(&input.proof_kind, "proof_kind", PROOF_KINDS)?;
        if input.media_time_seconds < 0.0 {
            return Err(ObsServiceError::Invalid {
                field: "media_time_seconds",
                message: "must not be negative",
            });
        }
        Ok(self
            .store
            .capture_sponsor_proof(inventory_id, input)
            .await?)
    }

    pub async fn review_sponsor_proof(
        &self,
        proof_id: &str,
        input: SponsorReviewInput,
    ) -> ObsServiceResult<Value> {
        require_text(proof_id, "proof_id")?;
        require_one_of(&input.status, "status", REVIEW_STATUSES)?;
        Ok(self.store.review_sponsor_proof(proof_id, input).await?)
    }
}
