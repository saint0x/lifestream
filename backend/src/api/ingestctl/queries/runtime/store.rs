use super::*;

pub(super) async fn upsert_live_runtime_output(
    pool: &SqlitePool,
    session: &LiveIngestSession,
    runtime_state: &str,
    packaging_status: &str,
    archive_status: &str,
    manifest_relative_path: Option<String>,
    archive_relative_path: Option<String>,
    last_error: Option<String>,
) -> AppResult<LiveRuntimeOutput> {
    let now = Utc::now().to_rfc3339();
    let profile = derive_live_runtime_profile(pool, session).await?;
    sqlx::query(
        r#"
        INSERT INTO live_runtime_outputs (
            id, session_id, creator_id, broadcast_id, runtime_state, packaging_status,
            archive_status, runtime_class, latency_profile, segment_format,
            partial_segments_enabled, blocking_reload_enabled, target_segment_duration_sec,
            hold_back_segments, discontinuity_sequence, ladder_policy, content_class,
            manifest_relative_path, archive_relative_path, last_error, last_runtime_event_at,
            created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(session_id) DO UPDATE SET
            creator_id = excluded.creator_id,
            broadcast_id = excluded.broadcast_id,
            runtime_state = excluded.runtime_state,
            packaging_status = excluded.packaging_status,
            archive_status = excluded.archive_status,
            runtime_class = excluded.runtime_class,
            latency_profile = excluded.latency_profile,
            segment_format = excluded.segment_format,
            partial_segments_enabled = excluded.partial_segments_enabled,
            blocking_reload_enabled = excluded.blocking_reload_enabled,
            target_segment_duration_sec = excluded.target_segment_duration_sec,
            hold_back_segments = excluded.hold_back_segments,
            discontinuity_sequence = excluded.discontinuity_sequence,
            ladder_policy = excluded.ladder_policy,
            content_class = excluded.content_class,
            manifest_relative_path = excluded.manifest_relative_path,
            archive_relative_path = excluded.archive_relative_path,
            last_error = excluded.last_error,
            last_runtime_event_at = excluded.last_runtime_event_at,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(format!("lro-{}", Uuid::new_v4().simple()))
    .bind(&session.id)
    .bind(&session.creator_id)
    .bind(&session.broadcast_id)
    .bind(runtime_state)
    .bind(packaging_status)
    .bind(archive_status)
    .bind(&profile.runtime_class)
    .bind(&profile.latency_profile)
    .bind(&profile.segment_format)
    .bind(profile.partial_segments_enabled as i64)
    .bind(profile.blocking_reload_enabled as i64)
    .bind(profile.target_segment_duration_sec)
    .bind(profile.hold_back_segments)
    .bind(profile.discontinuity_sequence)
    .bind(&profile.ladder_policy)
    .bind(&profile.content_class)
    .bind(manifest_relative_path)
    .bind(archive_relative_path)
    .bind(last_error)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    fetch_live_runtime_output_for_session(pool, &session.id)
        .await?
        .ok_or_else(|| AppError::Internal("missing live runtime output after update".to_string()))
}
