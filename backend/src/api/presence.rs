use super::*;

pub(super) async fn reconcile_stale_presence_sessions(state: SharedState) -> AppResult<()> {
    let cutoff = active_presence_cutoff();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE live_viewer_sessions SET disconnected_at = COALESCE(disconnected_at, ?), last_seen_at = MIN(last_seen_at, ?) WHERE disconnected_at IS NULL AND last_seen_at < ?",
    )
    .bind(&now)
    .bind(&cutoff)
    .bind(&cutoff)
    .execute(&state.pool)
    .await?;

    reconcile_stale_creator_live_socket_sessions_for_read(&state.pool, None, None).await?;

    let session_rows = sqlx::query(
        "SELECT DISTINCT collaboration_session_id FROM collaboration_socket_sessions WHERE disconnected_at IS NULL AND last_seen_at < ?",
    )
    .bind(&cutoff)
    .fetch_all(&state.pool)
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

pub(super) async fn reconcile_stale_creator_live_socket_sessions_for_read(
    pool: &SqlitePool,
    creator_filter: Option<&str>,
    user_filter: Option<&str>,
) -> AppResult<()> {
    let cutoff = active_presence_cutoff();
    let now = Utc::now().to_rfc3339();

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

pub(super) async fn reconcile_single_creator_live_socket_session(
    state: SharedState,
    creator_id: &str,
    socket_id: &str,
) -> AppResult<CreatorLiveSocketPresenceReconciliationReport> {
    let before =
        fetch_creator_live_socket_presence_by_id_raw(&state.pool, creator_id, socket_id).await?;
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
        .execute(&state.pool)
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

    let socket_session =
        fetch_creator_live_socket_presence_by_id_raw(&state.pool, creator_id, socket_id).await?;
    if !actions.is_empty() {
        publish_creator_live_state(&state, creator_id).await?;
    }
    Ok(CreatorLiveSocketPresenceReconciliationReport {
        creator_id: creator_id.to_string(),
        socket_session_id: socket_id.to_string(),
        reconciled_at: now,
        actions,
        socket_session,
    })
}

pub(super) async fn fetch_chat_messages_for_viewer(
    pool: &SqlitePool,
    stream_id: &str,
    viewer_user_id: Option<&str>,
    limit: i64,
    after_seq: Option<i64>,
) -> AppResult<Vec<ChatMessage>> {
    let rows = match after_seq {
        Some(sequence) if sequence > 0 => {
            sqlx::query(
                r#"
                SELECT id, sequence, user_handle, display_name, color, badges_json, body, sent_at
                FROM chat_messages
                WHERE stream_id = ?
                  AND sequence > ?
                  AND (hidden_by_moderation = 0 OR (user_id = ? AND hidden_by_moderation = 1))
                ORDER BY sequence ASC
                LIMIT ?
                "#,
            )
            .bind(stream_id)
            .bind(sequence)
            .bind(viewer_user_id)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        _ => {
            sqlx::query(
                r#"
                SELECT id, sequence, user_handle, display_name, color, badges_json, body, sent_at
                FROM chat_messages
                WHERE stream_id = ?
                  AND (hidden_by_moderation = 0 OR (user_id = ? AND hidden_by_moderation = 1))
                ORDER BY sequence DESC
                LIMIT ?
                "#,
            )
            .bind(stream_id)
            .bind(viewer_user_id)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
    };

    let mut messages = rows
        .into_iter()
        .map(|row| ChatMessage {
            id: row.get("id"),
            sequence: row.get("sequence"),
            user_handle: row.get("user_handle"),
            display_name: row.get("display_name"),
            color: row.get("color"),
            badges: from_json(row.get::<String, _>("badges_json")).unwrap_or_default(),
            body: row.get("body"),
            sent_at: row.get("sent_at"),
        })
        .collect::<Vec<_>>();
    if after_seq.unwrap_or(0) <= 0 {
        messages.reverse();
    }
    Ok(messages)
}

pub(super) async fn next_chat_message_sequence(
    pool: &SqlitePool,
    stream_id: &str,
) -> AppResult<i64> {
    let row = sqlx::query(
        r#"
        INSERT INTO chat_stream_cursors (stream_id, last_sequence)
        VALUES (?, 1)
        ON CONFLICT(stream_id) DO UPDATE SET
            last_sequence = chat_stream_cursors.last_sequence + 1
        RETURNING last_sequence AS next_sequence
        "#,
    )
    .bind(stream_id)
    .fetch_one(pool)
    .await?;
    Ok(row.get("next_sequence"))
}

pub(super) async fn ensure_stream_exists(pool: &SqlitePool, stream_id: &str) -> AppResult<()> {
    let fresh_cutoff = stale_live_ingest_cutoff();
    let exists = sqlx::query(
        r#"
        SELECT 1
        FROM live_streams ls
        JOIN streamers s ON s.id = ls.streamer_id
        WHERE ls.id = ?
          AND EXISTS (
            SELECT 1
            FROM creator_profiles cp
            JOIN live_ingest_sessions lis ON lis.creator_id = cp.id
            WHERE cp.handle = s.handle
              AND lis.status = 'connected'
              AND lis.last_heartbeat_at >= ?
          )
        "#,
    )
    .bind(&stream_id)
    .bind(&fresh_cutoff)
    .fetch_optional(pool)
    .await?
    .is_some();
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound)
    }
}

async fn stream_viewers(pool: &SqlitePool, stream_id: &str) -> AppResult<i64> {
    let row = sqlx::query("SELECT viewers FROM live_streams WHERE id = ?")
        .bind(&stream_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(row.get("viewers"))
}

pub(super) async fn effective_live_viewer_count(
    pool: &SqlitePool,
    stream_id: &str,
) -> AppResult<i64> {
    let reported = stream_viewers(pool, stream_id).await?;
    let connected = count_active_live_viewer_sessions(pool, stream_id).await?;
    Ok(reported.max(connected))
}

pub(super) async fn count_active_live_viewer_sessions(
    pool: &SqlitePool,
    stream_id: &str,
) -> AppResult<i64> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS count
        FROM (
            SELECT COALESCE('u:' || user_id, 's:' || session_token_hash) AS viewer_key
            FROM live_viewer_sessions
            WHERE stream_id = ?
              AND disconnected_at IS NULL
              AND last_seen_at >= ?
            GROUP BY viewer_key
        ) active_viewers
        "#,
    )
    .bind(stream_id)
    .bind(active_presence_cutoff())
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}

