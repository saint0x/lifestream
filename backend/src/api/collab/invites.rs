use super::*;

pub(crate) async fn fetch_collaboration_invites_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<Vec<CollaborationInvite>> {
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, host_creator_id, invitee_user_id, invitee_creator_id, role, state,
               mirror_to_guest_channel, message, created_at, responded_at, expires_at
        FROM collaboration_invites
        WHERE session_id = ?
        ORDER BY created_at ASC
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| CollaborationInvite {
            id: row.get("id"),
            session_id: row.get("session_id"),
            host_creator_id: row.get("host_creator_id"),
            invitee_user_id: row.get("invitee_user_id"),
            invitee_creator_id: row.get("invitee_creator_id"),
            role: row.get("role"),
            state: row.get("state"),
            mirror_to_guest_channel: row.get::<i64, _>("mirror_to_guest_channel") == 1,
            message: row.get("message"),
            created_at: row.get("created_at"),
            responded_at: row.get("responded_at"),
            expires_at: row.get("expires_at"),
        })
        .collect())
}

pub(crate) async fn fetch_pending_collaboration_invites_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<Vec<CollaborationInvite>> {
    Ok(fetch_collaboration_invites_for_session(pool, session_id)
        .await?
        .into_iter()
        .filter(|invite| invite.state == "pending")
        .collect())
}

pub(crate) async fn fetch_collaboration_invite_by_id(
    pool: &SqlitePool,
    invite_id: &str,
) -> AppResult<CollaborationInvite> {
    let row = sqlx::query(
        r#"
        SELECT id, session_id, host_creator_id, invitee_user_id, invitee_creator_id, role, state,
               mirror_to_guest_channel, message, created_at, responded_at, expires_at
        FROM collaboration_invites
        WHERE id = ?
        "#,
    )
    .bind(invite_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(CollaborationInvite {
        id: row.get("id"),
        session_id: row.get("session_id"),
        host_creator_id: row.get("host_creator_id"),
        invitee_user_id: row.get("invitee_user_id"),
        invitee_creator_id: row.get("invitee_creator_id"),
        role: row.get("role"),
        state: row.get("state"),
        mirror_to_guest_channel: row.get::<i64, _>("mirror_to_guest_channel") == 1,
        message: row.get("message"),
        created_at: row.get("created_at"),
        responded_at: row.get("responded_at"),
        expires_at: row.get("expires_at"),
    })
}

pub(crate) async fn fetch_collaboration_invites_for_user(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<Vec<CollaborationInvite>> {
    let rows = sqlx::query(
        r#"
        SELECT id
        FROM collaboration_invites
        WHERE invitee_user_id = ?
          AND state = 'pending'
        ORDER BY created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    let mut invites = Vec::with_capacity(rows.len());
    for row in rows {
        let invite_id: String = row.get("id");
        invites.push(fetch_collaboration_invite_by_id(pool, &invite_id).await?);
    }
    Ok(invites)
}

pub(crate) async fn has_pending_collaboration_invite_for_user(
    pool: &SqlitePool,
    session_id: &str,
    user_id: &str,
) -> AppResult<bool> {
    let row = sqlx::query(
        r#"
        SELECT 1
        FROM collaboration_invites
        WHERE session_id = ? AND invitee_user_id = ? AND state = 'pending'
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some())
}
