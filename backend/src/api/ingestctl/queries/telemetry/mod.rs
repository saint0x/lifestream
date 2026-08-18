use super::*;
use crate::models::{LiveRuntimeTelemetry, LiveRuntimeTelemetrySummary};

mod current;
mod recent;
mod record;
mod summary;

pub(crate) use current::fetch_current_operational_telemetry;
pub(crate) use recent::{
    fetch_live_runtime_telemetry_for_session, fetch_recent_live_runtime_telemetry,
};
pub(crate) use record::record_live_runtime_telemetry;
pub(crate) use summary::{
    fetch_live_runtime_telemetry_summary, fetch_live_runtime_telemetry_summary_for_session,
};

fn live_runtime_telemetry_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> AppResult<LiveRuntimeTelemetry> {
    Ok(LiveRuntimeTelemetry {
        id: row.get("id"),
        session_id: row.get("session_id"),
        creator_id: row.get("creator_id"),
        broadcast_id: row.get("broadcast_id"),
        sample_kind: row.get("sample_kind"),
        runtime_state: row.get("runtime_state"),
        packaging_status: row.get("packaging_status"),
        archive_status: row.get("archive_status"),
        bitrate_kbps: row.get("bitrate_kbps"),
        viewers: row.get("viewers"),
        dropped_frames: row.get("dropped_frames"),
        cpu_percent: row.get("cpu_percent"),
        free_disk_gb: row.get("free_disk_gb"),
        detail: serde_json::from_str(&row.get::<String, _>("detail_json")).unwrap_or(json!({})),
        collected_at: row.get("collected_at"),
    })
}