pub(super) async fn count_all_active_live_viewer_sessions(pool: &SqlitePool) -> AppResult<i64> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS count
        FROM (
            SELECT stream_id, COALESCE('u:' || user_id, 's:' || session_token_hash) AS viewer_key
            FROM live_viewer_sessions
            WHERE disconnected_at IS NULL
              AND last_seen_at >= ?
            GROUP BY stream_id, viewer_key
        ) active_viewers
        "#,
    )
    .bind(active_presence_cutoff())
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}

pub(super) async fn fetch_live_viewer_sample_users(
    pool: &SqlitePool,
    stream_id: &str,
    limit: i64,
) -> AppResult<Vec<String>> {
    let rows = sqlx::query(
        r#"
        SELECT u.handle
        FROM live_viewer_sessions lvs
        JOIN users u ON u.id = lvs.user_id
        WHERE lvs.stream_id = ?
          AND lvs.user_id IS NOT NULL
          AND lvs.disconnected_at IS NULL
          AND lvs.last_seen_at >= ?
        GROUP BY u.id, u.handle
        ORDER BY MAX(lvs.last_seen_at) DESC
        LIMIT ?
        "#,
    )
    .bind(stream_id)
    .bind(active_presence_cutoff())
    .bind(limit.max(1))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.get("handle")).collect())
}

