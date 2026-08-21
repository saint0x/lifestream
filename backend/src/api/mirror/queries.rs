use super::*;
use sqlx::sqlite::SqliteRow;

fn grant_from_row(row: &SqliteRow) -> CollaborationMirrorGrant {
    CollaborationMirrorGrant {
        id: row.get("id"),
        session_id: row.get("session_id"),
        participant_id: row.get("participant_id"),
        host_creator_id: row.get("host_creator_id"),
        guest_creator_id: row.get("guest_creator_id"),
        scope: row.get("scope"),
        state: row.get("state"),
        publish_to_host: row.get::<i64, _>("publish_to_host") == 1,
        mirror_to_guest_channel: row.get::<i64, _>("mirror_to_guest_channel") == 1,
        issued_at: row.get("issued_at"),
        activated_at: row.get("activated_at"),
        revoked_at: row.get("revoked_at"),
        expires_at: row.get("expires_at"),
    }
}

fn pickup_from_row(row: &SqliteRow) -> CollaborationMirrorPickup {
    CollaborationMirrorPickup {
        id: row.get("id"),
        session_id: row.get("session_id"),
        participant_id: row.get("participant_id"),
        grant_id: row.get("grant_id"),
        host_creator_id: row.get("host_creator_id"),
        guest_creator_id: row.get("guest_creator_id"),
        source_broadcast_id: row.get("source_broadcast_id"),
        guest_broadcast_id: row.get("guest_broadcast_id"),
        state: row.get("state"),
        activated_at: row.get("activated_at"),
        updated_at: row.get("updated_at"),
        ended_at: row.get("ended_at"),
    }
}

pub(crate) async fn fetch_collaboration_mirror_grants_for_participant(
    pool: &SqlitePool,
    participant_id: &str,
) -> AppResult<Vec<CollaborationMirrorGrant>> {
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, participant_id, host_creator_id, guest_creator_id, scope, state,
               publish_to_host, mirror_to_guest_channel, issued_at, activated_at, revoked_at, expires_at
        FROM collaboration_mirror_grants
        WHERE participant_id = ?
        ORDER BY issued_at DESC
        "#,
    )
    .bind(participant_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| grant_from_row(&row)).collect())
}

pub(crate) async fn fetch_collaboration_mirror_grants_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<Vec<CollaborationMirrorGrant>> {
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, participant_id, host_creator_id, guest_creator_id, scope, state,
               publish_to_host, mirror_to_guest_channel, issued_at, activated_at, revoked_at, expires_at
        FROM collaboration_mirror_grants
        WHERE session_id = ?
        ORDER BY issued_at DESC
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| grant_from_row(&row)).collect())
}

#[cfg(test)]
pub(crate) async fn fetch_collaboration_mirror_pickups_for_participant(
    pool: &SqlitePool,
    participant_id: &str,
) -> AppResult<Vec<CollaborationMirrorPickup>> {
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, participant_id, grant_id, host_creator_id, guest_creator_id,
               source_broadcast_id, guest_broadcast_id, state, activated_at, updated_at, ended_at
        FROM collaboration_mirror_pickups
        WHERE participant_id = ?
        ORDER BY activated_at DESC
        "#,
    )
    .bind(participant_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| pickup_from_row(&row)).collect())
}

pub(crate) async fn fetch_collaboration_mirror_pickups_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<Vec<CollaborationMirrorPickup>> {
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, participant_id, grant_id, host_creator_id, guest_creator_id,
               source_broadcast_id, guest_broadcast_id, state, activated_at, updated_at, ended_at
        FROM collaboration_mirror_pickups
        WHERE session_id = ?
        ORDER BY activated_at DESC
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| pickup_from_row(&row)).collect())
}

pub(crate) async fn fetch_collaboration_mirror_grant_by_id(
    pool: &SqlitePool,
    grant_id: &str,
) -> AppResult<CollaborationMirrorGrant> {
    let row = sqlx::query(
        r#"
        SELECT id, session_id, participant_id, host_creator_id, guest_creator_id, scope, state,
               publish_to_host, mirror_to_guest_channel, issued_at, activated_at, revoked_at, expires_at
        FROM collaboration_mirror_grants
        WHERE id = ?
        "#,
    )
    .bind(grant_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(grant_from_row(&row))
}

pub(crate) async fn fetch_revocable_collaboration_mirror_grants_for_participant(
    pool: &SqlitePool,
    participant_id: &str,
) -> AppResult<Vec<CollaborationMirrorGrant>> {
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, participant_id, host_creator_id, guest_creator_id, scope, state,
               publish_to_host, mirror_to_guest_channel, issued_at, activated_at, revoked_at, expires_at
        FROM collaboration_mirror_grants
        WHERE participant_id = ?
          AND state IN ('issued', 'active')
        ORDER BY issued_at DESC
        "#,
    )
    .bind(participant_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| grant_from_row(&row)).collect())
}

pub(crate) async fn fetch_revocable_collaboration_mirror_grants_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<Vec<CollaborationMirrorGrant>> {
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, participant_id, host_creator_id, guest_creator_id, scope, state,
               publish_to_host, mirror_to_guest_channel, issued_at, activated_at, revoked_at, expires_at
        FROM collaboration_mirror_grants
        WHERE session_id = ?
          AND state IN ('issued', 'active')
        ORDER BY issued_at DESC
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| grant_from_row(&row)).collect())
}

pub(crate) async fn fetch_active_collaboration_mirror_pickup_for_grant(
    pool: &SqlitePool,
    grant_id: &str,
) -> AppResult<Option<CollaborationMirrorPickup>> {
    let row = sqlx::query(
        r#"
        SELECT id, session_id, participant_id, grant_id, host_creator_id, guest_creator_id,
               source_broadcast_id, guest_broadcast_id, state, activated_at, updated_at, ended_at
        FROM collaboration_mirror_pickups
        WHERE grant_id = ? AND state = 'active'
        ORDER BY activated_at DESC
        LIMIT 1
        "#,
    )
    .bind(grant_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| pickup_from_row(&row)))
}

pub(crate) async fn fetch_active_collaboration_mirror_pickups_for_grants(
    pool: &SqlitePool,
    grants: &[CollaborationMirrorGrant],
) -> AppResult<Vec<CollaborationMirrorPickup>> {
    let mut pickups = Vec::new();
    for grant in grants {
        if let Some(pickup) =
            fetch_active_collaboration_mirror_pickup_for_grant(pool, &grant.id).await?
        {
            pickups.push(pickup);
        }
    }
    Ok(pickups)
}
