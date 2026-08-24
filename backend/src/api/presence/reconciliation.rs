use super::*;

const READ_RECONCILIATION_MIN_INTERVAL: Duration = Duration::from_millis(750);

pub(crate) async fn reconcile_stale_presence_sessions(state: SharedState) -> AppResult<()> {
    let cutoff = active_presence_cutoff();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE live_viewer_sessions SET disconnected_at = COALESCE(disconnected_at, ?), last_seen_at = MIN(last_seen_at, ?) WHERE disconnected_at IS NULL AND last_seen_at < ?",
    )
    .bind(&now)
    .bind(&cutoff)
    .bind(&cutoff)
    .execute(state.db.try_sqlite_adapter()?)
    .await?;

    reconcile_stale_creator_live_socket_sessions_for_read(
        state.db.try_sqlite_adapter()?,
        None,
        None,
    )
    .await?;

    let session_rows = sqlx::query(
        "SELECT DISTINCT collaboration_session_id FROM collaboration_socket_sessions WHERE disconnected_at IS NULL AND last_seen_at < ?",
    )
    .bind(&cutoff)
    .fetch_all(state.db.try_sqlite_adapter()?)
    .await?;

    for row in session_rows {
        let session_id: String = row.get("collaboration_session_id");
        let _ = disconnect_stale_collaboration_socket_sessions_for_session(
            &state,
            &session_id,
            &now,
            &cutoff,
        )
        .await?;
    }

    Ok(())
}

pub(crate) async fn reconcile_stale_creator_live_socket_sessions_for_read_coalesced(
    state: &SharedState,
    creator_filter: Option<&str>,
    user_filter: Option<&str>,
) -> AppResult<()> {
    let gate_key = format!(
        "creator-live-sockets:{}:{}",
        creator_filter.unwrap_or("*"),
        user_filter.unwrap_or("*"),
    );
    if !state
        .reconciliation_gates
        .should_run(&gate_key, READ_RECONCILIATION_MIN_INTERVAL)
        .await
    {
        return Ok(());
    }
    reconcile_stale_creator_live_socket_sessions_for_read(
        state.db.try_sqlite_adapter()?,
        creator_filter,
        user_filter,
    )
    .await
}

pub(crate) async fn reconcile_stale_creator_live_socket_sessions_for_read(
    pool: &SqlitePool,
    creator_filter: Option<&str>,
    user_filter: Option<&str>,
) -> AppResult<()> {
    let cutoff = active_presence_cutoff();
    let now = Utc::now().to_rfc3339();
    let stale_exists: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT 1
        FROM creator_live_socket_sessions
        WHERE disconnected_at IS NULL
          AND last_seen_at < ?
          AND (?2 IS NULL OR creator_id = ?2)
          AND (?3 IS NULL OR user_id = ?3)
        LIMIT 1
        "#,
    )
    .bind(&cutoff)
    .bind(creator_filter)
    .bind(user_filter)
    .fetch_optional(pool)
    .await?;
    if stale_exists.is_none() {
        return Ok(());
    }

    sqlx::query(
        r#"
        UPDATE creator_live_socket_sessions
        SET disconnected_at = COALESCE(disconnected_at, ?),
            last_seen_at = MIN(last_seen_at, ?)
        WHERE disconnected_at IS NULL
          AND last_seen_at < ?
          AND (?4 IS NULL OR creator_id = ?4)
          AND (?5 IS NULL OR user_id = ?5)
        "#,
    )
    .bind(&now)
    .bind(&cutoff)
    .bind(&cutoff)
    .bind(creator_filter)
    .bind(user_filter)
    .execute(pool)
    .await?;

    Ok(())
}

pub(crate) async fn reconcile_single_creator_live_socket_session(
    state: SharedState,
    creator_id: &str,
    socket_id: &str,
) -> AppResult<CreatorLiveSocketPresenceReconciliationReport> {
    let before = fetch_creator_live_socket_presence_by_id_raw(
        state.db.try_sqlite_adapter()?,
        creator_id,
        socket_id,
    )
    .await?;
    let now = Utc::now().to_rfc3339();
    let cutoff = active_presence_cutoff();
    let mut actions = Vec::new();

    if before.disconnected_at.is_none() && before.last_seen_at < cutoff {
        let updated = sqlx::query(
            "UPDATE creator_live_socket_sessions SET disconnected_at = COALESCE(disconnected_at, ?), last_seen_at = MIN(last_seen_at, ?) WHERE creator_id = ? AND id = ? AND disconnected_at IS NULL",
        )
        .bind(&now)
        .bind(&cutoff)
        .bind(creator_id)
        .bind(socket_id)
        .execute(state.db.try_sqlite_adapter()?)
        .await?;
        if updated.rows_affected() > 0 {
            actions.push(CreatorLiveSocketPresenceReconciliationAction {
                action_type: "socket_disconnected".to_string(),
                target_id: socket_id.to_string(),
                previous_state: Some("connected".to_string()),
                next_state: Some("disconnected".to_string()),
                reason: "creator live socket session exceeded the active presence TTL".to_string(),
                occurred_at: now.clone(),
            });
        }
    }

    let socket_session = fetch_creator_live_socket_presence_by_id_raw(
        state.db.try_sqlite_adapter()?,
        creator_id,
        socket_id,
    )
    .await?;
    if !actions.is_empty() {
        publish_current_creator_live_state(&state, creator_id).await?;
    }
    Ok(CreatorLiveSocketPresenceReconciliationReport {
        creator_id: creator_id.to_string(),
        socket_session_id: socket_id.to_string(),
        reconciled_at: now,
        actions,
        socket_session,
    })
}

pub(crate) fn active_presence_cutoff() -> String {
    (Utc::now() - ChronoDuration::seconds(WS_PRESENCE_TTL_SECONDS)).to_rfc3339()
}
