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
