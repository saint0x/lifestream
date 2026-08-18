use super::*;

pub(super) async fn fetch_live_stream_owner_creator_id(
    pool: &SqlitePool,
    stream_id: &str,
) -> AppResult<String> {
    let row = sqlx::query(
        r#"
        SELECT cp.id AS creator_id
        FROM live_streams ls
        JOIN streamers s ON s.id = ls.streamer_id
        JOIN creator_profiles cp ON cp.handle = s.handle
        WHERE ls.id = ?
        "#,
    )
    .bind(stream_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(row.get("creator_id"))
}

pub(super) async fn authorize_live_stream_owner(
    pool: &SqlitePool,
    stream_id: &str,
    identity: &RequestIdentity,
) -> AppResult<String> {
    let creator_id = fetch_live_stream_owner_creator_id(pool, stream_id).await?;
    if identity.creator_id.as_deref() == Some(creator_id.as_str()) {
        Ok(creator_id)
    } else {
        Err(AppError::Forbidden)
    }
}

pub(super) async fn authorize_live_stream_moderation(
    pool: &SqlitePool,
    stream_id: &str,
    identity: &RequestIdentity,
) -> AppResult<String> {
    let creator_id = fetch_live_stream_owner_creator_id(pool, stream_id).await?;
    if identity.creator_id.as_deref() == Some(creator_id.as_str()) {
        return Ok(creator_id);
    }

    let has_moderator_access =
        sqlx::query("SELECT 1 FROM creator_moderators WHERE creator_id = ? AND user_id = ?")
            .bind(&creator_id)
            .bind(&identity.user_id)
            .fetch_optional(pool)
            .await?
            .is_some();
    if has_moderator_access {
        Ok(creator_id)
    } else {
        Err(AppError::Forbidden)
    }
}

pub(super) async fn can_bypass_live_chat_restrictions(
    pool: &SqlitePool,
    creator_id: &str,
    identity: &RequestIdentity,
) -> AppResult<bool> {
    if identity.creator_id.as_deref() == Some(creator_id) {
        return Ok(true);
    }

    let is_moderator =
        sqlx::query("SELECT 1 FROM creator_moderators WHERE creator_id = ? AND user_id = ?")
            .bind(creator_id)
            .bind(&identity.user_id)
            .fetch_optional(pool)
            .await?
            .is_some();
    Ok(is_moderator)
}

pub(super) async fn validate_live_moderation_subject(
    pool: &SqlitePool,
    stream_id: &str,
    creator_id: &str,
    identity: &RequestIdentity,
    subject_user_id: &str,
) -> AppResult<()> {
    let creator_profile = fetch_creator_profile(pool, creator_id).await?;
    if creator_profile.user_id == subject_user_id {
        return Err(AppError::BadRequest(
            "moderation actions cannot target the stream owner".to_string(),
        ));
    }

    let actor_is_owner = identity.creator_id.as_deref() == Some(creator_id);
    if actor_is_owner {
        return Ok(());
    }

    let subject_is_moderator =
        sqlx::query("SELECT 1 FROM creator_moderators WHERE creator_id = ? AND user_id = ?")
            .bind(creator_id)
            .bind(subject_user_id)
            .fetch_optional(pool)
            .await?
            .is_some();
    if subject_is_moderator {
        return Err(AppError::BadRequest(
            "moderators cannot apply live moderation actions to other moderators".to_string(),
        ));
    }

    let subject_active_action =
        fetch_active_live_moderation_action(pool, stream_id, subject_user_id).await?;
    if matches!(
        subject_active_action
            .as_ref()
            .map(|action| action.action_type.as_str()),
        Some("ban")
    ) {
        return Err(AppError::BadRequest(
            "subject already has an active ban on this stream".to_string(),
        ));
    }

    Ok(())
}

pub(super) async fn fetch_creator_moderators(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<CreatorModerator>> {
    let rows = sqlx::query(
        "SELECT creator_id, user_id, role, created_at FROM creator_moderators WHERE creator_id = ? ORDER BY created_at DESC",
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| CreatorModerator {
            creator_id: row.get("creator_id"),
            user_id: row.get("user_id"),
            role: row.get("role"),
            created_at: row.get("created_at"),
        })
        .collect())
}

pub(super) async fn fetch_creator_moderator(
    pool: &SqlitePool,
    creator_id: &str,
    user_id: &str,
) -> AppResult<CreatorModerator> {
    let row = sqlx::query(
        "SELECT creator_id, user_id, role, created_at FROM creator_moderators WHERE creator_id = ? AND user_id = ?",
    )
    .bind(creator_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(CreatorModerator {
        creator_id: row.get("creator_id"),
        user_id: row.get("user_id"),
        role: row.get("role"),
        created_at: row.get("created_at"),
    })
}

pub(super) async fn fetch_live_moderation_actions(
    pool: &SqlitePool,
    stream_id: &str,
    creator_id: &str,
) -> AppResult<Vec<LiveModerationAction>> {
    reconcile_expired_live_moderation_actions_for_read(
        pool,
        Some(stream_id),
        Some(creator_id),
        None,
        None,
    )
    .await?;
    let rows = sqlx::query(
        r#"
        SELECT id, stream_id, creator_id, subject_user_id, actor_user_id, action_type, reason, state,
               expires_at, created_at, revoked_at
        FROM live_moderation_actions
        WHERE stream_id = ? AND creator_id = ?
        ORDER BY created_at DESC
        "#,
    )
    .bind(stream_id)
    .bind(creator_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(live_moderation_action_from_row)
        .collect())
}

pub(super) async fn fetch_live_moderation_action_by_id(
    pool: &SqlitePool,
    action_id: &str,
) -> AppResult<LiveModerationAction> {
    reconcile_expired_live_moderation_actions_for_read(pool, None, None, None, Some(action_id))
        .await?;
    fetch_live_moderation_action_by_id_raw(pool, action_id).await
}

pub(super) async fn fetch_live_moderation_action_by_id_raw(
    pool: &SqlitePool,
    action_id: &str,
) -> AppResult<LiveModerationAction> {
    let row = sqlx::query(
        r#"
        SELECT id, stream_id, creator_id, subject_user_id, actor_user_id, action_type, reason, state,
               expires_at, created_at, revoked_at
        FROM live_moderation_actions
        WHERE id = ?
        "#,
    )
    .bind(action_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(live_moderation_action_from_row(row))
}

pub(super) async fn fetch_active_live_moderation_action(
    pool: &SqlitePool,
    stream_id: &str,
    subject_user_id: &str,
) -> AppResult<Option<LiveModerationAction>> {
    reconcile_expired_live_moderation_actions_for_read(
        pool,
        Some(stream_id),
        None,
        Some(subject_user_id),
        None,
    )
    .await?;
    let now = Utc::now().to_rfc3339();
    let row = sqlx::query(
        r#"
        SELECT id, stream_id, creator_id, subject_user_id, actor_user_id, action_type, reason, state,
               expires_at, created_at, revoked_at
        FROM live_moderation_actions
        WHERE stream_id = ?
          AND subject_user_id = ?
          AND state = 'active'
          AND (expires_at IS NULL OR expires_at > ?)
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(stream_id)
    .bind(subject_user_id)
    .bind(&now)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(live_moderation_action_from_row))
}

pub(super) async fn fetch_live_stream_reports(
    pool: &SqlitePool,
    stream_id: &str,
) -> AppResult<Vec<LiveStreamReportRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, stream_id, user_id, reason, details, status, resolved_by_user_id, resolution_note, created_at, resolved_at
        FROM live_stream_reports
        WHERE stream_id = ?
        ORDER BY created_at DESC
        "#,
    )
    .bind(stream_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(live_stream_report_from_row).collect())
}

pub(super) async fn fetch_live_stream_report_by_id(
    pool: &SqlitePool,
    report_id: &str,
) -> AppResult<LiveStreamReportRecord> {
    let row = sqlx::query(
        r#"
        SELECT id, stream_id, user_id, reason, details, status, resolved_by_user_id, resolution_note, created_at, resolved_at
        FROM live_stream_reports
        WHERE id = ?
        "#,
    )
    .bind(report_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(live_stream_report_from_row(row))
}

pub(super) async fn fetch_moderation_audit_log(
    pool: &SqlitePool,
    creator_id: &str,
    stream_id: Option<&str>,
) -> AppResult<Vec<ModerationAuditEntry>> {
    let rows = if let Some(stream_id) = stream_id {
        sqlx::query(
            r#"
            SELECT id, creator_id, stream_id, actor_user_id, subject_user_id, event_type, payload_json, created_at
            FROM moderation_audit_log
            WHERE creator_id = ? AND stream_id = ?
            ORDER BY created_at DESC
            "#,
        )
        .bind(creator_id)
        .bind(&stream_id)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT id, creator_id, stream_id, actor_user_id, subject_user_id, event_type, payload_json, created_at
            FROM moderation_audit_log
            WHERE creator_id = ?
            ORDER BY created_at DESC
            "#,
        )
        .bind(creator_id)
        .fetch_all(pool)
        .await?
    };
    Ok(rows
        .into_iter()
        .map(|row| ModerationAuditEntry {
            id: row.get("id"),
            creator_id: row.get("creator_id"),
            stream_id: row.get("stream_id"),
            actor_user_id: row.get("actor_user_id"),
            subject_user_id: row.get("subject_user_id"),
            event_type: row.get("event_type"),
            payload: serde_json::from_str(&row.get::<String, _>("payload_json"))
                .unwrap_or(Value::Null),
            created_at: row.get("created_at"),
        })
        .collect())
}

pub(super) async fn write_moderation_audit_entry(
    pool: &SqlitePool,
    creator_id: &str,
    stream_id: Option<&str>,
    actor_user_id: &str,
    subject_user_id: Option<&str>,
    event_type: &str,
    payload: Value,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO moderation_audit_log (
            id, creator_id, stream_id, actor_user_id, subject_user_id, event_type, payload_json, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(format!("moda-{}", Uuid::new_v4().simple()))
    .bind(creator_id)
    .bind(stream_id)
    .bind(actor_user_id)
    .bind(subject_user_id)
    .bind(event_type)
    .bind(to_json(&payload)?)
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) fn live_moderation_action_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> LiveModerationAction {
    LiveModerationAction {
        id: row.get("id"),
        stream_id: row.get("stream_id"),
        creator_id: row.get("creator_id"),
        subject_user_id: row.get("subject_user_id"),
        actor_user_id: row.get("actor_user_id"),
        action_type: row.get("action_type"),
        reason: row.get("reason"),
        state: row.get("state"),
        expires_at: row.get("expires_at"),
        created_at: row.get("created_at"),
        revoked_at: row.get("revoked_at"),
    }
}

pub(super) fn creator_enforcement_action_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> CreatorEnforcementAction {
    CreatorEnforcementAction {
        id: row.get("id"),
        creator_id: row.get("creator_id"),
        scope: row.get("scope"),
        state: row.get("state"),
        reason: row.get("reason"),
        resolution_note: row.get("resolution_note"),
        created_by_user_id: row.get("created_by_user_id"),
        released_by_user_id: row.get("released_by_user_id"),
        created_at: row.get("created_at"),
        released_at: row.get("released_at"),
        expires_at: row.get("expires_at"),
    }
}

pub(super) fn live_stream_report_from_row(row: sqlx::sqlite::SqliteRow) -> LiveStreamReportRecord {
    LiveStreamReportRecord {
        id: row.get("id"),
        stream_id: row.get("stream_id"),
        user_id: row.get("user_id"),
        reason: row.get("reason"),
        details: row.get("details"),
        status: row.get("status"),
        resolved_by_user_id: row.get("resolved_by_user_id"),
        resolution_note: row.get("resolution_note"),
        created_at: row.get("created_at"),
        resolved_at: row.get("resolved_at"),
    }
}

pub(super) fn validate_creator_moderator_role(role: &str) -> AppResult<()> {
    match role {
        "mod" | "admin" => Ok(()),
        _ => Err(AppError::BadRequest(
            "unsupported moderator role".to_string(),
        )),
    }
}

pub(super) fn validate_slow_mode_seconds(seconds: i64) -> AppResult<()> {
    if !(0..=300).contains(&seconds) {
        return Err(AppError::BadRequest(
            "slowModeSeconds must be between 0 and 300".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn validate_auto_mod_level(level: &str) -> AppResult<()> {
    match level {
        "off" | "standard" | "strict" => Ok(()),
        _ => Err(AppError::BadRequest(
            "autoModLevel must be one of off, standard, or strict".to_string(),
        )),
    }
}

pub(super) fn validate_live_moderation_action_type(action_type: &str) -> AppResult<()> {
    match action_type {
        "mute" | "ban" | "shadowban" => Ok(()),
        _ => Err(AppError::BadRequest(
            "unsupported live moderation action type".to_string(),
        )),
    }
}

pub(super) fn validate_live_report_status(status: &str) -> AppResult<()> {
    match status {
        "open" | "reviewing" | "resolved" | "dismissed" => Ok(()),
        _ => Err(AppError::BadRequest(
            "unsupported live stream report status".to_string(),
        )),
    }
}

pub(super) fn validate_creator_enforcement_scope(scope: &str) -> AppResult<()> {
    match scope.trim() {
        "live_streaming" | "uploads" | "collaboration" | "monetization" | "payouts" => Ok(()),
        other => Err(AppError::BadRequest(format!(
            "unsupported creator enforcement scope: {other}"
        ))),
    }
}
