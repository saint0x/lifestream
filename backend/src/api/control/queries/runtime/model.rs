use super::*;

pub(crate) async fn initialize_live_runtime_output(
    pool: &SqlitePool,
    session: &LiveIngestSession,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let profile = derive_live_runtime_profile(pool, session).await?;
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO live_runtime_outputs (
            id, session_id, creator_id, broadcast_id, runtime_state, packaging_status,
            archive_status, runtime_class, latency_profile, segment_format,
            partial_segments_enabled, blocking_reload_enabled, target_segment_duration_sec,
            hold_back_segments, discontinuity_sequence, ladder_policy, content_class,
            manifest_relative_path, archive_relative_path, last_error, last_runtime_event_at,
            created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(format!("lro-{}", Uuid::new_v4().simple()))
    .bind(&session.id)
    .bind(&session.creator_id)
    .bind(&session.broadcast_id)
    .bind("pending_attach")
    .bind("pending")
    .bind("not_started")
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
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn fetch_live_runtime_output_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<Option<LiveRuntimeOutput>> {
    let row = sqlx::query(
        r#"
        SELECT id, session_id, creator_id, broadcast_id, runtime_state, packaging_status,
               archive_status, runtime_class, latency_profile, segment_format,
               partial_segments_enabled, blocking_reload_enabled, target_segment_duration_sec,
               hold_back_segments, discontinuity_sequence, ladder_policy, content_class,
               manifest_relative_path, archive_relative_path, last_error, last_runtime_event_at,
               created_at, updated_at
        FROM live_runtime_outputs
        WHERE session_id = ?
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;

    row.map(live_runtime_output_from_row).transpose()
}

pub(crate) async fn fetch_recent_live_runtime_outputs(
    pool: &SqlitePool,
    creator_id: &str,
    limit: i64,
) -> AppResult<Vec<LiveRuntimeOutput>> {
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, creator_id, broadcast_id, runtime_state, packaging_status,
               archive_status, runtime_class, latency_profile, segment_format,
               partial_segments_enabled, blocking_reload_enabled, target_segment_duration_sec,
               hold_back_segments, discontinuity_sequence, ladder_policy, content_class,
               manifest_relative_path, archive_relative_path, last_error, last_runtime_event_at,
               created_at, updated_at
        FROM live_runtime_outputs
        WHERE creator_id = ?
        ORDER BY updated_at DESC
        LIMIT ?
        "#,
    )
    .bind(creator_id)
    .bind(limit.max(1))
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(live_runtime_output_from_row).collect()
}

pub(super) fn live_runtime_output_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> AppResult<LiveRuntimeOutput> {
    Ok(LiveRuntimeOutput {
        id: row.get("id"),
        session_id: row.get("session_id"),
        creator_id: row.get("creator_id"),
        broadcast_id: row.get("broadcast_id"),
        runtime_state: row.get("runtime_state"),
        packaging_status: row.get("packaging_status"),
        archive_status: row.get("archive_status"),
        runtime_class: row.get("runtime_class"),
        latency_profile: row.get("latency_profile"),
        segment_format: row.get("segment_format"),
        partial_segments_enabled: row.get::<i64, _>("partial_segments_enabled") != 0,
        blocking_reload_enabled: row.get::<i64, _>("blocking_reload_enabled") != 0,
        target_segment_duration_sec: row.get("target_segment_duration_sec"),
        hold_back_segments: row.get("hold_back_segments"),
        discontinuity_sequence: row.get("discontinuity_sequence"),
        ladder_policy: row.get("ladder_policy"),
        content_class: row.get("content_class"),
        manifest_relative_path: row.get("manifest_relative_path"),
        archive_relative_path: row.get("archive_relative_path"),
        last_error: row.get("last_error"),
        last_runtime_event_at: row.get("last_runtime_event_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}
