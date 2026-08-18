use super::*;

pub(crate) fn monetized_access_policy(access_policy: &str) -> bool {
    matches!(
        access_policy,
        "subscription" | "purchase" | "subscription_or_purchase"
    )
}

pub(crate) fn parse_optional_future_timestamp(value: Option<&str>) -> AppResult<Option<String>> {
    let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let parsed = chrono::DateTime::parse_from_rfc3339(raw)
        .map_err(|_| {
            AppError::BadRequest("expiresAt must be a valid RFC3339 timestamp".to_string())
        })?
        .with_timezone(&Utc);
    if parsed <= Utc::now() {
        return Err(AppError::BadRequest(
            "expiresAt must be in the future".to_string(),
        ));
    }
    Ok(Some(parsed.to_rfc3339()))
}
