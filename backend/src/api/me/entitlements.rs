use super::*;

pub(crate) async fn get_my_membership_entitlement(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(creator_id): Path<String>,
) -> AppResult<Json<CreatorMembership>> {
    let identity = require_identity(&state.db, &headers).await?;
    if state.database_kind == crate::config::DatabaseKind::Postgres {
        let entitlements = super::state::build_postgres_viewer_app_state(
            &state.db,
            &identity.user_id,
            &identity.session_id,
        )
        .await?
        .entitlements;
        return Ok(Json(
            entitlements
                .memberships
                .into_iter()
                .find(|membership| membership.creator_id == creator_id)
                .ok_or(AppError::NotFound)?,
        ));
    }
    Ok(Json(
        fetch_creator_membership(
            state.db.try_sqlite_adapter()?,
            &identity.user_id,
            &creator_id,
        )
        .await?,
    ))
}

pub(crate) async fn reconcile_my_membership_entitlement(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(creator_id): Path<String>,
) -> AppResult<Json<CreatorMembershipReconciliationReport>> {
    let identity = require_identity(&state.db, &headers).await?;
    if state.database_kind == crate::config::DatabaseKind::Postgres {
        return Err(AppError::NotFound);
    }
    let membership = fetch_creator_membership(
        state.db.try_sqlite_adapter()?,
        &identity.user_id,
        &creator_id,
    )
    .await?;
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
    let identity = require_identity(&state.db, &headers).await?;
    if state.database_kind == crate::config::DatabaseKind::Postgres {
        let entitlements = super::state::build_postgres_viewer_app_state(
            &state.db,
            &identity.user_id,
            &identity.session_id,
        )
        .await?
        .entitlements;
        return Ok(Json(
            entitlements
                .purchases
                .into_iter()
                .find(|purchase| purchase.id == purchase_id)
                .ok_or(AppError::NotFound)?,
        ));
    }
    let purchase =
        fetch_content_purchase_by_id(state.db.try_sqlite_adapter()?, &purchase_id).await?;
    if purchase_belongs_to_user(
        state.db.try_sqlite_adapter()?,
        &identity.user_id,
        &purchase.id,
    )
    .await?
    {
        return Ok(Json(purchase));
    }
    Err(AppError::NotFound)
}

pub(crate) async fn reconcile_my_purchase_entitlement(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(purchase_id): Path<String>,
) -> AppResult<Json<ContentPurchaseReconciliationReport>> {
    let identity = require_identity(&state.db, &headers).await?;
    if state.database_kind == crate::config::DatabaseKind::Postgres {
        return Err(AppError::NotFound);
    }
    if !purchase_belongs_to_user(
        state.db.try_sqlite_adapter()?,
        &identity.user_id,
        &purchase_id,
    )
    .await?
    {
        return Err(AppError::NotFound);
    }
    Ok(Json(
        reconcile_single_purchase_entitlement(state, &identity.user_id, &purchase_id).await?,
    ))
}
