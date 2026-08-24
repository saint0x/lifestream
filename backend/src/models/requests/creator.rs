use super::*;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCreatorLiveSettingsRequest {
    pub subscriber_only: Option<bool>,
    pub slow_mode_seconds: Option<i64>,
    pub auto_mod_level: Option<String>,
    pub notify_followers_default: Option<bool>,
    pub delivery_class: Option<String>,
    pub active_scene_id: Option<String>,
    pub scenes: Option<Vec<CreatorScene>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCreatorOperationalStateRequest {
    pub legal_name: Option<String>,
    pub support_email: Option<String>,
    pub business_type: Option<String>,
    pub payout_country: Option<String>,
    pub payout_provider: Option<String>,
    pub submit_onboarding: Option<bool>,
    pub submit_identity_verification: Option<bool>,
    pub submit_tax_profile: Option<bool>,
    pub submit_payout_method: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCreatorSubscriberTierRequest {
    pub tier_name: String,
    pub rank: Option<i64>,
    pub monthly_price: f64,
    pub accent_color: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCreatorSubscriberTierRequest {
    pub tier_name: Option<String>,
    pub rank: Option<i64>,
    pub monthly_price: Option<f64>,
    pub accent_color: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCreatorModeratorRequest {
    pub user_id: Id,
    pub role: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCreatorEnforcementActionRequest {
    pub scope: String,
    pub reason: String,
    pub expires_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseCreatorEnforcementActionRequest {
    pub resolution_note: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCreatorSeriesRequest {
    pub slug: String,
    pub title: String,
    pub synopsis: String,
    pub rating: String,
    pub genres: Vec<String>,
    pub hero_color: String,
    pub poster_url: String,
    pub backdrop_url: String,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCreatorSeriesRequest {
    pub title: Option<String>,
    pub synopsis: Option<String>,
    pub rating: Option<String>,
    pub genres: Option<Vec<String>>,
    pub hero_color: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub status: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCreditInput {
    pub person_id: Option<Id>,
    pub person_slug: Option<String>,
    pub role: String,
    pub character: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectCreditsRequest {
    pub credits: Vec<ProjectCreditInput>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitAdOfferReviewRequest {
    pub submission_url: String,
    pub notes: Option<String>,
}
