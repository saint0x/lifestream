use super::*;
use crate::api::dashboard::fetch_creator_dashboard_shell;
use crate::api::discovery::fetch_categories_for_live_streams;

pub(crate) async fn home(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<HomeResponse>> {
    let (trending_series, trending_films, featured_live, maybe_identity) = tokio::try_join!(
        fetch_series(&state.pool, Some("WHERE trending = 1"), Some(6)),
        fetch_films(&state.pool, Some("WHERE trending = 1"), Some(6)),
        fetch_live_streams(&state.pool, None),
        optional_identity(&state.pool, &headers),
    )?;
    let categories = fetch_categories_for_live_streams(&state.pool, &featured_live).await?;
    let continue_watching = match maybe_identity {
        Some(identity) => fetch_continue_watching_entries(&state.pool, &identity.user_id).await?,
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
    let identity = optional_identity(&state.pool, &headers).await?;
    let creator = match identity.as_ref() {
        Some(identity) if identity.creator_id.is_some() => {
            Some(fetch_creator_dashboard_shell(
                &state.pool,
                identity.require_creator_scope()?,
            )
            .await?)
        }
        _ => None,
    };

    Ok(Json(serde_json::json!({
        "home": null,
        "me": null,
        "viewer": null,
        "creator": creator,
        "creatorState": null
    })))
}
