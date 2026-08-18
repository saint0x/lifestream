use super::*;

pub(crate) async fn request_context_middleware(
    State(state): State<SharedState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    state.metrics.begin_request();

    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    if let Ok(value) = HeaderValue::from_str(&request_id) {
        request.headers_mut().insert("x-request-id", value);
    }

    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert("x-request-id", value);
    }
    state
        .metrics
        .finish_request(response.status().as_u16())
        .await;
    response
}

pub(crate) fn validate_request_origin(state: &SharedState, headers: &HeaderMap) -> AppResult<()> {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return Ok(());
    };
    if state.allows_origin(origin) {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

pub(crate) async fn enforce_rate_limit(
    state: &SharedState,
    key: &str,
    limit: usize,
    window: Duration,
) -> AppResult<()> {
    state
        .rate_limits
        .check(key, limit, window)
        .await
        .map_err(|_| {
            state.metrics.increment_rate_limit();
            AppError::RateLimited
        })
}
