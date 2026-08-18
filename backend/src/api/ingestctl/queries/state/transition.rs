use super::*;

pub(crate) fn validate_runtime_report_transition(
    session: &LiveIngestSession,
    current: Option<&LiveRuntimeOutput>,
    next: &UpdateLiveRuntimeStateRequest,
) -> AppResult<()> {
    let next_state = next.runtime_state.trim();
    validate_runtime_state_for_session_status(session, next_state)?;

    let Some(current) = current else {
        return Ok(());
    };
    let current_state = current.runtime_state.as_str();
    if current_state == next_state {
        return Ok(());
    }
    if is_runtime_transition_allowed(current_state, next_state) {
        return Ok(());
    }

    Err(AppError::BadRequest(format!(
        "runtimeState transition {current_state} -> {next_state} is not allowed"
    )))
}

fn validate_runtime_state_for_session_status(
    session: &LiveIngestSession,
    runtime_state: &str,
) -> AppResult<()> {
    match session.status.as_str() {
        "connected" => Ok(()),
        "stale"
            if matches!(
                runtime_state,
                "stale" | "archive_finalizing" | "archive_complete" | "failed"
            ) =>
        {
            Ok(())
        }
        "ended" | "terminated"
            if matches!(
                runtime_state,
                "disconnected" | "archive_finalizing" | "archive_complete" | "failed"
            ) =>
        {
            Ok(())
        }
        "stale" => Err(AppError::BadRequest(format!(
            "stale ingest sessions cannot report runtimeState={runtime_state}"
        ))),
        "ended" | "terminated" => Err(AppError::BadRequest(format!(
            "terminal ingest sessions cannot report runtimeState={runtime_state}"
        ))),
        status => Err(AppError::BadRequest(format!(
            "live ingest session status {status} cannot accept runtime reports"
        ))),
    }
}

fn is_runtime_transition_allowed(current: &str, next: &str) -> bool {
    match current {
        "pending_attach" => matches!(
            next,
            "attached"
                | "healthy"
                | "degraded"
                | "packaging_active"
                | "packaging_degraded"
                | "stale"
                | "disconnected"
                | "failed"
        ),
        "attached" => matches!(
            next,
            "healthy"
                | "degraded"
                | "packaging_active"
                | "packaging_degraded"
                | "stale"
                | "disconnected"
                | "failed"
        ),
        "healthy" => matches!(
            next,
            "degraded"
                | "packaging_active"
                | "packaging_degraded"
                | "archive_finalizing"
                | "stale"
                | "disconnected"
                | "failed"
        ),
        "degraded" => matches!(
            next,
            "healthy"
                | "packaging_active"
                | "packaging_degraded"
                | "archive_finalizing"
                | "stale"
                | "disconnected"
                | "failed"
        ),
        "packaging_active" => matches!(
            next,
            "healthy"
                | "degraded"
                | "packaging_degraded"
                | "archive_finalizing"
                | "stale"
                | "disconnected"
                | "failed"
        ),
        "packaging_degraded" => matches!(
            next,
            "healthy"
                | "degraded"
                | "packaging_active"
                | "archive_finalizing"
                | "stale"
                | "disconnected"
                | "failed"
        ),
        "archive_finalizing" => {
            matches!(next, "archive_complete" | "disconnected" | "failed")
        }
        "stale" => matches!(next, "archive_finalizing" | "archive_complete" | "failed"),
        "disconnected" => matches!(next, "archive_finalizing" | "archive_complete" | "failed"),
        "archive_complete" | "failed" => false,
        _ => false,
    }
}
