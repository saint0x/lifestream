use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsPoint {
    pub date: String,
    pub viewers: i64,
    pub watch_minutes: i64,
    pub revenue: f64,
    pub new_followers: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficSource {
    pub source: String,
    pub sessions: i64,
    pub share: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorAttentionScore {
    pub algorithm_version: String,
    pub qualified_viewers: i64,
    pub verified_viewer_score: f64,
    pub creator_attention_value: f64,
    pub baseline_value_per_qualified_viewer: f64,
    pub average_watch_minutes: f64,
    pub attention_multiplier: f64,
    pub engagement_multiplier: f64,
    pub retention_multiplier: f64,
    pub audience_quality_multiplier: f64,
    pub data_confidence_multiplier: f64,
    pub qualified_viewer_rate: f64,
    pub returning_viewer_rate: f64,
    pub measured_sessions: i64,
    pub measured_viewers: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopContent {
    pub id: Id,
    pub title: String,
    pub kind: String,
    pub views: i64,
    pub watch_hours: i64,
    pub trend: f64,
    pub thumbnail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevenueEntry {
    pub id: Id,
    pub date: String,
    pub source: String,
    pub description: String,
    pub amount: f64,
}
