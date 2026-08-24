use super::*;
const BOOTSTRAP_RESPONSE_CACHE_TTL: Duration = Duration::from_millis(2_000);

async fn build_home_response(state: &SharedState, headers: &HeaderMap) -> AppResult<HomeResponse> {
    CatalogRepository::new(state).home_response(headers).await
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
    let identity = optional_identity(&state.db, &headers).await?;
    let bootstrap_cache_key = identity
        .as_ref()
        .map(|identity| format!("session:{}", identity.session_id))
        .unwrap_or_else(|| "anon".to_string());
    if let Some(cached) = state
        .bootstrap_cache
        .get(&bootstrap_cache_key, BOOTSTRAP_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok((
            [(header::CONTENT_TYPE, "application/json")],
            Body::from(cached),
        )
            .into_response());
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
        return Ok((
            [(header::CONTENT_TYPE, "application/json")],
            Body::from(cached),
        )
            .into_response());
    }
    let home = build_home_response(&state, &headers).await?;
    let catalog = CatalogRepository::new(&state);
    let me = match identity.as_ref() {
        Some(identity) => Some(catalog.user(&identity.user_id).await?),
        None => None,
    };
    let viewer = match identity.as_ref() {
        Some(identity) if state.database_kind == crate::config::DatabaseKind::Postgres => Some(
            crate::api::me::state::build_postgres_viewer_app_state(
                &state.db,
                &identity.user_id,
                &identity.session_id,
            )
            .await?,
        ),
        Some(identity) => Some(
            catalog
                .viewer_app_state(&identity.user_id, &identity.session_id)
                .await?,
        ),
        _ => None,
    };
    let (creator, creator_state) = match identity.as_ref() {
        Some(identity)
            if identity.creator_id.is_some()
                && state.database_kind != crate::config::DatabaseKind::Postgres =>
        {
            let creator_id = identity.require_creator_scope()?;
            (
                Some(catalog.creator_dashboard_shell(creator_id).await?),
                Some(catalog.creator_app_state(identity).await?),
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
    Ok((
        [(header::CONTENT_TYPE, "application/json")],
        Body::from(response_body),
    )
        .into_response())
}
