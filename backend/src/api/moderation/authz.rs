use super::*;

pub(crate) struct LiveStreamOwnerContext {
    pub(crate) creator_id: String,
    pub(crate) current_broadcast_id: Option<String>,
}

pub(crate) async fn fetch_live_stream_owner_context(
    pool: &SqlitePool,
    stream_id: &str,
) -> AppResult<LiveStreamOwnerContext> {
    let fresh_cutoff = stale_live_ingest_cutoff();
    let row = sqlx::query(
        r#"
        SELECT cp.id AS creator_id, cp.current_broadcast_id
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

    Ok(LiveStreamOwnerContext {
        creator_id: row.get("creator_id"),
        current_broadcast_id: row.get("current_broadcast_id"),
    })
}

pub(crate) async fn fetch_live_stream_owner_creator_id(
    pool: &SqlitePool,
    stream_id: &str,
) -> AppResult<String> {
    Ok(fetch_live_stream_owner_context(pool, stream_id)
        .await?
        .creator_id)
}

pub(crate) async fn authorize_live_stream_owner(
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

pub(crate) async fn authorize_live_stream_moderation(
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

pub(crate) async fn can_bypass_live_chat_restrictions(
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

pub(crate) async fn validate_live_moderation_subject(
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
