use super::*;

pub(crate) async fn reconcile_playback_sessions_for_user(
    pool: &SqlitePool,
    user_id: &str,
    creator_id: Option<&str>,
    upload_id: Option<&str>,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let rows = match (creator_id, upload_id) {
        (Some(creator_id), Some(upload_id)) => {
            sqlx::query(
                r#"
        SELECT id, user_id, creator_id, asset_id, content_id, content_kind, access_scope,
               auth_session_id, created_at, expires_at, last_used_at
        FROM playback_sessions
        WHERE user_id = ? AND creator_id = ? AND content_id = ? AND expires_at > ?
                ORDER BY expires_at ASC
                "#,
            )
            .bind(user_id)
            .bind(creator_id)
            .bind(upload_id)
            .bind(&now)
            .fetch_all(pool)
            .await?
        }
        (Some(creator_id), None) => {
            sqlx::query(
                r#"
        SELECT id, user_id, creator_id, asset_id, content_id, content_kind, access_scope,
               auth_session_id, created_at, expires_at, last_used_at
        FROM playback_sessions
        WHERE user_id = ? AND creator_id = ? AND expires_at > ?
                ORDER BY expires_at ASC
                "#,
            )
            .bind(user_id)
            .bind(creator_id)
            .bind(&now)
            .fetch_all(pool)
            .await?
        }
        (None, Some(upload_id)) => {
            sqlx::query(
                r#"
        SELECT id, user_id, creator_id, asset_id, content_id, content_kind, access_scope,
               auth_session_id, created_at, expires_at, last_used_at
        FROM playback_sessions
        WHERE user_id = ? AND content_id = ? AND expires_at > ?
                ORDER BY expires_at ASC
                "#,
            )
            .bind(user_id)
            .bind(upload_id)
            .bind(&now)
            .fetch_all(pool)
            .await?
        }
        (None, None) => {
            sqlx::query(
                r#"
        SELECT id, user_id, creator_id, asset_id, content_id, content_kind, access_scope,
               auth_session_id, created_at, expires_at, last_used_at
        FROM playback_sessions
        WHERE user_id = ? AND expires_at > ?
                ORDER BY expires_at ASC
                "#,
            )
            .bind(user_id)
            .bind(&now)
            .fetch_all(pool)
            .await?
        }
    };

    for row in rows {
        let session = playback_session_record_from_row(row);
        if !validate_existing_playback_session_access(pool, &session, None).await? {
            expire_playback_session_by_id(pool, &session.id).await?;
        }
    }

    Ok(())
}

pub(crate) async fn reconcile_playback_sessions_for_read(
    pool: &SqlitePool,
    creator_id: Option<&str>,
    content_id: Option<&str>,
    session_id: Option<&str>,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"
        SELECT id, user_id, creator_id, asset_id, content_id, content_kind, access_scope,
               auth_session_id, created_at, expires_at, last_used_at
        FROM playback_sessions
        WHERE expires_at > ?
          AND (?2 IS NULL OR creator_id = ?2)
          AND (?3 IS NULL OR content_id = ?3)
          AND (?4 IS NULL OR id = ?4)
        ORDER BY created_at DESC
        "#,
    )
    .bind(&now)
    .bind(creator_id)
    .bind(content_id)
    .bind(session_id)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let session = playback_session_record_from_row(row);
        if !validate_existing_playback_session_access(pool, &session, None).await? {
            expire_playback_session_by_id(pool, &session.id).await?;
        }
    }

    Ok(())
}

pub(crate) async fn reconcile_single_playback_session(
    state: SharedState,
    session_id: &str,
) -> AppResult<PlaybackReconciliationReport> {
    let now = Utc::now().to_rfc3339();
    let session =
        fetch_playback_session_record_by_id(state.db.sqlite_adapter(), session_id).await?;
    let mut actions = Vec::new();

    if session.expires_at > now
        && !validate_existing_playback_session_access(state.db.sqlite_adapter(), &session, None)
            .await?
    {
        expire_playback_session_by_id(state.db.sqlite_adapter(), &session.id).await?;
        actions.push(PlaybackReconciliationAction {
            action_type: "session_invalidated".to_string(),
            target_id: session.id.clone(),
            previous_state: Some("active".to_string()),
            next_state: Some("invalid".to_string()),
            reason: "playback session no longer satisfied access requirements".to_string(),
            occurred_at: now.clone(),
        });
    }

    let record = fetch_admin_playback_session_record(&state.db, session_id).await?;
    Ok(PlaybackReconciliationReport {
        session_id: session_id.to_string(),
        reconciled_at: now,
        actions,
        record,
    })
}

pub(crate) async fn reconcile_invalid_playback_sessions(state: SharedState) -> AppResult<()> {
    reconcile_playback_sessions_for_read(state.db.sqlite_adapter(), None, None, None).await
}