pub(super) async fn register_live_viewer_session(
    pool: &SqlitePool,
    stream_id: &str,
    identity: Option<&RequestIdentity>,
    session_token: Option<&str>,
) -> AppResult<(String, bool, String)> {
    let now = Utc::now().to_rfc3339();
    if let Some(token) = session_token.filter(|value| !value.trim().is_empty()) {
        let token_hash = hash_token(token);
        let existing = sqlx::query(
            r#"
            SELECT user_id
            FROM live_viewer_sessions
            WHERE stream_id = ? AND session_token_hash = ?
            ORDER BY connected_at DESC
            LIMIT 1
            "#,
        )
        .bind(&stream_id)
        .bind(&token_hash)
        .fetch_optional(pool)
        .await?;
        if let Some(row) = existing {
            let bound_user_id = row.get::<Option<String>, _>("user_id");
            let requested_user_id = identity.map(|item| item.user_id.as_str());
            if bound_user_id
                .as_deref()
                .is_some_and(|bound| Some(bound) != requested_user_id)
            {
                return Err(AppError::Forbidden);
            }

            let result = sqlx::query(
                r#"
                UPDATE live_viewer_sessions
                SET user_id = COALESCE(?, user_id),
                    connected_at = ?,
                    last_seen_at = ?,
                    disconnected_at = NULL
                WHERE stream_id = ? AND session_token_hash = ?
                "#,
            )
            .bind(requested_user_id)
            .bind(&now)
            .bind(&now)
            .bind(&stream_id)
            .bind(&token_hash)
            .execute(pool)
            .await?;
            if result.rows_affected() > 0 {
                return Ok((token.to_string(), true, now));
            }
        }
    }

    let raw_token = format!(
        "wss_{}_{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    sqlx::query(
        r#"
        INSERT INTO live_viewer_sessions (
            id, stream_id, user_id, session_token_hash, connected_at, last_seen_at, disconnected_at
        ) VALUES (?, ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(format!("lvs-{}", Uuid::new_v4().simple()))
    .bind(stream_id)
    .bind(identity.map(|item| item.user_id.as_str()))
    .bind(hash_token(&raw_token))
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok((raw_token, false, now))
}

pub(super) async fn register_creator_live_socket_session(
    pool: &SqlitePool,
    creator_id: &str,
    user_id: &str,
    session_token: Option<&str>,
) -> AppResult<(String, bool, String)> {
    let now = Utc::now().to_rfc3339();
    if let Some(token) = session_token.filter(|value| !value.trim().is_empty()) {
        let token_hash = hash_token(token);
        let result = sqlx::query(
            r#"
            UPDATE creator_live_socket_sessions
            SET connected_at = ?,
                last_seen_at = ?,
                disconnected_at = NULL
            WHERE creator_id = ? AND user_id = ? AND session_token_hash = ?
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(creator_id)
        .bind(user_id)
        .bind(&token_hash)
        .execute(pool)
        .await?;
        if result.rows_affected() > 0 {
            return Ok((token.to_string(), true, now));
        }
    }

    let raw_token = format!(
        "cws_{}_{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    sqlx::query(
        r#"
        INSERT INTO creator_live_socket_sessions (
            id, creator_id, user_id, session_token_hash, connected_at, last_seen_at, disconnected_at
        ) VALUES (?, ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(format!("cls-{}", Uuid::new_v4().simple()))
    .bind(creator_id)
    .bind(user_id)
    .bind(hash_token(&raw_token))
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok((raw_token, false, now))
}

pub(super) async fn touch_live_viewer_session(
    pool: &SqlitePool,
    stream_id: &str,
    session_token: &str,
    lease_connected_at: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE live_viewer_sessions SET last_seen_at = ?, disconnected_at = NULL WHERE stream_id = ? AND session_token_hash = ? AND connected_at = ?",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(stream_id)
    .bind(hash_token(session_token))
    .bind(lease_connected_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn touch_creator_live_socket_session(
    pool: &SqlitePool,
    creator_id: &str,
    session_token: &str,
    lease_connected_at: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE creator_live_socket_sessions SET last_seen_at = ?, disconnected_at = NULL WHERE creator_id = ? AND session_token_hash = ? AND connected_at = ?",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(creator_id)
    .bind(hash_token(session_token))
    .bind(lease_connected_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn disconnect_live_viewer_session(
    pool: &SqlitePool,
    stream_id: &str,
    session_token: &str,
    lease_connected_at: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE live_viewer_sessions SET last_seen_at = ?, disconnected_at = ? WHERE stream_id = ? AND session_token_hash = ? AND connected_at = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(stream_id)
    .bind(hash_token(session_token))
    .bind(lease_connected_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn disconnect_creator_live_socket_session(
    pool: &SqlitePool,
    creator_id: &str,
    session_token: &str,
    lease_connected_at: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE creator_live_socket_sessions SET last_seen_at = ?, disconnected_at = ? WHERE creator_id = ? AND session_token_hash = ? AND connected_at = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(creator_id)
    .bind(hash_token(session_token))
    .bind(lease_connected_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn register_collaboration_socket_session(
    pool: &SqlitePool,
    session: &CollaborationSessionView,
    identity: &RequestIdentity,
    session_token: Option<&str>,
) -> AppResult<(String, bool, String)> {
    let now = Utc::now().to_rfc3339();
    if let Some(token) = session_token.filter(|value| !value.trim().is_empty()) {
        let token_hash = hash_token(token);
        let result = sqlx::query(
            r#"
            UPDATE collaboration_socket_sessions
            SET participant_id = ?,
                creator_id = COALESCE(?, creator_id),
                connected_at = ?,
                last_seen_at = ?,
                disconnected_at = NULL
            WHERE collaboration_session_id = ? AND user_id = ? AND session_token_hash = ?
            "#,
        )
        .bind(&session.participant.id)
        .bind(identity.creator_id.as_deref())
        .bind(&now)
        .bind(&now)
        .bind(&session.id)
        .bind(&identity.user_id)
        .bind(&token_hash)
        .execute(pool)
        .await?;
        if result.rows_affected() > 0 {
            return Ok((token.to_string(), true, now));
        }
    }

    let raw_token = format!(
        "wsc_{}_{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    sqlx::query(
        r#"
        INSERT INTO collaboration_socket_sessions (
            id, collaboration_session_id, user_id, creator_id, participant_id,
            session_token_hash, connected_at, last_seen_at, disconnected_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(format!("css-{}", Uuid::new_v4().simple()))
    .bind(&session.id)
    .bind(&identity.user_id)
    .bind(identity.creator_id.as_deref())
    .bind(&session.participant.id)
    .bind(hash_token(&raw_token))
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok((raw_token, false, now))
}

pub(super) async fn touch_collaboration_socket_session(
    pool: &SqlitePool,
    session_id: &str,
    session_token: &str,
    lease_connected_at: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE collaboration_socket_sessions SET last_seen_at = ?, disconnected_at = NULL WHERE collaboration_session_id = ? AND session_token_hash = ? AND connected_at = ?",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(session_id)
    .bind(hash_token(session_token))
    .bind(lease_connected_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn disconnect_collaboration_socket_session(
    pool: &SqlitePool,
    session_id: &str,
    session_token: &str,
    lease_connected_at: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE collaboration_socket_sessions SET last_seen_at = ?, disconnected_at = ? WHERE collaboration_session_id = ? AND session_token_hash = ? AND connected_at = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(session_id)
    .bind(hash_token(session_token))
    .bind(lease_connected_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn count_active_collaboration_socket_sessions(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<i64> {
    let row = sqlx::query(
        "SELECT COUNT(DISTINCT participant_id) AS count FROM collaboration_socket_sessions WHERE collaboration_session_id = ? AND disconnected_at IS NULL AND last_seen_at >= ?",
    )
    .bind(session_id)
    .bind(active_presence_cutoff())
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}

pub(super) async fn count_all_active_collaboration_socket_sessions(
    pool: &SqlitePool,
) -> AppResult<i64> {
    let row = sqlx::query(
        "SELECT COUNT(DISTINCT collaboration_session_id || ':' || participant_id) AS count FROM collaboration_socket_sessions WHERE disconnected_at IS NULL AND last_seen_at >= ?",
    )
    .bind(active_presence_cutoff())
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}

pub(super) async fn count_all_active_creator_live_socket_sessions(
    pool: &SqlitePool,
) -> AppResult<i64> {
    let row = sqlx::query(
        "SELECT COUNT(DISTINCT creator_id || ':' || user_id || ':' || session_token_hash) AS count FROM creator_live_socket_sessions WHERE disconnected_at IS NULL AND last_seen_at >= ?",
    )
    .bind(active_presence_cutoff())
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}

pub(super) fn active_presence_cutoff() -> String {
    (Utc::now() - ChronoDuration::seconds(WS_PRESENCE_TTL_SECONDS)).to_rfc3339()
}

pub(super) async fn fetch_auth_sessions(
    pool: &SqlitePool,
    user_id: &str,
    current_session_id: &str,
) -> AppResult<Vec<AuthSession>> {
    let rows = sqlx::query(
        r#"
        SELECT id, label, scopes_json, created_at, expires_at, revoked_at, last_used_at
        FROM auth_sessions
        WHERE user_id = ?
        ORDER BY created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| AuthSession {
            id: row.get("id"),
            label: row.get("label"),
            scopes: from_json(row.get::<String, _>("scopes_json")).unwrap_or_default(),
            created_at: row.get("created_at"),
            expires_at: row.get("expires_at"),
            revoked_at: row.get("revoked_at"),
            last_used_at: row.get("last_used_at"),
            is_current: row.get::<String, _>("id") == current_session_id,
        })
        .collect())
}

pub(super) async fn fetch_continue_watching_entry(
    pool: &SqlitePool,
    user_id: &str,
    content_id: Option<&str>,
    slug_or_id: &str,
) -> AppResult<Option<ContinueWatchingEntry>> {
    let target_content_id = match content_id {
        Some(id) => id.to_string(),
        None => {
            if let Some(row) = sqlx::query("SELECT id FROM series WHERE slug = ?")
                .bind(slug_or_id)
                .fetch_optional(pool)
                .await?
            {
                row.get("id")
            } else if let Some(row) = sqlx::query("SELECT id FROM films WHERE slug = ?")
                .bind(slug_or_id)
                .fetch_optional(pool)
                .await?
            {
                row.get("id")
            } else {
                return Ok(None);
            }
        }
    };

    let row = sqlx::query(
        "SELECT content_id, kind, episode_id, progress_sec, duration_sec, last_watched_at FROM continue_watching WHERE user_id = ? AND content_id = ?",
    )
    .bind(user_id)
    .bind(target_content_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|item| ContinueWatchingEntry {
        content_id: item.get("content_id"),
        kind: item.get("kind"),
        episode_id: item.get("episode_id"),
        progress_sec: item.get("progress_sec"),
        duration_sec: item.get("duration_sec"),
        last_watched_at: item.get("last_watched_at"),
    }))
}

pub(super) async fn upsert_watch_history_entry(
    pool: &SqlitePool,
    user_id: &str,
    content_id: &str,
    kind: &str,
    episode_id: Option<&str>,
    progress_sec: i64,
    duration_sec: i64,
    completed: bool,
    watched_at: &str,
) -> AppResult<()> {
    let completed_at = if completed {
        Some(watched_at.to_string())
    } else {
        None
    };
    sqlx::query(
        r#"
        INSERT INTO user_watch_history (
            user_id, content_id, kind, episode_id, progress_sec, duration_sec,
            completed, completed_at, last_watched_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(user_id, content_id) DO UPDATE SET
            kind = excluded.kind,
            episode_id = excluded.episode_id,
            progress_sec = excluded.progress_sec,
            duration_sec = excluded.duration_sec,
            completed = excluded.completed,
            completed_at = excluded.completed_at,
            last_watched_at = excluded.last_watched_at
        "#,
    )
    .bind(user_id)
    .bind(content_id)
    .bind(kind)
    .bind(episode_id)
    .bind(progress_sec)
    .bind(duration_sec)
    .bind(completed as i64)
    .bind(completed_at)
    .bind(watched_at)
    .execute(pool)
    .await?;
    Ok(())
}
