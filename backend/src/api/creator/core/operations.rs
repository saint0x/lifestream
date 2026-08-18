use super::*;

pub(super) async fn get_creator_upload_operations(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorUploadOperationsResponse>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_creator_upload_operations_response(&state.pool, creator_id).await?,
    ))
}

pub(super) async fn get_creator_operational_state(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorOperationalState>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let profile = fetch_creator_profile(&state.pool, creator_id).await?;
    Ok(Json(
        fetch_creator_operational_state(&state.pool, &profile).await?,
    ))
}

pub(super) async fn update_creator_operational_state(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<UpdateCreatorOperationalStateRequest>,
) -> AppResult<Json<CreatorOperationalState>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let profile = fetch_creator_profile(&state.pool, creator_id).await?;
    let current = fetch_creator_operational_state(&state.pool, &profile).await?;
    let now = Utc::now().to_rfc3339();

    let legal_name = input
        .legal_name
        .unwrap_or(current.legal_name)
        .trim()
        .to_string();
    let support_email = input
        .support_email
        .unwrap_or(current.support_email)
        .trim()
        .to_string();
    let business_type = input
        .business_type
        .unwrap_or(current.business_type)
        .trim()
        .to_string();
    let payout_country = input
        .payout_country
        .unwrap_or(current.payout_country)
        .trim()
        .to_ascii_uppercase();
    let payout_provider = input
        .payout_provider
        .unwrap_or(current.payout_provider)
        .trim()
        .to_string();

    if legal_name.is_empty()
        || support_email.is_empty()
        || business_type.is_empty()
        || payout_country.is_empty()
        || payout_provider.is_empty()
    {
        return Err(AppError::BadRequest(
            "legalName, supportEmail, businessType, payoutCountry, and payoutProvider must be non-empty"
                .to_string(),
        ));
    }
    if !support_email.contains('@') {
        return Err(AppError::BadRequest(
            "supportEmail must be a valid email address".to_string(),
        ));
    }
    if payout_country.len() < 2 || payout_country.len() > 3 {
        return Err(AppError::BadRequest(
            "payoutCountry must be a 2-3 character country code".to_string(),
        ));
    }

    let onboarding_status = transition_creator_operational_status(
        &current.onboarding_status,
        input.submit_onboarding.unwrap_or(false),
        "approved",
        "blocked",
    )?;
    let identity_status = transition_creator_operational_status(
        &current.identity_status,
        input.submit_identity_verification.unwrap_or(false),
        "verified",
        "rejected",
    )?;
    let tax_status = transition_creator_operational_status(
        &current.tax_status,
        input.submit_tax_profile.unwrap_or(false),
        "verified",
        "rejected",
    )?;
    let payout_status = transition_creator_operational_status(
        &current.payout_status,
        input.submit_payout_method.unwrap_or(false),
        "active",
        "disabled",
    )?;

    sqlx::query(
        r#"
        UPDATE creator_operational_state
        SET legal_name = ?, support_email = ?, business_type = ?, payout_country = ?, payout_provider = ?,
            onboarding_status = ?, identity_status = ?, tax_status = ?, payout_status = ?, updated_at = ?
        WHERE creator_id = ?
        "#,
    )
    .bind(&legal_name)
    .bind(&support_email)
    .bind(&business_type)
    .bind(&payout_country)
    .bind(&payout_provider)
    .bind(&onboarding_status)
    .bind(&identity_status)
    .bind(&tax_status)
    .bind(&payout_status)
    .bind(&now)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;

    let refreshed_profile = fetch_creator_profile(&state.pool, creator_id).await?;
    Ok(Json(
        fetch_creator_operational_state(&state.pool, &refreshed_profile).await?,
    ))
}
