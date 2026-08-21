use super::*;
use crate::api::dashboard::{fetch_creator_app_state, fetch_creator_dashboard_shell};
use crate::api::discovery::{
    fetch_categories_for_live_streams, fetch_user, fetch_viewer_app_state,
};

const BOOTSTRAP_RESPONSE_CACHE_TTL: Duration = Duration::from_millis(2_000);

async fn build_home_response(state: &SharedState, headers: &HeaderMap) -> AppResult<HomeResponse> {
    let (trending_series, trending_films, featured_live, maybe_identity) = tokio::try_join!(
        fetch_series(&state.pool, Some("WHERE trending = 1"), Some(6)),
        fetch_films(&state.pool, Some("WHERE trending = 1"), Some(6)),
        fetch_live_streams(&state.pool, None),
        optional_identity(&state.pool, headers),
    )?;
    let categories = fetch_categories_for_live_streams(&state.pool, &featured_live).await?;
    let continue_watching = match maybe_identity {
        Some(identity) => fetch_continue_watching_entries(&state.pool, &identity.user_id).await?,
        None => Vec::new(),
    };

    Ok(HomeResponse {
        trending_series,
        trending_films,
        featured_live,
        categories,
        continue_watching,
    })
}

pub(crate) async fn home(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<HomeResponse>> {
    Ok(Json(build_home_response(&state, &headers).await?))
}

pub(crate) async fn bootstrap(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let identity = optional_identity(&state.pool, &headers).await?;
    let bootstrap_cache_key = identity
        .as_ref()
        .map(|identity| format!("session:{}", identity.session_id))
        .unwrap_or_else(|| "anon".to_string());
    if let Some(cached) = state
        .bootstrap_cache
        .get(&bootstrap_cache_key, BOOTSTRAP_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok(([(header::CONTENT_TYPE, "application/json")], Body::from(cached)).into_response());
    }
    let _coalesced = state
        .request_coalescer
        .acquire(&format!("bootstrap:{bootstrap_cache_key}"))
        .await;
    if let Some(cached) = state
        .bootstrap_cache
        .get(&bootstrap_cache_key, BOOTSTRAP_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok(([(header::CONTENT_TYPE, "application/json")], Body::from(cached)).into_response());
    }
    let home = build_home_response(&state, &headers).await?;
    let me = match identity.as_ref() {
        Some(identity) => Some(fetch_user(&state.pool, &identity.user_id).await?),
        None => None,
    };
    let viewer = match identity.as_ref() {
        Some(identity) => Some(
            fetch_viewer_app_state(&state.pool, &identity.user_id, &identity.session_id).await?,
        ),
        None => None,
    };
    let (creator, creator_state) = match identity.as_ref() {
        Some(identity) if identity.creator_id.is_some() => {
            let creator_id = identity.require_creator_scope()?;
            (
                Some(fetch_creator_dashboard_shell(&state.pool, creator_id).await?),
                Some(
                    fetch_creator_app_state(
                        &state,
                        identity,
                        &CreatorContentQuery {
                            kind: None,
                            status: None,
                            q: None,
                            sort: None,
                        },
                    )
                    .await?,
                ),
            )
        }
        _ => (None, None),
    };
    let response = serde_json::json!({
        "home": home,
        "me": me,
        "viewer": viewer,
        "creator": creator,
        "creatorState": creator_state
    });
    let response_body = Bytes::from(serde_json::to_vec(&response)?);
    state
        .bootstrap_cache
        .put(&bootstrap_cache_key, response_body.clone())
        .await;
    Ok(([(header::CONTENT_TYPE, "application/json")], Body::from(response_body)).into_response())
}
