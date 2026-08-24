use super::*;
use crate::api::control::{
    describe_live_runtime_artifact_health, reconcile_live_runtime_output_artifacts,
};
use crate::models::{
    AdminLiveIngestOverview, AdminLiveIngestOverviewQuery, LiveRuntimeRepairReport,
    RepairLiveRuntimeOutputRequest,
};

pub(crate) async fn get_admin_live_ingest_overview(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<AdminLiveIngestOverviewQuery>,
) -> AppResult<Json<AdminLiveIngestOverview>> {
    let identity = require_identity(&state.db, &headers).await?;
    identity.require_admin_scope()?;
    Ok(Json(
        fetch_admin_live_ingest_overview(
            state.db.try_sqlite_adapter()?,
            query.creator_id.as_deref(),
        )
        .await?,
    ))
}

pub(crate) async fn list_admin_live_ingest_sessions(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<AdminLiveIngestQuery>,
) -> AppResult<Json<Vec<AdminLiveIngestSessionRecord>>> {
    let identity = require_identity(&state.db, &headers).await?;
    identity.require_admin_scope()?;
    let records = fetch_admin_live_ingest_sessions(
        state.db.try_sqlite_adapter()?,
        query.creator_id.as_deref(),
        query.status.as_deref(),
        query.limit.unwrap_or(100),
    )
    .await?;
    let mut enriched = Vec::with_capacity(records.len());
    for record in records {
        enriched.push(build_authoritative_admin_live_ingest_record(&state, record).await?);
    }
    Ok(Json(enriched))
}

pub(crate) async fn get_admin_live_ingest_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<AdminLiveIngestSessionRecord>> {
    let identity = require_identity(&state.db, &headers).await?;
    identity.require_admin_scope()?;
    let session =
        fetch_live_ingest_session_by_id_global(state.db.try_sqlite_adapter()?, &session_id).await?;
    let _ = reconcile_live_runtime_output_artifacts(&state, &session).await?;
    let record =
        fetch_admin_live_ingest_session_record(state.db.try_sqlite_adapter()?, &session_id).await?;
    Ok(Json(
        build_authoritative_admin_live_ingest_record(&state, record).await?,
    ))
}

pub(crate) async fn reconcile_admin_live_ingest_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<LiveIngestReconciliationReport>> {
    let identity = require_identity(&state.db, &headers).await?;
    identity.require_admin_scope()?;
    fetch_live_ingest_session_by_id_global_unreconciled(
        state.db.try_sqlite_adapter()?,
        &session_id,
    )
    .await?;
    Ok(Json(
        reconcile_single_live_ingest_session(state, &session_id).await?,
    ))
}

pub(crate) async fn terminate_admin_live_ingest_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Json(input): Json<TerminateLiveIngestRequest>,
) -> AppResult<Json<AdminLiveIngestSessionRecord>> {
    let identity = require_identity(&state.db, &headers).await?;
    identity.require_admin_scope()?;
    let session =
        fetch_live_ingest_session_by_id_global(state.db.try_sqlite_adapter()?, &session_id).await?;
    if session.status != "connected" {
        return Err(AppError::BadRequest(
            "only connected live ingest sessions can be terminated by operators".to_string(),
        ));
    }
    close_live_ingest_session(
        &state,
        &session,
        "terminated",
        "admin_terminated",
        json!({
            "reason": input.reason.unwrap_or_else(|| "operator requested termination".to_string()),
            "actorUserId": identity.user_id,
        }),
    )
    .await?;
    let record =
        fetch_admin_live_ingest_session_record(state.db.try_sqlite_adapter()?, &session_id).await?;
    Ok(Json(
        build_authoritative_admin_live_ingest_record(&state, record).await?,
    ))
}

pub(crate) async fn repair_admin_live_runtime_output(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Json(input): Json<RepairLiveRuntimeOutputRequest>,
) -> AppResult<Json<LiveRuntimeRepairReport>> {
    let identity = require_identity(&state.db, &headers).await?;
    identity.require_admin_scope()?;
    let session =
        fetch_live_ingest_session_by_id_global(state.db.try_sqlite_adapter()?, &session_id).await?;
    Ok(Json(
        super::repair::repair_runtime_output_authoritatively(
            state,
            session,
            &identity.user_id,
            "admin",
            input,
        )
        .await?,
    ))
}

async fn build_authoritative_admin_live_ingest_record(
    state: &SharedState,
    mut record: AdminLiveIngestSessionRecord,
) -> AppResult<AdminLiveIngestSessionRecord> {
    if record.session.status == "connected" || record.session.status == "stale" {
        let _ = reconcile_live_runtime_output_artifacts(state, &record.session).await?;
        record = fetch_admin_live_ingest_session_record(
            state.db.try_sqlite_adapter()?,
            &record.session.id,
        )
        .await?;
    }
    if let Some(output) = record.runtime_output.as_ref() {
        record.artifact_health =
            Some(describe_live_runtime_artifact_health(state, &record.session, output).await?);
    }
    Ok(record)
}
