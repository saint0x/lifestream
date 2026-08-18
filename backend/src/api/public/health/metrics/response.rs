use super::*;

pub(super) fn write_counter(body: &mut String, name: &str, value: impl std::fmt::Display) {
    let _ = writeln!(body, "# TYPE {name} counter");
    let _ = writeln!(body, "{name} {value}");
}

pub(super) fn write_gauge(body: &mut String, name: &str, value: impl std::fmt::Display) {
    let _ = writeln!(body, "# TYPE {name} gauge");
    let _ = writeln!(body, "{name} {value}");
}

pub(super) fn write_optional_gauge(
    body: &mut String,
    name: &str,
    value: Option<impl std::fmt::Display>,
) {
    if let Some(value) = value {
        write_gauge(body, name, value);
    }
}

pub(super) fn finish_metrics_response(body: String) -> AppResult<Response> {
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )
        .body(Body::from(body))
        .map_err(|_| AppError::BadRequest("failed to build metrics response".to_string()))
}
