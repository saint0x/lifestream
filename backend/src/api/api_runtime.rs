use super::*;
use crate::api::control::reconcile_live_runtime_output_artifacts_background;
use crate::api::dashboard::reconcile_creator_attention_rollups;

pub(super) fn start_background_workers(state: SharedState) {
    tokio::spawn(async move {
        run_background_worker_loop(state).await;
    });
}

pub(super) async fn run_background_worker_loop(state: SharedState) {
    tracing::info!("background worker loop started");
    loop {
        run_background_worker_pass(state.clone()).await;
        sleep(Duration::from_secs(5)).await;
    }
}

async fn run_background_worker_pass(state: SharedState) {
    state.background_worker.mark_tick().await;
    if state.database_kind == crate::config::DatabaseKind::Postgres {
        match state.db.check().await {
            Ok(true) => state.background_worker.mark_success().await,
            Ok(false) => {
                state
                    .background_worker
                    .mark_failure("postgres health check failed".to_string())
                    .await;
            }
            Err(error) => {
                tracing::warn!(error = %error, "postgres background worker health check failed");
                state
                    .background_worker
                    .mark_failure(format!(
                        "postgres background worker health check failed: {error}"
                    ))
                    .await;
            }
        }
        if state
            .reconciliation_gates
            .should_run("creator_attention_rollups", Duration::from_secs(60))
            .await
        {
            match reconcile_creator_attention_rollups(&state.db).await {
                Ok(updated) => {
                    tracing::info!(
                        updated_rollups = updated,
                        "creator attention rollup reconciliation completed"
                    );
                }
                Err(error) => {
                    tracing::warn!(error = %error, "creator attention rollup reconciliation failed");
                    state
                        .background_worker
                        .mark_failure(format!(
                            "creator attention rollup reconciliation failed: {error}"
                        ))
                        .await;
                }
            }
        }
        return;
    }

    let mut errors = Vec::new();

    match fetch_pending_media_jobs(state.db.sqlite_adapter()).await {
        Ok(pending_jobs) => {
            for (creator_id, job_id) in pending_jobs {
                tracing::info!(
                    creator_id = %creator_id,
                    job_id = %job_id,
                    "scheduling pending media job"
                );
                schedule_media_processing(state.clone(), creator_id, job_id).await;
            }
        }
        Err(error) => {
            tracing::warn!(error = %error, "pending media jobs fetch failed");
            errors.push(format!("pending media jobs fetch failed: {error}"));
        }
    }

    if let Err(error) = reconcile_stale_live_ingest_sessions(state.clone()).await {
        tracing::warn!(error = %error, "stale live ingest reconciliation failed");
        errors.push(format!("stale live ingest reconciliation failed: {error}"));
    }
    if let Err(error) = reconcile_live_runtime_output_artifacts_background(state.clone()).await {
        tracing::warn!(error = %error, "live runtime artifact reconciliation failed");
        errors.push(format!(
            "live runtime artifact reconciliation failed: {error}"
        ));
    }
    if let Err(error) = reconcile_expired_collaboration_invites(state.clone()).await {
        tracing::warn!(error = %error, "expired collaboration invite reconciliation failed");
        errors.push(format!(
            "expired collaboration invite reconciliation failed: {error}"
        ));
    }
    if let Err(error) = reconcile_expired_collaboration_mirror_grants(state.clone()).await {
        tracing::warn!(error = %error, "expired collaboration mirror grant reconciliation failed");
        errors.push(format!(
            "expired collaboration mirror grant reconciliation failed: {error}"
        ));
    }
    if let Err(error) = reconcile_expired_user_entitlements(state.clone()).await {
        tracing::warn!(error = %error, "expired entitlement reconciliation failed");
        errors.push(format!(
            "expired entitlement reconciliation failed: {error}"
        ));
    }
    if let Err(error) = reconcile_expired_live_moderation_actions(state.clone()).await {
        tracing::warn!(error = %error, "expired live moderation reconciliation failed");
        errors.push(format!(
            "expired live moderation reconciliation failed: {error}"
        ));
    }
    if let Err(error) = reconcile_expired_creator_enforcement_actions(state.clone()).await {
        tracing::warn!(error = %error, "expired creator enforcement reconciliation failed");
        errors.push(format!(
            "expired creator enforcement reconciliation failed: {error}"
        ));
    }
    if let Err(error) = reconcile_notification_deliveries(state.clone()).await {
        tracing::warn!(error = %error, "notification delivery reconciliation failed");
        errors.push(format!(
            "notification delivery reconciliation failed: {error}"
        ));
    }
    if let Err(error) = reconcile_stale_media_processing_jobs(state.clone()).await {
        tracing::warn!(error = %error, "stale media processing reconciliation failed");
        errors.push(format!(
            "stale media processing reconciliation failed: {error}"
        ));
    }
    if let Err(error) = reconcile_scheduled_upload_releases(state.clone()).await {
        tracing::warn!(error = %error, "scheduled release reconciliation failed");
        errors.push(format!("scheduled release reconciliation failed: {error}"));
    }
    if let Err(error) = reconcile_stale_presence_sessions(state.clone()).await {
        tracing::warn!(error = %error, "stale presence reconciliation failed");
        errors.push(format!("stale presence reconciliation failed: {error}"));
    }
    if let Err(error) = reconcile_invalid_playback_sessions(state.clone()).await {
        tracing::warn!(error = %error, "invalid playback session reconciliation failed");
        errors.push(format!(
            "invalid playback session reconciliation failed: {error}"
        ));
    }
    if state
        .reconciliation_gates
        .should_run("creator_attention_rollups", Duration::from_secs(60))
        .await
    {
        match reconcile_creator_attention_rollups(&state.db).await {
            Ok(updated) => {
                tracing::info!(
                    updated_rollups = updated,
                    "creator attention rollup reconciliation completed"
                );
            }
            Err(error) => {
                tracing::warn!(error = %error, "creator attention rollup reconciliation failed");
                errors.push(format!(
                    "creator attention rollup reconciliation failed: {error}"
                ));
            }
        }
    }

    if errors.is_empty() {
        state.background_worker.mark_success().await;
    } else {
        tracing::warn!(
            error_count = errors.len(),
            errors = %errors.join("; "),
            "background worker loop completed with failures"
        );
        state
            .background_worker
            .mark_failure(errors.join("; "))
            .await;
    }
}
