use super::doc::LiveRuntimeHealthSpec;
use super::*;

pub(super) fn build_live_runtime_health_spec(
    session: &LiveIngestSession,
    current_cpu_percent: Option<i64>,
    current_free_disk_gb: Option<f64>,
) -> LiveRuntimeHealthSpec {
    const CPU_WARN_PERCENT: i64 = 85;
    const CPU_CRITICAL_PERCENT: i64 = 95;
    const FREE_DISK_WARN_GB: f64 = 20.0;
    const FREE_DISK_CRITICAL_GB: f64 = 5.0;
    const INGEST_LATENCY_WARN_MS: i64 = 1500;
    const INGEST_LATENCY_CRITICAL_MS: i64 = 3000;
    const DROPPED_FRAMES_WARN: i64 = 100;
    const DROPPED_FRAMES_CRITICAL: i64 = 1000;

    let status = if current_cpu_percent.is_some_and(|value| value >= CPU_CRITICAL_PERCENT)
        || current_free_disk_gb.is_some_and(|value| value <= FREE_DISK_CRITICAL_GB)
        || session
            .ingest_latency_ms
            .is_some_and(|value| value >= INGEST_LATENCY_CRITICAL_MS)
        || session.dropped_frames >= DROPPED_FRAMES_CRITICAL
    {
        "critical"
    } else if current_cpu_percent.is_some_and(|value| value >= CPU_WARN_PERCENT)
        || current_free_disk_gb.is_some_and(|value| value <= FREE_DISK_WARN_GB)
        || session
            .ingest_latency_ms
            .is_some_and(|value| value >= INGEST_LATENCY_WARN_MS)
        || session.dropped_frames >= DROPPED_FRAMES_WARN
    {
        "warn"
    } else {
        "ok"
    };

    LiveRuntimeHealthSpec {
        status: status.to_string(),
        current_cpu_percent,
        current_free_disk_gb,
        current_ingest_latency_ms: session.ingest_latency_ms,
        current_dropped_frames: session.dropped_frames,
        cpu_warn_percent: CPU_WARN_PERCENT,
        cpu_critical_percent: CPU_CRITICAL_PERCENT,
        free_disk_warn_gb: FREE_DISK_WARN_GB,
        free_disk_critical_gb: FREE_DISK_CRITICAL_GB,
        ingest_latency_warn_ms: INGEST_LATENCY_WARN_MS,
        ingest_latency_critical_ms: INGEST_LATENCY_CRITICAL_MS,
        dropped_frames_warn: DROPPED_FRAMES_WARN,
        dropped_frames_critical: DROPPED_FRAMES_CRITICAL,
    }
}
