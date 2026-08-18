use super::record_collab::build_live_runtime_telemetry_collaboration;
use super::record_detail::build_live_runtime_telemetry_detail;
use super::*;
use crate::api::control::queries::fetch_live_runtime_targets_for_session;

pub(crate) async fn record_live_runtime_telemetry(
    pool: &SqlitePool,
    session: &LiveIngestSession,
    sample_kind: &str,
    runtime_state: &str,
    packaging_status: &str,
    archive_status: &str,
    cpu_percent: Option<i64>,
    free_disk_gb: Option<f64>,
    detail: Value,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let output = fetch_live_runtime_output_for_session(pool, &session.id).await?;
    let targets = fetch_live_runtime_targets_for_session(pool, &session.id).await?;
    let collaboration =
        build_live_runtime_telemetry_collaboration(pool, &session.broadcast_id).await?;
    let normalized_detail = build_live_runtime_telemetry_detail(
        session,
        sample_kind,
        runtime_state,
        packaging_status,
        archive_status,
        cpu_percent,
        free_disk_gb,
        output.as_ref(),
        &targets,
        collaboration.as_ref(),
        detail,
    );
    sqlx::query(
        r#"
        INSERT INTO live_runtime_telemetry (
            id, session_id, creator_id, broadcast_id, sample_kind, runtime_state,
            packaging_status, archive_status, bitrate_kbps, viewers, dropped_frames,
            cpu_percent, free_disk_gb, detail_json, collected_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(format!("lrt-{}", Uuid::new_v4().simple()))
    .bind(&session.id)
    .bind(&session.creator_id)
    .bind(&session.broadcast_id)
    .bind(sample_kind)
    .bind(runtime_state)
    .bind(packaging_status)
    .bind(archive_status)
    .bind(session.bitrate_kbps)
    .bind(session.viewers)
    .bind(session.dropped_frames)
    .bind(cpu_percent)
    .bind(free_disk_gb)
    .bind(normalized_detail.to_string())
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}
