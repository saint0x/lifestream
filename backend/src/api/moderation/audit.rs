use super::*;

pub(crate) async fn fetch_live_stream_reports(
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

pub(crate) async fn fetch_live_stream_report_by_id(
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

pub(crate) async fn fetch_moderation_audit_log(
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
        .bind(stream_id)
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

pub(crate) async fn write_moderation_audit_entry(
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

pub(crate) fn creator_enforcement_action_from_row(
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

pub(crate) fn live_stream_report_from_row(row: sqlx::sqlite::SqliteRow) -> LiveStreamReportRecord {
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
