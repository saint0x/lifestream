use super::*;

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
    database: &crate::db::Database,
    session_id: &str,
    playback_token: &str,
) -> AppResult<PlaybackSessionRecord> {
    let now = Utc::now().to_rfc3339();
    let session = database
        .fetch_active_playback_session_record(session_id, playback_token, &now)
        .await?;
    if !validate_existing_playback_session_access_for_database(database, &session, None).await? {
        expire_playback_session_by_id_for_database(database, &session.id).await?;
        return Err(AppError::Unauthorized);
    }

    Ok(session)
}

pub(crate) async fn validate_playback_session_record_for_path(
    database: &crate::db::Database,
    playback_token: &str,
    relative_path: &str,
) -> AppResult<PlaybackSessionRecord> {
    if relative_path.is_empty() {
        return Err(AppError::Unauthorized);
    }
    let now = Utc::now().to_rfc3339();
    let session = database
        .fetch_latest_active_playback_session_record_by_token(playback_token, &now)
        .await?;
    if !validate_existing_playback_session_access_for_database(
        database,
        &session,
        Some(relative_path),
    )
    .await?
    {
        expire_playback_session_by_id_for_database(database, &session.id).await?;
        return Err(AppError::Unauthorized);
    }

    Ok(session)
}

async fn validate_existing_playback_session_access_for_database(
    database: &crate::db::Database,
    session: &PlaybackSessionRecord,
    relative_path: Option<&str>,
) -> AppResult<bool> {
    if let Ok(pool) = database.try_postgres_adapter() {
        return validate_postgres_playback_session_access(pool, database, session, relative_path)
            .await;
    }
    validate_existing_playback_session_access(
        database.try_sqlite_adapter()?,
        session,
        relative_path,
    )
    .await
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
    Ok(resolve_upload_playback_access(
        &crate::db::Database::from_sqlite(pool.clone()),
        identity.as_ref(),
        &target,
    )
    .await
    .is_ok())
}

async fn validate_postgres_playback_session_access(
    pool: &sqlx::PgPool,
    database: &crate::db::Database,
    session: &PlaybackSessionRecord,
    relative_path: Option<&str>,
) -> AppResult<bool> {
    if !postgres_playback_session_auth_binding_is_active(pool, session).await? {
        return Ok(false);
    }
    if session.content_kind == "live" {
        return Ok(false);
    }

    let target =
        match fetch_upload_playback_target_for_database(database, &session.content_id).await {
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

    let identity = postgres_playback_request_identity_for_session(pool, session).await?;
    Ok(
        resolve_upload_playback_access(database, identity.as_ref(), &target)
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

async fn postgres_playback_session_auth_binding_is_active(
    pool: &sqlx::PgPool,
    session: &PlaybackSessionRecord,
) -> AppResult<bool> {
    let Some(auth_session_id) = session.auth_session_id.as_deref() else {
        return Ok(true);
    };
    let now = Utc::now().to_rfc3339();
    let row = sqlx::query(
        r#"
        SELECT COUNT(*)::BIGINT AS count
        FROM auth_sessions
        WHERE id = $1
          AND revoked_at IS NULL
          AND (expires_at IS NULL OR expires_at > $2)
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

async fn postgres_playback_request_identity_for_session(
    pool: &sqlx::PgPool,
    session: &PlaybackSessionRecord,
) -> AppResult<Option<RequestIdentity>> {
    let Some(user_id) = session.user_id.clone() else {
        return Ok(None);
    };
    let row = sqlx::query("SELECT id FROM creator_profiles WHERE user_id = $1")
        .bind(&user_id)
        .fetch_optional(pool)
        .await?;
    Ok(Some(RequestIdentity {
        session_id: session.id.clone(),
        user_id,
        creator_id: row.map(|row| row.get("id")),
        scopes: Vec::new(),
    }))
}

async fn expire_playback_session_by_id_for_database(
    database: &crate::db::Database,
    session_id: &str,
) -> AppResult<()> {
    if let Ok(pool) = database.try_postgres_adapter() {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE playback_sessions SET expires_at = $1, last_used_at = $2 WHERE id = $3 AND expires_at > $4",
        )
        .bind(&now)
        .bind(&now)
        .bind(session_id)
        .bind(&now)
        .execute(pool)
        .await?;
        return Ok(());
    }
    expire_playback_session_by_id(database.try_sqlite_adapter()?, session_id).await
}
