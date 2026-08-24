use super::*;

pub(super) async fn list_analytics(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<AnalyticsPoint>>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_analytics(state.db.sqlite_adapter(), creator_id).await?,
    ))
}

pub(super) async fn list_revenue(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<RevenueEntry>>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_revenue_entries(state.db.sqlite_adapter(), creator_id).await?,
    ))
}

pub(super) async fn list_notifications(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<CreatorNotification>>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_notifications_rows(state.db.sqlite_adapter(), creator_id).await?,
    ))
}

pub(super) async fn mark_creator_notification_read(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(notification_id): Path<String>,
) -> AppResult<StatusCode> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        "UPDATE notification_deliveries SET read_at = COALESCE(read_at, ?) WHERE id = ? AND recipient_creator_id = ?",
    )
    .bind(&now)
    .bind(&notification_id)
    .bind(creator_id)
    .execute(state.db.sqlite_adapter())
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
