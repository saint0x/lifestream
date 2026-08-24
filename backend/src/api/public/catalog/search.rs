use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct SearchQuery {
    pub(crate) q: String,
    pub(crate) limit: Option<i64>,
    pub(crate) offset: Option<i64>,
}

pub(crate) async fn search(
    State(state): State<SharedState>,
    Query(query): Query<SearchQuery>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(
        CatalogRepository::new(&state)
            .search_page(&query.q, query.limit, query.offset)
            .await?,
    ))
}
