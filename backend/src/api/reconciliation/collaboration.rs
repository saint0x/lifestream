use super::*;

pub(crate) async fn reconcile_expired_collaboration_invites(state: SharedState) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT session_id
        FROM collaboration_invites
        WHERE state = 'pending' AND expires_at <= ?
        "#,
    )
    .bind(&now)
    .fetch_all(state.db.sqlite_adapter())
    .await?;

    for row in rows {
        let session_id: String = row.get("session_id");
        let _ = expire_pending_collaboration_invites_for_session(&state, &session_id, &now).await?;
    }

    Ok(())
}

pub(crate) async fn reconcile_expired_collaboration_mirror_grants(
    state: SharedState,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT session_id
        FROM collaboration_mirror_grants
        WHERE state IN ('issued', 'active') AND expires_at <= ?
        "#,
    )
    .bind(&now)
    .fetch_all(state.db.sqlite_adapter())
    .await?;

    for row in rows {
        let session_id: String = row.get("session_id");
        let _ = expire_collaboration_mirror_grants_for_session(&state, &session_id, &now).await?;
    }

    Ok(())
}
