use super::*;

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/api/v1/admin/notifications/deliveries",
            get(list_admin_notification_deliveries),
        )
        .route(
            "/api/v1/admin/notifications/deliveries/:delivery_id",
            get(get_admin_notification_delivery),
        )
        .route(
            "/api/v1/admin/notifications/deliveries/:delivery_id/reconcile",
            post(reconcile_admin_notification_delivery),
        )
        .route(
            "/api/v1/admin/notifications/deliveries/:delivery_id/retry",
            post(retry_admin_notification_delivery),
        )
        .route(
            "/api/v1/admin/media/upload-jobs",
            get(list_admin_media_jobs),
        )
        .route(
            "/api/v1/admin/media/upload-jobs/:job_id",
            get(get_admin_media_job),
        )
        .route(
            "/api/v1/admin/media/upload-jobs/:job_id/reconcile",
            post(reconcile_admin_media_job),
        )
        .route(
            "/api/v1/admin/media/upload-jobs/:job_id/retry",
            post(retry_admin_media_job),
        )
}

pub(super) async fn list_admin_notification_deliveries(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<NotificationDeliveryQuery>,
) -> AppResult<Json<Vec<NotificationDeliveryRecord>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    Ok(Json(
        fetch_notification_deliveries(
            &state.pool,
            query.state.as_deref(),
            query.creator_id.as_deref(),
            query.limit.unwrap_or(100),
        )
        .await?,
    ))
}

pub(super) async fn get_admin_notification_delivery(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(delivery_id): Path<String>,
) -> AppResult<Json<NotificationDeliveryRecord>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    Ok(Json(
        fetch_notification_delivery_by_id_raw(&state.pool, &delivery_id).await?,
    ))
}

pub(super) async fn reconcile_admin_notification_delivery(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(delivery_id): Path<String>,
) -> AppResult<Json<NotificationDeliveryReconciliationReport>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    fetch_notification_delivery_by_id_raw(&state.pool, &delivery_id).await?;
    Ok(Json(
        reconcile_single_notification_delivery(state, &delivery_id).await?,
    ))
}

pub(super) async fn retry_admin_notification_delivery(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(delivery_id): Path<String>,
) -> AppResult<Json<NotificationDeliveryRecord>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    let delivery = fetch_notification_delivery_by_id(&state.pool, &delivery_id).await?;
    if delivery.state == "delivered" {
        return Ok(Json(delivery));
    }

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        UPDATE notification_deliveries
        SET state = 'retrying', failed_at = NULL, last_error = NULL, next_attempt_at = ?, last_attempted_at = NULL
        WHERE id = ?
        "#,
    )
    .bind(&now)
    .bind(&delivery_id)
    .execute(&state.pool)
    .await?;

    dispatch_notification_delivery(&state.pool, &delivery_id).await?;
    Ok(Json(
        fetch_notification_delivery_by_id(&state.pool, &delivery_id).await?,
    ))
}

pub(super) async fn list_admin_media_jobs(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<AdminMediaJobQuery>,
) -> AppResult<Json<Vec<AdminMediaJobRecord>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    Ok(Json(
        fetch_admin_media_jobs(
            &state.pool,
            query.status.as_deref(),
            query.creator_id.as_deref(),
            query.limit.unwrap_or(100),
        )
        .await?,
    ))
}

pub(super) async fn get_admin_media_job(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
) -> AppResult<Json<AdminMediaJobRecord>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    let creator_id = fetch_upload_job_creator_id(&state.pool, &job_id).await?;
    Ok(Json(
        fetch_admin_media_job_record(&state.pool, &creator_id, &job_id).await?,
    ))
}

pub(super) async fn reconcile_admin_media_job(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
) -> AppResult<Json<MediaJobReconciliationReport>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    fetch_upload_job_creator_id(&state.pool, &job_id).await?;
    Ok(Json(reconcile_single_media_job(state, &job_id).await?))
}

pub(super) async fn retry_admin_media_job(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
) -> AppResult<Json<AdminMediaJobRecord>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    let job = fetch_upload_job_by_id_global(&state.pool, &job_id).await?;
    if job.upload_job.status != "failed"
        && !(job.upload_job.status == "processing" && is_upload_job_stale(&job.upload_job))
    {
        return Err(AppError::BadRequest(
            "only failed or stale processing media jobs can be retried by operators".to_string(),
        ));
    }
    requeue_media_job_for_processing(&state.pool, &job.creator_id, &job.upload_job.id).await?;
    schedule_media_processing(
        state.clone(),
        job.creator_id.clone(),
        job.upload_job.id.clone(),
    )
    .await;
    Ok(Json(
        fetch_admin_media_job_record(&state.pool, &job.creator_id, &job.upload_job.id).await?,
    ))
}
