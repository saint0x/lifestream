use super::*;
use crate::models::{
    AdvertiserAccountResponse, CreateAdvertiserInviteRequest, UpdateAdvertiserCompanyRequest,
    UpdateAdvertiserSeatRequest,
};

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/api/v1/advertiser/me/account",
            get(get_account).patch(update_company),
        )
        .route("/api/v1/advertiser/me/invites", post(create_invite))
        .route(
            "/api/v1/advertiser/me/seats/:user_id",
            patch(update_seat),
        )
}

async fn get_account(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<AdvertiserAccountResponse>> {
    let identity = require_identity(&state.db, &headers).await?;
    Ok(Json(
        state
            .db
            .fetch_advertiser_account_for_auth_user(&identity.user_id)
            .await?,
    ))
}

async fn update_company(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<UpdateAdvertiserCompanyRequest>,
) -> AppResult<Json<AdvertiserAccountResponse>> {
    validate_company_input(&input)?;
    let identity = require_identity(&state.db, &headers).await?;
    let now = Utc::now().to_rfc3339();
    Ok(Json(
        state
            .db
            .update_advertiser_company_for_auth_user(&identity.user_id, &input, &now)
            .await?,
    ))
}

async fn create_invite(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<CreateAdvertiserInviteRequest>,
) -> AppResult<Json<AdvertiserAccountResponse>> {
    let email = input.email.trim().to_lowercase();
    if !email.contains('@') {
        return Err(AppError::BadRequest("invite email is invalid".to_string()));
    }
    if input
        .name
        .as_deref()
        .is_some_and(|name| name.trim().is_empty())
    {
        return Err(AppError::BadRequest(
            "invite name cannot be blank".to_string(),
        ));
    }
    let role = input.role.trim();
    if crate::db::advertiser_permissions_for_role(role).is_none() {
        return Err(AppError::BadRequest(format!(
            "unsupported advertiser role `{role}`"
        )));
    }
    let identity = require_identity(&state.db, &headers).await?;
    let now = Utc::now().to_rfc3339();
    let expires_at = (Utc::now() + ChronoDuration::days(14)).to_rfc3339();
    let invite_id = format!("adv-invite-{}", Uuid::new_v4());
    let token_hash = hash_token(&invite_id);
    Ok(Json(
        state
            .db
            .create_advertiser_invite_for_auth_user(
                &identity.user_id,
                &invite_id,
                &email,
                role,
                &token_hash,
                &now,
                &expires_at,
            )
            .await?,
    ))
}

async fn update_seat(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(input): Json<UpdateAdvertiserSeatRequest>,
) -> AppResult<Json<AdvertiserAccountResponse>> {
    let role = input.role.trim();
    if crate::db::advertiser_permissions_for_role(role).is_none() {
        return Err(AppError::BadRequest(format!(
            "unsupported advertiser role `{role}`"
        )));
    }
    if let Some(status) = input.status.as_deref() {
        if !matches!(status, "active" | "suspended") {
            return Err(AppError::BadRequest(
                "seat status must be active or suspended".to_string(),
            ));
        }
    }
    let identity = require_identity(&state.db, &headers).await?;
    let now = Utc::now().to_rfc3339();
    Ok(Json(
        state
            .db
            .update_advertiser_seat_for_auth_user(
                &identity.user_id,
                &user_id,
                role,
                input.status.as_deref(),
                &now,
            )
            .await?,
    ))
}

fn validate_company_input(input: &UpdateAdvertiserCompanyRequest) -> AppResult<()> {
    if input.name.trim().is_empty() {
        return Err(AppError::BadRequest("company name is required".to_string()));
    }
    if input.industry.trim().is_empty() {
        return Err(AppError::BadRequest("industry is required".to_string()));
    }
    if input.billing_name.trim().is_empty() {
        return Err(AppError::BadRequest("billing name is required".to_string()));
    }
    if !input.billing_email.contains('@') {
        return Err(AppError::BadRequest("billing email is invalid".to_string()));
    }
    Ok(())
}
