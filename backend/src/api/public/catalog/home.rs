use super::*;

pub(crate) async fn home(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<HomeResponse>> {
    let trending_series = fetch_series(&state.pool, Some("WHERE trending = 1"), Some(6)).await?;
    let trending_films = fetch_films(&state.pool, Some("WHERE trending = 1"), Some(6)).await?;
    let featured_live = fetch_live_streams(&state.pool, None).await?;
    let categories = fetch_categories(&state.pool).await?;
    let maybe_identity = optional_identity(&state.pool, &headers).await?;
    let continue_watching = match maybe_identity {
        Some(identity) => {
            fetch_user(&state.pool, &identity.user_id)
                .await?
                .continue_watching
        }
        None => Vec::new(),
    };

    Ok(Json(HomeResponse {
        trending_series,
        trending_films,
        featured_live,
        categories,
        continue_watching,
    }))
}

pub(crate) async fn bootstrap(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<serde_json::Value>> {
    let home = home(State(state.clone()), headers.clone()).await?.0;
    let identity = optional_identity(&state.pool, &headers).await?;
    let me = match identity.as_ref() {
        Some(identity) => Some(fetch_user(&state.pool, &identity.user_id).await?),
        None => None,
    };
    let creator = match identity.as_ref() {
        Some(identity) if identity.creator_id.is_some() => {
            Some(creator_dashboard_payload(&state.pool, identity).await?)
        }
        _ => None,
    };

    Ok(Json(serde_json::json!({
        "home": home,
        "me": me,
        "viewer": null,
        "creator": creator,
        "creatorState": null
    })))
}
