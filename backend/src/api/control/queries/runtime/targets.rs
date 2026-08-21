use super::*;
use crate::models::LiveRuntimeTarget;

#[derive(Clone, Debug, Default)]
pub(crate) struct LiveRuntimeTargetSyncReport {
    pub created: usize,
    pub updated: usize,
    pub removed: usize,
}

pub(crate) async fn sync_live_runtime_targets(
    pool: &SqlitePool,
    session: &LiveIngestSession,
    targets: &[LiveRuntimeTarget],
) -> AppResult<LiveRuntimeTargetSyncReport> {
    let existing = fetch_live_runtime_targets_for_session(pool, &session.id).await?;
    let existing_by_key = existing
        .iter()
        .map(|target| (runtime_target_identity_key(target), target))
        .collect::<std::collections::HashMap<_, _>>();
    let target_keys = targets
        .iter()
        .map(runtime_target_identity_key)
        .collect::<std::collections::HashSet<_>>();
    let mut tx = pool.begin().await?;
    let mut report = LiveRuntimeTargetSyncReport::default();

    for target in targets {
        let target_key = runtime_target_identity_key(target);
        let existing_target = existing_by_key.get(&target_key).copied();
        if existing_target.is_some_and(|current| !runtime_target_changed(current, target)) {
            continue;
        }
        let created_at = existing_target
            .map(|current| current.created_at.clone())
            .unwrap_or_else(|| target.created_at.clone());
        sqlx::query(
            r#"
            INSERT INTO live_runtime_targets (
                id, session_id, creator_id, broadcast_id, target_kind, target_key, target_label,
                route_state, target_creator_id, target_broadcast_id, playback_enabled,
                recording_enabled, mix_minus_required, relative_path, source_participant_ids_json,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(session_id, target_kind, target_key) DO UPDATE SET
                id = excluded.id,
                creator_id = excluded.creator_id,
                broadcast_id = excluded.broadcast_id,
                target_label = excluded.target_label,
                route_state = excluded.route_state,
                target_creator_id = excluded.target_creator_id,
                target_broadcast_id = excluded.target_broadcast_id,
                playback_enabled = excluded.playback_enabled,
                recording_enabled = excluded.recording_enabled,
                mix_minus_required = excluded.mix_minus_required,
                relative_path = excluded.relative_path,
                source_participant_ids_json = excluded.source_participant_ids_json,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&target.id)
        .bind(&target.session_id)
        .bind(&target.creator_id)
        .bind(&target.broadcast_id)
        .bind(&target.target_kind)
        .bind(&target.target_key)
        .bind(&target.target_label)
        .bind(&target.route_state)
        .bind(&target.target_creator_id)
        .bind(&target.target_broadcast_id)
        .bind(target.playback_enabled as i64)
        .bind(target.recording_enabled as i64)
        .bind(target.mix_minus_required as i64)
        .bind(&target.relative_path)
        .bind(
            serde_json::to_string(&target.source_participant_ids)
                .map_err(|error| AppError::Internal(error.to_string()))?,
        )
        .bind(&created_at)
        .bind(&target.updated_at)
        .execute(&mut *tx)
        .await?;

        if existing_target.is_some() {
            report.updated += 1;
        } else {
            report.created += 1;
        }
    }

    for target in existing {
        if target_keys.contains(&runtime_target_identity_key(&target)) {
            continue;
        }
        sqlx::query("DELETE FROM live_runtime_targets WHERE id = ?")
            .bind(&target.id)
            .execute(&mut *tx)
            .await?;
        report.removed += 1;
    }

    tx.commit().await?;
    Ok(report)
}

pub(crate) async fn fetch_live_runtime_targets_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<Vec<LiveRuntimeTarget>> {
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, creator_id, broadcast_id, target_kind, target_key, target_label,
               route_state, target_creator_id, target_broadcast_id, playback_enabled,
               recording_enabled, mix_minus_required, relative_path, source_participant_ids_json,
               created_at, updated_at
        FROM live_runtime_targets
        WHERE session_id = ?
        ORDER BY target_kind ASC, target_key ASC
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(live_runtime_target_from_row).collect()
}

pub(crate) async fn fetch_recent_live_runtime_targets(
    pool: &SqlitePool,
    creator_id: &str,
    limit: i64,
) -> AppResult<Vec<LiveRuntimeTarget>> {
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, creator_id, broadcast_id, target_kind, target_key, target_label,
               route_state, target_creator_id, target_broadcast_id, playback_enabled,
               recording_enabled, mix_minus_required, relative_path, source_participant_ids_json,
               created_at, updated_at
        FROM live_runtime_targets
        WHERE creator_id = ?
        ORDER BY updated_at DESC, target_kind ASC, target_key ASC
        LIMIT ?
        "#,
    )
    .bind(creator_id)
    .bind(limit.max(1))
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(live_runtime_target_from_row).collect()
}

fn live_runtime_target_from_row(row: sqlx::sqlite::SqliteRow) -> AppResult<LiveRuntimeTarget> {
    Ok(LiveRuntimeTarget {
        id: row.get("id"),
        session_id: row.get("session_id"),
        creator_id: row.get("creator_id"),
        broadcast_id: row.get("broadcast_id"),
        target_kind: row.get("target_kind"),
        target_key: row.get("target_key"),
        target_label: row.get("target_label"),
        route_state: row.get("route_state"),
        target_creator_id: row.get("target_creator_id"),
        target_broadcast_id: row.get("target_broadcast_id"),
        playback_enabled: row.get::<i64, _>("playback_enabled") != 0,
        recording_enabled: row.get::<i64, _>("recording_enabled") != 0,
        mix_minus_required: row.get::<i64, _>("mix_minus_required") != 0,
        relative_path: row.get("relative_path"),
        source_participant_ids: serde_json::from_str(
            &row.get::<String, _>("source_participant_ids_json"),
        )
        .map_err(|error| AppError::Internal(error.to_string()))?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn runtime_target_identity_key(target: &LiveRuntimeTarget) -> String {
    format!("{}:{}", target.target_kind, target.target_key)
}

fn runtime_target_changed(current: &LiveRuntimeTarget, next: &LiveRuntimeTarget) -> bool {
    current.id != next.id
        || current.creator_id != next.creator_id
        || current.broadcast_id != next.broadcast_id
        || current.target_label != next.target_label
        || current.route_state != next.route_state
        || current.target_creator_id != next.target_creator_id
        || current.target_broadcast_id != next.target_broadcast_id
        || current.playback_enabled != next.playback_enabled
        || current.recording_enabled != next.recording_enabled
        || current.mix_minus_required != next.mix_minus_required
        || current.relative_path != next.relative_path
        || current.source_participant_ids != next.source_participant_ids
}
