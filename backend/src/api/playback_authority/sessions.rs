use super::*;

pub(crate) async fn fetch_upload_playback_target(
    pool: &SqlitePool,
    upload_id: &str,
) -> AppResult<UploadPlaybackTarget> {
    let row = sqlx::query(
        r#"
        SELECT creator_id
        FROM uploads
        WHERE id = ?
        "#,
    )
    .bind(upload_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let creator_id: String = row.get("creator_id");
    let upload = fetch_upload_by_id(pool, &creator_id, upload_id).await?;
    let asset = fetch_media_asset_by_upload_id(pool, &creator_id, upload_id).await?;
    if asset.status != "ready" && asset.status != "published" {
        return Err(AppError::BadRequest(
            "asset is not ready for playback".to_string(),
        ));
    }
    Ok(UploadPlaybackTarget {
        creator_id,
        upload,
        asset,
    })
}

pub(crate) async fn fetch_live_stream_playback_target(
    pool: &SqlitePool,
    stream_id: &str,
) -> AppResult<LivePlaybackTarget> {
    let fresh_cutoff = stale_live_ingest_cutoff();
    let row = sqlx::query(
        r#"
        SELECT ls.id, ls.title, ls.playback_asset_id, ls.poster_relative_path, ls.playback_relative_path, cp.id AS creator_id
        FROM live_streams ls
        JOIN streamers s ON s.id = ls.streamer_id
        JOIN creator_profiles cp ON cp.handle = s.handle
        WHERE ls.id = ?
          AND (
            EXISTS (
                SELECT 1
                FROM live_ingest_sessions lis
                WHERE lis.creator_id = cp.id
                  AND lis.status = 'connected'
                  AND lis.last_heartbeat_at >= ?
            )
            OR EXISTS (
                SELECT 1
                FROM collaboration_mirror_pickups cmp
                JOIN live_ingest_sessions lis
                  ON lis.creator_id = cmp.host_creator_id
                 AND lis.broadcast_id = cmp.source_broadcast_id
                WHERE cmp.guest_creator_id = cp.id
                  AND cmp.guest_broadcast_id = cp.current_broadcast_id
                  AND cmp.state = 'active'
                  AND lis.status = 'connected'
                  AND lis.last_heartbeat_at >= ?
            )
          )
        "#,
    )
    .bind(stream_id)
    .bind(&fresh_cutoff)
    .bind(&fresh_cutoff)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let playback_asset_id = row
        .get::<Option<String>, _>("playback_asset_id")
        .ok_or_else(|| AppError::BadRequest("live playback asset unavailable".to_string()))?;
    let playback_relative_path = row
        .get::<Option<String>, _>("playback_relative_path")
        .ok_or_else(|| AppError::BadRequest("live playback manifest unavailable".to_string()))?;

    let asset_exists =
        sqlx::query("SELECT 1 FROM media_assets WHERE id = ? AND status IN ('ready', 'published')")
            .bind(&playback_asset_id)
            .fetch_optional(pool)
            .await?
            .is_some();
    if !asset_exists {
        return Err(AppError::BadRequest(
            "live playback asset is not ready".to_string(),
        ));
    }

    let creator_id: String = row.get("creator_id");
    let asset = fetch_media_asset_by_id_any_creator(pool, &playback_asset_id).await?;

    Ok(LivePlaybackTarget {
        creator_id,
        asset_id: playback_asset_id,
        title: row.get("title"),
        poster_relative_path: row.get("poster_relative_path"),
        playback_relative_path,
        asset,
    })
}

pub(crate) fn playback_session_from_record(session: &PlaybackSessionRecord) -> PlaybackSession {
    PlaybackSession {
        id: session.id.clone(),
        content_id: session.content_id.clone(),
        content_kind: session.content_kind.clone(),
        access_scope: session.access_scope.clone(),
        created_at: session.created_at.clone(),
        expires_at: session.expires_at.clone(),
        last_used_at: session.last_used_at.clone(),
    }
}

fn playback_session_record_from_row(row: sqlx::sqlite::SqliteRow) -> PlaybackSessionRecord {
    PlaybackSessionRecord {
        id: row.get("id"),
        auth_session_id: row.get("auth_session_id"),
        user_id: row.get("user_id"),
        creator_id: row.get("creator_id"),
        asset_id: row.get("asset_id"),
        content_id: row.get("content_id"),
        content_kind: row.get("content_kind"),
        access_scope: row.get("access_scope"),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        last_used_at: row.get("last_used_at"),
    }
}

pub(crate) async fn fetch_playback_session_record_by_id(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<PlaybackSessionRecord> {
    let row = sqlx::query(
        r#"
        SELECT id, user_id, creator_id, asset_id, content_id, content_kind, access_scope,
               auth_session_id,
               created_at, expires_at, last_used_at
        FROM playback_sessions
        WHERE id = ?
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(playback_session_record_from_row(row))
}

pub(crate) async fn validate_playback_session_record(
    pool: &SqlitePool,
    session_id: &str,
    playback_token: &str,
) -> AppResult<PlaybackSessionRecord> {
    let now = Utc::now().to_rfc3339();
    let row = sqlx::query(
        r#"
        SELECT id, user_id, creator_id, asset_id, content_id, content_kind, access_scope,
               auth_session_id,
               created_at, expires_at, last_used_at
        FROM playback_sessions
        WHERE id = ? AND token_hash = ? AND expires_at > ?
        "#,
    )
    .bind(session_id)
    .bind(crate::auth::hash_token(playback_token))
    .bind(&now)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    let session = playback_session_record_from_row(row);
    if !validate_existing_playback_session_access(pool, &session, None).await? {
        expire_playback_session_by_id(pool, &session.id).await?;
        return Err(AppError::Unauthorized);
    }

    sqlx::query("UPDATE playback_sessions SET last_used_at = ? WHERE id = ?")
        .bind(&now)
        .bind(session_id)
        .execute(pool)
        .await?;

    Ok(PlaybackSessionRecord {
        last_used_at: now,
        ..session
    })
}

pub(crate) async fn validate_playback_session_record_for_path(
    pool: &SqlitePool,
    playback_token: &str,
    relative_path: &str,
) -> AppResult<PlaybackSessionRecord> {
    let now = Utc::now().to_rfc3339();
    let row = sqlx::query(
        r#"
        SELECT id, user_id, creator_id, asset_id, content_id, content_kind, access_scope,
               auth_session_id,
               created_at, expires_at, last_used_at
        FROM playback_sessions
        WHERE token_hash = ? AND expires_at > ?
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(crate::auth::hash_token(playback_token))
    .bind(&now)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    let session = playback_session_record_from_row(row);
    if relative_path.is_empty() {
        return Err(AppError::Unauthorized);
    }
    if !validate_existing_playback_session_access(pool, &session, Some(relative_path)).await? {
        expire_playback_session_by_id(pool, &session.id).await?;
        return Err(AppError::Unauthorized);
    }

    sqlx::query("UPDATE playback_sessions SET last_used_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&session.id)
        .execute(pool)
        .await?;

    Ok(PlaybackSessionRecord {
        last_used_at: now,
        ..session
    })
}

pub(crate) async fn validate_existing_playback_session_access(
    pool: &SqlitePool,
    session: &PlaybackSessionRecord,
    relative_path: Option<&str>,
) -> AppResult<bool> {
    if !playback_session_auth_binding_is_active(pool, session).await? {
        return Ok(false);
    }
    if session.content_kind == "live" {
        let target = match fetch_live_stream_playback_target(pool, &session.content_id).await {
            Ok(target) => target,
            Err(_) => return Ok(false),
        };
        if target.asset_id != session.asset_id {
            return Ok(false);
        }
        if let Some(relative_path) = relative_path {
            if !path_allowed_for_paths(
                relative_path,
                &target.playback_relative_path,
                target.poster_relative_path.as_deref(),
                Some(&target.playback_relative_path),
                &[],
            ) {
                return Err(AppError::Forbidden);
            }
        }
        return Ok(true);
    }

    let target = match fetch_upload_playback_target(pool, &session.content_id).await {
        Ok(target) => target,
        Err(_) => return Ok(false),
    };
    if target.asset.id != session.asset_id {
        return Ok(false);
    }
    if let Some(relative_path) = relative_path {
        if !playback_path_allowed_for_asset(&target.asset, relative_path) {
            return Err(AppError::Forbidden);
        }
    }

    let identity = playback_request_identity_for_session(pool, session).await?;
    Ok(
        resolve_upload_playback_access(pool, identity.as_ref(), &target)
            .await
            .is_ok(),
    )
}

async fn playback_session_auth_binding_is_active(
    pool: &SqlitePool,
    session: &PlaybackSessionRecord,
) -> AppResult<bool> {
    let Some(auth_session_id) = session.auth_session_id.as_deref() else {
        return Ok(true);
    };
    let now = Utc::now().to_rfc3339();
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS count
        FROM auth_sessions
        WHERE id = ?
          AND revoked_at IS NULL
          AND (expires_at IS NULL OR expires_at > ?)
        "#,
    )
    .bind(auth_session_id)
    .bind(&now)
    .fetch_one(pool)
    .await?;
    let count: i64 = row.get("count");
    Ok(count == 1)
}

async fn playback_request_identity_for_session(
    pool: &SqlitePool,
    session: &PlaybackSessionRecord,
) -> AppResult<Option<RequestIdentity>> {
    let Some(user_id) = session.user_id.clone() else {
        return Ok(None);
    };
    let creator_id = fetch_creator_id_for_user(pool, &user_id).await?;
    Ok(Some(RequestIdentity {
        session_id: session.id.clone(),
        user_id,
        creator_id,
        scopes: Vec::new(),
    }))
}

pub(crate) async fn expire_playback_session_by_id(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE playback_sessions SET expires_at = ?, last_used_at = ? WHERE id = ? AND expires_at > ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(session_id)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn expire_playback_sessions_for_upload(
    pool: &SqlitePool,
    upload_id: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE playback_sessions SET expires_at = ?, last_used_at = ? WHERE content_id = ? AND expires_at > ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(upload_id)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn expire_playback_sessions_for_auth_session(
    pool: &SqlitePool,
    auth_session_id: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE playback_sessions SET expires_at = ?, last_used_at = ? WHERE auth_session_id = ? AND expires_at > ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(auth_session_id)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

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
    let session = fetch_playback_session_record_by_id(&state.pool, session_id).await?;
    let mut actions = Vec::new();

    if session.expires_at > now
        && !validate_existing_playback_session_access(&state.pool, &session, None).await?
    {
        expire_playback_session_by_id(&state.pool, &session.id).await?;
        actions.push(PlaybackReconciliationAction {
            action_type: "session_invalidated".to_string(),
            target_id: session.id.clone(),
            previous_state: Some("active".to_string()),
            next_state: Some("invalid".to_string()),
            reason: "playback session no longer satisfied access requirements".to_string(),
            occurred_at: now.clone(),
        });
    }

    let record = fetch_admin_playback_session_record(&state.pool, session_id).await?;
    Ok(PlaybackReconciliationReport {
        session_id: session_id.to_string(),
        reconciled_at: now,
        actions,
        record,
    })
}

pub(crate) async fn reconcile_invalid_playback_sessions(state: SharedState) -> AppResult<()> {
    reconcile_playback_sessions_for_read(&state.pool, None, None, None).await
}
