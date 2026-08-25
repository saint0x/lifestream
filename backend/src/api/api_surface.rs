use super::*;
use tower_http::compression::CompressionLayer;

pub(super) fn router(state: SharedState) -> Router {
    Router::new()
        .merge(public::routes())
        .merge(me::routes())
        .merge(advertiser::routes())
        .merge(creator::routes())
        .merge(creator_api::routes())
        .merge(collabs::routes())
        .merge(ingest::routes())
        .merge(media::jobs::routes())
        .merge(playback::routes())
        .merge(realtime::routes())
        .merge(uploads::routes())
        .merge(admin_ops::routes())
        .route("/api/v1/media/*path", get(serve_media_file))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            app_request::request_context_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            admin_api_gate,
        ))
        .with_state(state.clone())
        .layer(build_cors_layer(state.as_ref()))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
}

async fn admin_api_gate(
    State(state): State<SharedState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if request.uri().path().starts_with("/api/v1/admin/") && !state.admin_api_enabled {
        return StatusCode::NOT_FOUND.into_response();
    }
    next.run(request).await
}

fn build_cors_layer(state: &AppState) -> CorsLayer {
    CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::ACCEPT,
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ORIGIN,
            header::HeaderName::from_static("x-ingest-token"),
            header::HeaderName::from_static("x-request-id"),
            header::HeaderName::from_static("x-upload-token"),
            header::HeaderName::from_static("x-vanta-api-key"),
        ])
        .allow_origin(state.cors_allowed_origins.clone())
        .allow_credentials(true)
        .expose_headers([header::HeaderName::from_static("x-request-id")])
}
