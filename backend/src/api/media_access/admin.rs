use super::*;

pub(crate) async fn fetch_admin_playback_sessions(
    pool: &SqlitePool,
    creator_filter: Option<&str>,
    content_filter: Option<&str>,
    state_filter: Option<&str>,
    limit: i64,
) -> AppResult<Vec<AdminPlaybackSessionRecord>> {
    reconcile_playback_sessions_for_read(pool, creator_filter, content_filter, None).await?;
    let limit = limit.clamp(1, 250);
    let now = Utc::now().to_rfc3339();
    let rows = match state_filter {
        Some("active") | Some("valid") => {
            sqlx::query(
                r#"
                SELECT id
                FROM playback_sessions
                WHERE (?1 IS NULL OR creator_id = ?1)
                  AND (?2 IS NULL OR content_id = ?2)
                  AND expires_at > ?3
                ORDER BY created_at DESC
                LIMIT ?4
                "#,
            )
            .bind(creator_filter)
            .bind(content_filter)
            .bind(&now)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        Some("expired") | Some("invalid") => {
            sqlx::query(
                r#"
                SELECT id
                FROM playback_sessions
                WHERE (?1 IS NULL OR creator_id = ?1)
                  AND (?2 IS NULL OR content_id = ?2)
                  AND expires_at <= ?3
                ORDER BY created_at DESC
                LIMIT ?4
                "#,
            )
            .bind(creator_filter)
            .bind(content_filter)
            .bind(&now)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        Some(_) | None => {
            sqlx::query(
                r#"
                SELECT id
                FROM playback_sessions
                WHERE (?1 IS NULL OR creator_id = ?1)
                  AND (?2 IS NULL OR content_id = ?2)
                ORDER BY created_at DESC
                LIMIT ?3
                "#,
            )
            .bind(creator_filter)
            .bind(content_filter)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
    };

    let mut sessions = Vec::with_capacity(rows.len());
    for row in rows {
        let session_id: String = row.get("id");
        sessions.push(fetch_admin_playback_session_record(pool, &session_id).await?);
    }
    Ok(sessions)
}

pub(crate) async fn fetch_admin_playback_session_record(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<AdminPlaybackSessionRecord> {
    reconcile_playback_sessions_for_read(pool, None, None, Some(session_id)).await?;
    let mut session = fetch_playback_session_record_by_id(pool, session_id).await?;
    let now = Utc::now().to_rfc3339();
    let mut active = session.expires_at > now;
    let valid_access = if active {
        validate_existing_playback_session_access(pool, &session, None).await?
    } else {
        false
    };
    if active && !valid_access {
        expire_playback_session_by_id(pool, session_id).await?;
        session = fetch_playback_session_record_by_id(pool, session_id).await?;
        active = false;
    }
    Ok(AdminPlaybackSessionRecord {
        session: playback_session_from_record(&session),
        user_id: session.user_id.clone(),
        creator_id: session.creator_id.clone(),
        asset_id: session.asset_id.clone(),
        active,
        valid_access,
    })
}
