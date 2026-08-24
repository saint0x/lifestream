use super::*;

const HOST_READ_RECONCILIATION_MIN_INTERVAL: Duration = Duration::from_millis(750);

pub(crate) fn validate_collaboration_socket_access(
    session: &CollaborationSessionView,
) -> AppResult<()> {
    if session.status == "ended" {
        return Err(AppError::Forbidden);
    }
    if matches!(session.participant.state.as_str(), "left" | "removed") {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

pub(crate) async fn fetch_current_collaboration_socket_session_view(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
) -> AppResult<CollaborationSessionView> {
    reconcile_collaboration_session_expiry_for_read(state, session_id).await?;
    if let Ok(session) = fetch_collaboration_session_for_participant(
        state.db.sqlite_adapter(),
        &identity.user_id,
        session_id,
    )
    .await
    {
        validate_collaboration_socket_access(&session)?;
        return Ok(session);
    }

    let creator_id = identity.creator_id.as_deref().ok_or(AppError::Forbidden)?;
    let host_session =
        fetch_collaboration_session_for_host(state.db.sqlite_adapter(), creator_id, session_id)
            .await?;
    let host =
        fetch_collaboration_host_summary(state.db.sqlite_adapter(), &host_session.host_creator_id)
            .await?;
    let host_view = collaboration_session_view_for_host(host_session, host)?;
    validate_collaboration_socket_access(&host_view)?;
    Ok(host_view)
}

pub(crate) async fn reconcile_collaboration_session_expiry_for_read(
    state: &SharedState,
    session_id: &str,
) -> AppResult<()> {
    if !collaboration_session_requires_reconciliation_for_read(
        state.db.sqlite_adapter(),
        session_id,
    )
    .await?
    {
        return Ok(());
    }
    let _ = reconcile_single_collaboration_session(state.clone(), session_id).await?;
    Ok(())
}

pub(crate) async fn reconcile_collaboration_expiry_for_host_read(
    state: &SharedState,
    creator_id: &str,
) -> AppResult<()> {
    let gate_key = format!("collab-host-read:{creator_id}");
    if !state
        .reconciliation_gates
        .should_run(&gate_key, HOST_READ_RECONCILIATION_MIN_INTERVAL)
        .await
    {
        return Ok(());
    }
    let now = Utc::now().to_rfc3339();
    let cutoff = active_presence_cutoff();
    let expired_exists: Option<String> = sqlx::query_scalar(
        r#"
        SELECT session_id
        FROM (
            SELECT i.session_id AS session_id
            FROM collaboration_invites i
            JOIN collaboration_sessions s ON s.id = i.session_id
            WHERE s.host_creator_id = ?
              AND i.state = 'pending'
              AND i.expires_at <= ?
            UNION
            SELECT g.session_id AS session_id
            FROM collaboration_mirror_grants g
            JOIN collaboration_sessions s ON s.id = g.session_id
            WHERE s.host_creator_id = ?
              AND g.state IN ('issued', 'active')
              AND g.expires_at <= ?
            UNION
            SELECT css.collaboration_session_id AS session_id
            FROM collaboration_socket_sessions css
            JOIN collaboration_sessions s ON s.id = css.collaboration_session_id
            WHERE s.host_creator_id = ?
              AND css.disconnected_at IS NULL
              AND css.last_seen_at < ?
            UNION
            SELECT s.id AS session_id
            FROM collaboration_sessions s
            JOIN broadcasts b ON b.id = s.source_broadcast_id
            WHERE s.host_creator_id = ?
              AND s.status != 'ended'
              AND b.status NOT IN ('ready', 'live')
        )
        LIMIT 1
        "#,
    )
    .bind(creator_id)
    .bind(&now)
    .bind(creator_id)
    .bind(&now)
    .bind(creator_id)
    .bind(&cutoff)
    .bind(creator_id)
    .fetch_optional(state.db.sqlite_adapter())
    .await?;
    if expired_exists.is_none() {
        return Ok(());
    }

    let rows = sqlx::query(
        r#"
        SELECT DISTINCT session_id
        FROM (
            SELECT i.session_id AS session_id
            FROM collaboration_invites i
            JOIN collaboration_sessions s ON s.id = i.session_id
            WHERE s.host_creator_id = ?
              AND i.state = 'pending'
              AND i.expires_at <= ?
            UNION
            SELECT g.session_id AS session_id
            FROM collaboration_mirror_grants g
            JOIN collaboration_sessions s ON s.id = g.session_id
            WHERE s.host_creator_id = ?
              AND g.state IN ('issued', 'active')
              AND g.expires_at <= ?
            UNION
            SELECT css.collaboration_session_id AS session_id
            FROM collaboration_socket_sessions css
            JOIN collaboration_sessions s ON s.id = css.collaboration_session_id
            WHERE s.host_creator_id = ?
              AND css.disconnected_at IS NULL
              AND css.last_seen_at < ?
            UNION
            SELECT s.id AS session_id
            FROM collaboration_sessions s
            JOIN broadcasts b ON b.id = s.source_broadcast_id
            WHERE s.host_creator_id = ?
              AND s.status != 'ended'
              AND b.status NOT IN ('ready', 'live')
        )
        "#,
    )
    .bind(creator_id)
    .bind(&now)
    .bind(creator_id)
    .bind(&now)
    .bind(creator_id)
    .bind(&cutoff)
    .bind(creator_id)
    .fetch_all(state.db.sqlite_adapter())
    .await?;
    for row in rows {
        let session_id: String = row.get("session_id");
        reconcile_collaboration_session_expiry_for_read(state, &session_id).await?;
    }
    Ok(())
}

pub(crate) async fn reconcile_collaboration_expiry_for_participant_read(
    state: &SharedState,
    user_id: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let cutoff = active_presence_cutoff();
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT session_id
        FROM (
            SELECT i.session_id AS session_id
            FROM collaboration_invites i
            WHERE i.invitee_user_id = ?
              AND i.state = 'pending'
              AND i.expires_at <= ?
            UNION
            SELECT g.session_id AS session_id
            FROM collaboration_mirror_grants g
            JOIN collaboration_participants p ON p.id = g.participant_id
            WHERE p.user_id = ?
              AND g.state IN ('issued', 'active')
              AND g.expires_at <= ?
            UNION
            SELECT css.collaboration_session_id AS session_id
            FROM collaboration_socket_sessions css
            WHERE css.user_id = ?
              AND css.disconnected_at IS NULL
              AND css.last_seen_at < ?
            UNION
            SELECT p.session_id AS session_id
            FROM collaboration_participants p
            JOIN collaboration_sessions s ON s.id = p.session_id
            JOIN broadcasts b ON b.id = s.source_broadcast_id
            WHERE p.user_id = ?
              AND s.status != 'ended'
              AND b.status NOT IN ('ready', 'live')
        )
        "#,
    )
    .bind(user_id)
    .bind(&now)
    .bind(user_id)
    .bind(&now)
    .bind(user_id)
    .bind(&cutoff)
    .bind(user_id)
    .fetch_all(state.db.sqlite_adapter())
    .await?;
    for row in rows {
        let session_id: String = row.get("session_id");
        reconcile_collaboration_session_expiry_for_read(state, &session_id).await?;
    }
    Ok(())
}

async fn collaboration_session_requires_reconciliation_for_read(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<bool> {
    let now = Utc::now().to_rfc3339();
    let cutoff = active_presence_cutoff();
    let requires_reconciliation: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT 1
        FROM collaboration_sessions s
        LEFT JOIN broadcasts b
          ON b.id = s.source_broadcast_id
        WHERE s.id = ?
          AND (
              EXISTS (
                  SELECT 1
                  FROM collaboration_invites i
                  WHERE i.session_id = s.id
                    AND i.state = 'pending'
                    AND i.expires_at <= ?
              )
              OR EXISTS (
                  SELECT 1
                  FROM collaboration_mirror_grants g
                  WHERE g.session_id = s.id
                    AND g.state IN ('issued', 'active')
                    AND g.expires_at <= ?
              )
              OR EXISTS (
                  SELECT 1
                  FROM collaboration_socket_sessions css
                  WHERE css.collaboration_session_id = s.id
                    AND css.disconnected_at IS NULL
                    AND css.last_seen_at < ?
              )
              OR (
                  s.status != 'ended'
                  AND b.status NOT IN ('ready', 'live')
              )
          )
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .bind(&now)
    .bind(&now)
    .bind(&cutoff)
    .fetch_optional(pool)
    .await?;

    Ok(requires_reconciliation.is_some())
}
