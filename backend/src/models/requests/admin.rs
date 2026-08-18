use super::*;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationDeliveryQuery {
    pub state: Option<String>,
    pub creator_id: Option<Id>,
    pub limit: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminMediaJobQuery {
    pub status: Option<String>,
    pub creator_id: Option<Id>,
    pub limit: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminLiveIngestQuery {
    pub creator_id: Option<Id>,
    pub status: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminLiveIngestOverviewQuery {
    pub creator_id: Option<Id>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminPlaybackSessionQuery {
    pub creator_id: Option<Id>,
    pub content_id: Option<Id>,
    pub state: Option<String>,
    pub limit: Option<i64>,
}
