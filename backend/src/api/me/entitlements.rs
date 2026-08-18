use super::*;

pub(crate) async fn get_my_membership_entitlement(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(creator_id): Path<String>,
) -> AppResult<Json<CreatorMembership>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_creator_membership(&state.pool, &identity.user_id, &creator_id).await?,
    ))
}

pub(crate) async fn reconcile_my_membership_entitlement(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(creator_id): Path<String>,
) -> AppResult<Json<CreatorMembershipReconciliationReport>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let membership = fetch_creator_membership(&state.pool, &identity.user_id, &creator_id).await?;
    if membership.creator_id != creator_id {
        return Err(AppError::NotFound);
    }
    Ok(Json(
        reconcile_single_membership_entitlement(state, &identity.user_id, &creator_id).await?,
    ))
}

pub(crate) async fn get_my_purchase_entitlement(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(purchase_id): Path<String>,
) -> AppResult<Json<ContentPurchase>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let purchase = fetch_content_purchase_by_id(&state.pool, &purchase_id).await?;
    if purchase_belongs_to_user(&state.pool, &identity.user_id, &purchase.id).await? {
        return Ok(Json(purchase));
    }
    Err(AppError::NotFound)
}

pub(crate) async fn reconcile_my_purchase_entitlement(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(purchase_id): Path<String>,
) -> AppResult<Json<ContentPurchaseReconciliationReport>> {
    let identity = require_identity(&state.pool, &headers).await?;
    if !purchase_belongs_to_user(&state.pool, &identity.user_id, &purchase_id).await? {
        return Err(AppError::NotFound);
    }
    Ok(Json(
        reconcile_single_purchase_entitlement(state, &identity.user_id, &purchase_id).await?,
    ))
}
