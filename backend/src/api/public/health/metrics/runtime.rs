use super::*;
use crate::state::BackgroundWorkerHealthSnapshot;

pub(super) fn write_runtime_metrics(body: &mut String, worker: &BackgroundWorkerHealthSnapshot) {
    write_gauge(
        body,
        "vanta_background_worker_ready",
        if worker
            .last_success_age_seconds
            .is_some_and(|age| age <= BACKGROUND_WORKER_STALE_AFTER_SECONDS)
        {
            1
        } else {
            0
        },
    );
    write_gauge(
        body,
        "vanta_background_worker_last_tick_age_seconds",
        worker.last_tick_age_seconds.unwrap_or(u64::MAX),
    );
    write_gauge(
        body,
        "vanta_background_worker_last_success_age_seconds",
        worker.last_success_age_seconds.unwrap_or(u64::MAX),
    );
    write_gauge(
        body,
        "vanta_background_worker_consecutive_failures",
        worker.consecutive_failures,
    );
}
