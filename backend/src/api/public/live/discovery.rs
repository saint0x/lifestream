use super::*;

pub(crate) async fn list_live_streams(
    State(state): State<SharedState>,
) -> AppResult<Json<Vec<LiveStream>>> {
    Ok(Json(fetch_live_streams(&state.pool, None).await?))
}

pub(crate) async fn get_live_stream(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> AppResult<Json<LiveStream>> {
    Ok(Json(fetch_live_stream_by_slug(&state.pool, &slug).await?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveDiscoveryQuery {
    category: Option<String>,
    sort: Option<String>,
    limit: Option<i64>,
}

pub(crate) async fn get_live_discovery(
    State(state): State<SharedState>,
    Query(query): Query<LiveDiscoveryQuery>,
) -> AppResult<Json<LiveDiscoveryResponse>> {
    let categories = fetch_categories(&state.pool).await?;
    let active_category = match query.category.as_deref() {
        Some("all") | None => None,
        Some(category_name) => {
            if categories.iter().any(|item| item.name == category_name) {
                Some(category_name.to_string())
            } else {
                return Err(AppError::BadRequest(
                    "unknown live category filter".to_string(),
                ));
            }
        }
    };
    let active_sort = match query.sort.as_deref().unwrap_or("viewers") {
        "viewers" | "newest" => query.sort.unwrap_or_else(|| "viewers".to_string()),
        _ => {
            return Err(AppError::BadRequest(
                "sort must be either 'viewers' or 'newest'".to_string(),
            ));
        }
    };

    let limit = query.limit.unwrap_or(200).clamp(1, 500) as usize;
    let mut streams = fetch_live_streams(&state.pool, None).await?;
    let total_viewers = streams.iter().map(|stream| stream.viewers).sum();
    let total_channels = streams.len() as i64;
    if let Some(category_name) = active_category.as_deref() {
        streams.retain(|stream| stream.category == category_name);
    }
    sort_live_streams(&mut streams, &active_sort);
    if streams.len() > limit {
        streams.truncate(limit);
    }

    Ok(Json(LiveDiscoveryResponse {
        streams,
        categories,
        total_viewers,
        total_channels,
        active_category,
        active_sort,
    }))
}
