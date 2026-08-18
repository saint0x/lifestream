use super::*;

pub(crate) async fn end_collaboration_session_internal_raw(
    state: &SharedState,
    session: &CollaborationSession,
    actor_user_id: Option<String>,
    payload: Value,
) -> AppResult<CollaborationSession> {
    if session.status == "ended" {
        return fetch_collaboration_session_by_id(&state.pool, &session.id).await;
    }

    let now = Utc::now().to_rfc3339();
    let revoked_invites =
        fetch_pending_collaboration_invites_for_session(&state.pool, &session.id).await?;
    sqlx::query(
        "UPDATE collaboration_sessions SET status = 'ended', ended_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(&session.id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE collaboration_participants SET state = CASE WHEN state IN ('left', 'removed') THEN state ELSE 'left' END, left_at = COALESCE(left_at, ?), updated_at = ? WHERE session_id = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(&session.id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE collaboration_invites SET state = CASE WHEN state = 'pending' THEN 'revoked' ELSE state END, responded_at = COALESCE(responded_at, ?) WHERE session_id = ?",
    )
    .bind(&now)
    .bind(&session.id)
    .execute(&state.pool)
    .await?;
    publish_collaboration_invite_revoked_events_raw(
        state,
        &session.id,
        actor_user_id.clone(),
        &revoked_invites,
        &now,
        "session_ended",
    )
    .await?;
    revoke_collaboration_mirror_grants_for_session_raw(
        state,
        &session.id,
        actor_user_id.clone(),
        &now,
        "session_ended",
    )
    .await?;
    publish_collaboration_reconciliation_event(
        state,
        &session.id,
        actor_user_id,
        None,
        "session_ended",
        json!({
            "hostCreatorId": session.host_creator_id,
            "sourceBroadcastId": session.source_broadcast_id,
            "endedAt": now,
            "details": payload,
        }),
    )
    .await?;

    fetch_collaboration_session_by_id(&state.pool, &session.id).await
}

pub(crate) async fn end_collaboration_session_internal(
    state: &SharedState,
    session: &CollaborationSession,
    actor_user_id: Option<String>,
    payload: Value,
) -> AppResult<CollaborationSession> {
    if session.status == "ended" {
        return fetch_collaboration_session_by_id(&state.pool, &session.id).await;
    }

    let now = Utc::now().to_rfc3339();
    let revoked_invites =
        fetch_pending_collaboration_invites_for_session(&state.pool, &session.id).await?;
    sqlx::query(
        "UPDATE collaboration_sessions SET status = 'ended', ended_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(&session.id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE collaboration_participants SET state = CASE WHEN state IN ('left', 'removed') THEN state ELSE 'left' END, left_at = COALESCE(left_at, ?), updated_at = ? WHERE session_id = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(&session.id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE collaboration_invites SET state = CASE WHEN state = 'pending' THEN 'revoked' ELSE state END, responded_at = COALESCE(responded_at, ?) WHERE session_id = ?",
    )
    .bind(&now)
    .bind(&session.id)
    .execute(&state.pool)
    .await?;
    publish_collaboration_invite_revoked_events(
        state,
        &session.id,
        actor_user_id.clone(),
        &revoked_invites,
        &now,
        "session_ended",
    )
    .await?;
    revoke_collaboration_mirror_grants_for_session(
        state,
        &session.id,
        actor_user_id.clone(),
        &now,
        "session_ended",
    )
    .await?;
    publish_collaboration_event(
        state,
        &session.id,
        actor_user_id,
        None,
        "session_ended",
        json!({
            "hostCreatorId": session.host_creator_id,
            "sourceBroadcastId": session.source_broadcast_id,
            "endedAt": now,
            "details": payload,
        }),
    )
    .await?;

    fetch_collaboration_session_by_id(&state.pool, &session.id).await
}
