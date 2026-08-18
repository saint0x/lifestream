use super::*;

pub(super) fn start_background_workers(state: SharedState) {
    tokio::spawn(async move {
        loop {
            state.background_worker.mark_tick().await;
            let mut errors = Vec::new();

            match fetch_pending_media_jobs(&state.pool).await {
                Ok(pending_jobs) => {
                    for (creator_id, job_id) in pending_jobs {
                        schedule_media_processing(state.clone(), creator_id, job_id).await;
                    }
                }
                Err(error) => {
                    errors.push(format!("pending media jobs fetch failed: {error}"));
                }
            }

            if let Err(error) = reconcile_stale_live_ingest_sessions(state.clone()).await {
                errors.push(format!("stale live ingest reconciliation failed: {error}"));
            }
            if let Err(error) = reconcile_expired_collaboration_invites(state.clone()).await {
                errors.push(format!(
                    "expired collaboration invite reconciliation failed: {error}"
                ));
            }
            if let Err(error) = reconcile_expired_collaboration_mirror_grants(state.clone()).await {
                errors.push(format!(
                    "expired collaboration mirror grant reconciliation failed: {error}"
                ));
            }
            if let Err(error) = reconcile_expired_user_entitlements(state.clone()).await {
                errors.push(format!(
                    "expired entitlement reconciliation failed: {error}"
                ));
            }
            if let Err(error) = reconcile_expired_live_moderation_actions(state.clone()).await {
                errors.push(format!(
                    "expired live moderation reconciliation failed: {error}"
                ));
            }
            if let Err(error) = reconcile_expired_creator_enforcement_actions(state.clone()).await {
                errors.push(format!(
                    "expired creator enforcement reconciliation failed: {error}"
                ));
            }
            if let Err(error) = reconcile_notification_deliveries(state.clone()).await {
                errors.push(format!(
                    "notification delivery reconciliation failed: {error}"
                ));
            }
            if let Err(error) = reconcile_stale_media_processing_jobs(state.clone()).await {
                errors.push(format!(
                    "stale media processing reconciliation failed: {error}"
                ));
            }
            if let Err(error) = reconcile_scheduled_upload_releases(state.clone()).await {
                errors.push(format!("scheduled release reconciliation failed: {error}"));
            }
            if let Err(error) = reconcile_stale_presence_sessions(state.clone()).await {
                errors.push(format!("stale presence reconciliation failed: {error}"));
            }
            if let Err(error) = reconcile_invalid_playback_sessions(state.clone()).await {
                errors.push(format!(
                    "invalid playback session reconciliation failed: {error}"
                ));
            }

            if errors.is_empty() {
                state.background_worker.mark_success().await;
            } else {
                state
                    .background_worker
                    .mark_failure(errors.join("; "))
                    .await;
            }

            sleep(Duration::from_secs(5)).await;
        }
    });
}
