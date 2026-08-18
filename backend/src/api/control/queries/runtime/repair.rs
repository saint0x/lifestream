use super::*;

pub(crate) async fn update_live_runtime_output(
    pool: &SqlitePool,
    session: &LiveIngestSession,
    input: &UpdateLiveRuntimeStateRequest,
) -> AppResult<LiveRuntimeOutput> {
    validate_runtime_state_input(input)?;
    validate_runtime_output_model(input)?;
    let current = fetch_live_runtime_output_for_session(pool, &session.id).await?;
    validate_runtime_report_transition(session, current.as_ref(), input)?;
    let manifest_relative_path = resolve_manifest_relative_path(
        session,
        input.packaging_status.trim(),
        input.manifest_relative_path.as_deref(),
    )?;
    let archive_relative_path = resolve_archive_relative_path(
        session,
        input.archive_status.trim(),
        input.archive_relative_path.as_deref(),
    )?;
    let last_error = normalize_optional_text(input.last_error.as_deref());

    upsert_live_runtime_output(
        pool,
        session,
        input.runtime_state.trim(),
        input.packaging_status.trim(),
        input.archive_status.trim(),
        manifest_relative_path,
        archive_relative_path,
        last_error,
    )
    .await
}

pub(crate) async fn repair_live_runtime_output(
    pool: &SqlitePool,
    session: &LiveIngestSession,
    input: &RepairLiveRuntimeOutputRequest,
) -> AppResult<(LiveRuntimeOutput, Vec<LiveRuntimeRepairAction>)> {
    if input.reason.trim().is_empty() {
        return Err(AppError::BadRequest(
            "runtime repair reason is required".to_string(),
        ));
    }
    if input.clear_manifest_relative_path && input.manifest_relative_path.is_some() {
        return Err(AppError::BadRequest(
            "manifestRelativePath cannot be set and cleared in the same repair".to_string(),
        ));
    }
    if input.clear_archive_relative_path && input.archive_relative_path.is_some() {
        return Err(AppError::BadRequest(
            "archiveRelativePath cannot be set and cleared in the same repair".to_string(),
        ));
    }
    if input.clear_last_error && input.last_error.is_some() {
        return Err(AppError::BadRequest(
            "lastError cannot be set and cleared in the same repair".to_string(),
        ));
    }

    let current = match fetch_live_runtime_output_for_session(pool, &session.id).await? {
        Some(output) => output,
        None => {
            initialize_live_runtime_output(pool, session).await?;
            fetch_live_runtime_output_for_session(pool, &session.id)
                .await?
                .ok_or_else(|| {
                    AppError::Internal(
                        "missing live runtime output after initialization".to_string(),
                    )
                })?
        }
    };

    let manifest_relative_path = if input.clear_manifest_relative_path {
        None
    } else if let Some(value) = input.manifest_relative_path.as_deref() {
        resolve_manifest_relative_path(
            session,
            input
                .packaging_status
                .as_deref()
                .unwrap_or(current.packaging_status.as_str()),
            Some(value),
        )?
    } else {
        current.manifest_relative_path.clone()
    };
    let archive_relative_path = if input.clear_archive_relative_path {
        None
    } else if let Some(value) = input.archive_relative_path.as_deref() {
        resolve_archive_relative_path(
            session,
            input
                .archive_status
                .as_deref()
                .unwrap_or(current.archive_status.as_str()),
            Some(value),
        )?
    } else {
        current.archive_relative_path.clone()
    };
    let last_error = if input.clear_last_error {
        None
    } else if let Some(value) = input.last_error.as_deref() {
        normalize_optional_text(Some(value))
    } else {
        current.last_error.clone()
    };

    let merged = UpdateLiveRuntimeStateRequest {
        runtime_state: input
            .runtime_state
            .clone()
            .unwrap_or_else(|| current.runtime_state.clone()),
        packaging_status: input
            .packaging_status
            .clone()
            .unwrap_or_else(|| current.packaging_status.clone()),
        archive_status: input
            .archive_status
            .clone()
            .unwrap_or_else(|| current.archive_status.clone()),
        manifest_relative_path: manifest_relative_path.clone(),
        archive_relative_path: archive_relative_path.clone(),
        last_error: last_error.clone(),
    };
    validate_runtime_state_input(&merged)?;
    validate_runtime_output_model(&merged)?;

    let mut actions = Vec::new();
    push_repair_action(
        &mut actions,
        "runtimeState",
        Some(current.runtime_state.clone()),
        Some(merged.runtime_state.clone()),
    );
    push_repair_action(
        &mut actions,
        "packagingStatus",
        Some(current.packaging_status.clone()),
        Some(merged.packaging_status.clone()),
    );
    push_repair_action(
        &mut actions,
        "archiveStatus",
        Some(current.archive_status.clone()),
        Some(merged.archive_status.clone()),
    );
    push_repair_action(
        &mut actions,
        "manifestRelativePath",
        current.manifest_relative_path.clone(),
        manifest_relative_path.clone(),
    );
    push_repair_action(
        &mut actions,
        "archiveRelativePath",
        current.archive_relative_path.clone(),
        archive_relative_path.clone(),
    );
    push_repair_action(
        &mut actions,
        "lastError",
        current.last_error.clone(),
        last_error.clone(),
    );

    if actions.is_empty() {
        return Err(AppError::BadRequest(
            "runtime repair must change at least one field".to_string(),
        ));
    }

    let output = upsert_live_runtime_output(
        pool,
        session,
        &merged.runtime_state,
        &merged.packaging_status,
        &merged.archive_status,
        manifest_relative_path,
        archive_relative_path,
        last_error,
    )
    .await?;
    Ok((output, actions))
}

pub(crate) async fn set_live_runtime_output_session_state(
    pool: &SqlitePool,
    session: &LiveIngestSession,
    runtime_state: &str,
) -> AppResult<LiveRuntimeOutput> {
    let packaging_status = match runtime_state {
        "stale" | "disconnected" => "degraded",
        "archive_complete" => "complete",
        _ => "pending",
    };
    let archive_status = match runtime_state {
        "archive_finalizing" => "finalizing",
        "archive_complete" => "complete",
        _ => "not_started",
    };
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
            runtime_state = excluded.runtime_state,
            packaging_status = CASE
                WHEN live_runtime_outputs.packaging_status IN ('ready', 'complete', 'failed')
                    THEN live_runtime_outputs.packaging_status
                ELSE excluded.packaging_status
            END,
            archive_status = CASE
                WHEN live_runtime_outputs.archive_status IN ('finalizing', 'complete', 'failed')
                    THEN live_runtime_outputs.archive_status
                ELSE excluded.archive_status
            END,
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
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    fetch_live_runtime_output_for_session(pool, &session.id)
        .await?
        .ok_or_else(|| AppError::BadRequest("live runtime output unavailable".to_string()))
}

fn push_repair_action(
    actions: &mut Vec<LiveRuntimeRepairAction>,
    field: &str,
    previous_value: Option<String>,
    next_value: Option<String>,
) {
    if previous_value == next_value {
        return;
    }
    actions.push(LiveRuntimeRepairAction {
        field: field.to_string(),
        previous_value,
        next_value,
    });
}
