use super::*;

pub(super) fn router(state: SharedState) -> Router {
    Router::new()
        .merge(admin_ops::routes())
        .merge(public::routes())
        .merge(me::routes())
        .merge(creator::routes())
        .merge(collaboration::routes())
        .merge(ingest::routes())
        .merge(playback::routes())
        .merge(realtime::routes())
        .merge(uploads::routes())
        .merge(upload_jobs::routes())
        .route("/api/v1/media/*path", get(serve_media_file))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            app_request::request_context_middleware,
        ))
        .with_state(state.clone())
        .layer(build_cors_layer(state.as_ref()))
        .layer(TraceLayer::new_for_http())
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
            header::HeaderName::from_static("x-request-id"),
        ])
        .allow_origin(state.cors_allowed_origins.clone())
        .expose_headers([header::HeaderName::from_static("x-request-id")])
}
