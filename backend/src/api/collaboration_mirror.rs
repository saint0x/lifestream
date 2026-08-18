use super::collaboration_events::{
    publish_collaboration_event, publish_collaboration_mirror_grant_revoked_events,
    publish_collaboration_mirror_grant_revoked_events_raw,
};
use super::*;

pub(super) async fn fetch_collaboration_mirror_grants_for_participant(
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
    Ok(rows
        .into_iter()
        .map(|row| CollaborationMirrorGrant {
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
        })
        .collect())
}

pub(super) async fn fetch_collaboration_mirror_grants_for_session(
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
    Ok(rows
        .into_iter()
        .map(|row| CollaborationMirrorGrant {
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
        })
        .collect())
}

pub(super) async fn fetch_collaboration_mirror_pickups_for_participant(
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
    Ok(rows
        .into_iter()
        .map(|row| CollaborationMirrorPickup {
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
        })
        .collect())
}

pub(super) async fn fetch_collaboration_mirror_pickups_for_session(
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
    Ok(rows
        .into_iter()
        .map(|row| CollaborationMirrorPickup {
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
        })
        .collect())
}

pub(super) async fn fetch_collaboration_mirror_grant_by_id(
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
    Ok(CollaborationMirrorGrant {
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
    })
}

pub(super) async fn fetch_revocable_collaboration_mirror_grants_for_participant(
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
    Ok(rows
        .into_iter()
        .map(|row| CollaborationMirrorGrant {
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
        })
        .collect())
}

pub(super) async fn fetch_revocable_collaboration_mirror_grants_for_session(
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
    Ok(rows
        .into_iter()
        .map(|row| CollaborationMirrorGrant {
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
        })
        .collect())
}

pub(super) async fn fetch_active_collaboration_mirror_pickup_for_grant(
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
    Ok(row.map(|row| CollaborationMirrorPickup {
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
    }))
}

pub(super) async fn fetch_active_collaboration_mirror_pickups_for_grants(
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

async fn fetch_source_pickup_viewer_count(
    pool: &SqlitePool,
    pickup: &CollaborationMirrorPickup,
) -> AppResult<i64> {
    if let Some(session) =
        fetch_active_live_ingest_session_unreconciled(pool, &pickup.host_creator_id).await?
    {
        if session.broadcast_id == pickup.source_broadcast_id {
            return Ok(session.viewers);
        }
    }

    let host_creator = fetch_creator_profile(pool, &pickup.host_creator_id).await?;
    let stream_id = format!("lv-{}-live", host_creator.handle);
    if let Ok(stream) = fetch_live_stream_by_id(pool, &stream_id).await {
        return Ok(stream.viewers);
    }

    Ok(0)
}

async fn sync_mirror_pickup_playback_metadata(
    pool: &SqlitePool,
    pickup: &CollaborationMirrorPickup,
    guest_handle: &str,
) -> AppResult<()> {
    let host_creator = fetch_creator_profile(pool, &pickup.host_creator_id).await?;
    let host_stream_id = format!("lv-{}-live", host_creator.handle);
    let row = sqlx::query(
        r#"
        SELECT playback_asset_id, poster_relative_path, playback_relative_path
        FROM live_streams
        WHERE id = ?
        "#,
    )
    .bind(&host_stream_id)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = row {
        sqlx::query(
            r#"
            UPDATE live_streams
            SET playback_asset_id = ?,
                poster_relative_path = ?,
                playback_relative_path = ?
            WHERE id = ?
            "#,
        )
        .bind(row.get::<Option<String>, _>("playback_asset_id"))
        .bind(row.get::<Option<String>, _>("poster_relative_path"))
        .bind(row.get::<Option<String>, _>("playback_relative_path"))
        .bind(format!("lv-{}-live", guest_handle))
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn ensure_guest_broadcast_available_for_mirror_pickup(
    pool: &SqlitePool,
    session: &CollaborationSession,
    participant: &CollaborationParticipant,
    guest_creator_id: &str,
) -> AppResult<Broadcast> {
    let guest_profile = normalize_creator_live_profile(
        pool,
        guest_creator_id,
        fetch_broadcasts(pool, guest_creator_id).await?,
    )
    .await?;
    if let Some(current_broadcast_id) = guest_profile.current_broadcast_id.as_deref() {
        let existing_pickups =
            fetch_collaboration_mirror_pickups_for_session(pool, &session.id).await?;
        if let Some(existing_pickup) = existing_pickups.iter().find(|pickup| {
            pickup.participant_id == participant.id
                && pickup.guest_creator_id == guest_creator_id
                && pickup.state == "active"
                && pickup.guest_broadcast_id == current_broadcast_id
        }) {
            return fetch_broadcast_by_id(
                pool,
                guest_creator_id,
                &existing_pickup.guest_broadcast_id,
            )
            .await;
        }

        let existing_broadcast =
            fetch_broadcast_by_id(pool, guest_creator_id, current_broadcast_id).await?;
        if matches!(existing_broadcast.status.as_str(), "ready" | "live") {
            return Err(AppError::BadRequest(
                "guest creator already has another active or pending broadcast".to_string(),
            ));
        }
    }

    let source_broadcast =
        fetch_broadcast_by_id(pool, &session.host_creator_id, &session.source_broadcast_id).await?;
    let guest_broadcast = Broadcast {
        id: format!("bcast-collab-{}", Uuid::new_v4().simple()),
        title: source_broadcast.title.clone(),
        category: source_broadcast.category.clone(),
        tags: source_broadcast.tags.clone(),
        status: source_broadcast.status.clone(),
        started_at: source_broadcast.started_at.clone(),
        ended_at: None,
        duration_sec: None,
        peak_viewers: 0,
        average_viewers: 0,
        chat_messages: 0,
        new_followers: 0,
        new_subscribers: 0,
        revenue: 0.0,
        thumbnail: source_broadcast.thumbnail.clone(),
        is_mature: source_broadcast.is_mature,
    };

    sqlx::query(
        r#"
        INSERT INTO broadcasts (
            id, creator_id, title, category, tags_json, status, started_at, ended_at, duration_sec,
            peak_viewers, average_viewers, chat_messages, new_followers, new_subscribers,
            revenue, thumbnail, is_mature
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&guest_broadcast.id)
    .bind(guest_creator_id)
    .bind(&guest_broadcast.title)
    .bind(&guest_broadcast.category)
    .bind(to_json(&guest_broadcast.tags)?)
    .bind(&guest_broadcast.status)
    .bind(&guest_broadcast.started_at)
    .bind(&guest_broadcast.ended_at)
    .bind(&guest_broadcast.duration_sec)
    .bind(guest_broadcast.peak_viewers)
    .bind(guest_broadcast.average_viewers)
    .bind(guest_broadcast.chat_messages)
    .bind(guest_broadcast.new_followers)
    .bind(guest_broadcast.new_subscribers)
    .bind(guest_broadcast.revenue)
    .bind(&guest_broadcast.thumbnail)
    .bind(guest_broadcast.is_mature as i64)
    .execute(pool)
    .await?;

    Ok(guest_broadcast)
}

async fn sync_collaboration_mirror_pickup_broadcast_state(
    pool: &SqlitePool,
    pickup: &CollaborationMirrorPickup,
) -> AppResult<()> {
    let guest_creator = fetch_creator_profile(pool, &pickup.guest_creator_id).await?;
    let source_broadcast =
        fetch_broadcast_by_id(pool, &pickup.host_creator_id, &pickup.source_broadcast_id).await?;
    let target_status = if source_broadcast.status == "live" {
        "live"
    } else {
        "ready"
    };

    sqlx::query(
        "UPDATE broadcasts SET title = ?, category = ?, tags_json = ?, status = ?, started_at = ?, ended_at = NULL, duration_sec = NULL, thumbnail = ?, is_mature = ? WHERE id = ? AND creator_id = ?",
    )
    .bind(&source_broadcast.title)
    .bind(&source_broadcast.category)
    .bind(to_json(&source_broadcast.tags)?)
    .bind(target_status)
    .bind(&source_broadcast.started_at)
    .bind(&source_broadcast.thumbnail)
    .bind(source_broadcast.is_mature as i64)
    .bind(&pickup.guest_broadcast_id)
    .bind(&pickup.guest_creator_id)
    .execute(pool)
    .await?;

    sqlx::query(
        "UPDATE creator_profiles SET live_status = ?, current_broadcast_id = ? WHERE id = ?",
    )
    .bind(target_status)
    .bind(&pickup.guest_broadcast_id)
    .bind(&pickup.guest_creator_id)
    .execute(pool)
    .await?;

    sqlx::query("UPDATE streamers SET is_live = ? WHERE handle = ?")
        .bind((target_status == "live") as i64)
        .bind(&guest_creator.handle)
        .execute(pool)
        .await?;

    if target_status == "live" {
        let viewers = fetch_source_pickup_viewer_count(pool, pickup).await?;
        let refreshed_guest_broadcast =
            fetch_broadcast_by_id(pool, &pickup.guest_creator_id, &pickup.guest_broadcast_id)
                .await?;
        ensure_live_stream_row(pool, &guest_creator, &refreshed_guest_broadcast, viewers).await?;
        sync_mirror_pickup_playback_metadata(pool, pickup, &guest_creator.handle).await?;
    } else {
        sqlx::query("DELETE FROM live_streams WHERE id = ?")
            .bind(format!("lv-{}-live", guest_creator.handle))
            .execute(pool)
            .await?;
    }
    Ok(())
}

async fn activate_collaboration_mirror_pickup(
    state: &SharedState,
    session: &CollaborationSession,
    participant: &CollaborationParticipant,
    grant: &CollaborationMirrorGrant,
    activated_at: &str,
) -> AppResult<CollaborationMirrorPickup> {
    let guest_creator_id = participant.creator_id.clone().ok_or_else(|| {
        AppError::BadRequest(
            "collaboration mirror pickup requires the participant to have a creator profile"
                .to_string(),
        )
    })?;
    let guest_broadcast = ensure_guest_broadcast_available_for_mirror_pickup(
        &state.pool,
        session,
        participant,
        &guest_creator_id,
    )
    .await?;
    let pickup_id = format!("colmp-{}", Uuid::new_v4().simple());
    sqlx::query(
        r#"
        INSERT INTO collaboration_mirror_pickups (
            id, session_id, participant_id, grant_id, host_creator_id, guest_creator_id,
            source_broadcast_id, guest_broadcast_id, state, activated_at, updated_at, ended_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, NULL)
        "#,
    )
    .bind(&pickup_id)
    .bind(&session.id)
    .bind(&participant.id)
    .bind(&grant.id)
    .bind(&session.host_creator_id)
    .bind(&guest_creator_id)
    .bind(&session.source_broadcast_id)
    .bind(&guest_broadcast.id)
    .bind(activated_at)
    .bind(activated_at)
    .execute(&state.pool)
    .await?;

    let pickup = fetch_active_collaboration_mirror_pickup_for_grant(&state.pool, &grant.id)
        .await?
        .ok_or_else(|| {
            AppError::Internal("activated collaboration mirror pickup is missing".to_string())
        })?;
    sync_collaboration_mirror_pickup_broadcast_state(&state.pool, &pickup).await?;
    publish_authoritative_creator_live_state(state, &pickup.guest_creator_id).await?;
    Ok(pickup)
}

pub(super) async fn deactivate_collaboration_mirror_pickups_for_grants(
    state: &SharedState,
    grants: &[CollaborationMirrorGrant],
    terminal_state: &str,
    ended_at: &str,
) -> AppResult<()> {
    let pickups = fetch_active_collaboration_mirror_pickups_for_grants(&state.pool, grants).await?;
    let mut guest_creator_ids = Vec::new();
    for pickup in pickups {
        guest_creator_ids.push(pickup.guest_creator_id.clone());
        sqlx::query(
            "UPDATE collaboration_mirror_pickups SET state = ?, updated_at = ?, ended_at = COALESCE(ended_at, ?) WHERE id = ?",
        )
        .bind(terminal_state)
        .bind(ended_at)
        .bind(ended_at)
        .bind(&pickup.id)
        .execute(&state.pool)
        .await?;

        let guest_creator = fetch_creator_profile(&state.pool, &pickup.guest_creator_id).await?;
        let guest_broadcast = fetch_broadcast_by_id(
            &state.pool,
            &pickup.guest_creator_id,
            &pickup.guest_broadcast_id,
        )
        .await?;
        let started_at = chrono::DateTime::parse_from_rfc3339(&guest_broadcast.started_at)
            .map_err(|_| {
                AppError::BadRequest(
                    "invalid collaboration mirror pickup broadcast timestamp".to_string(),
                )
            })?
            .with_timezone(&Utc);
        let ended = chrono::DateTime::parse_from_rfc3339(ended_at)
            .map_err(|_| {
                AppError::BadRequest(
                    "invalid collaboration mirror pickup end timestamp".to_string(),
                )
            })?
            .with_timezone(&Utc);
        let duration_sec = (ended - started_at).num_seconds().max(0);

        sqlx::query(
            "UPDATE broadcasts SET status = 'ended', ended_at = COALESCE(ended_at, ?), duration_sec = COALESCE(duration_sec, ?) WHERE id = ? AND creator_id = ?",
        )
        .bind(ended_at)
        .bind(duration_sec)
        .bind(&pickup.guest_broadcast_id)
        .bind(&pickup.guest_creator_id)
        .execute(&state.pool)
        .await?;

        sqlx::query("DELETE FROM live_streams WHERE id = ?")
            .bind(format!("lv-{}-live", guest_creator.handle))
            .execute(&state.pool)
            .await?;
        sqlx::query("UPDATE streamers SET is_live = 0 WHERE handle = ?")
            .bind(&guest_creator.handle)
            .execute(&state.pool)
            .await?;
        reset_creator_live_operational_metrics(&state.pool, &pickup.guest_creator_id).await?;
        let guest_broadcasts = fetch_broadcasts(&state.pool, &pickup.guest_creator_id).await?;
        let _ =
            normalize_creator_live_profile(&state.pool, &pickup.guest_creator_id, guest_broadcasts)
                .await?;
    }
    publish_creator_live_states_for_creators(state, guest_creator_ids).await?;
    Ok(())
}

pub(super) async fn sync_active_collaboration_mirror_pickups_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<()> {
    let pickups = fetch_collaboration_mirror_pickups_for_session(pool, session_id).await?;
    for pickup in pickups
        .into_iter()
        .filter(|pickup| pickup.state == "active")
    {
        sync_collaboration_mirror_pickup_broadcast_state(pool, &pickup).await?;
    }
    Ok(())
}

async fn publish_creator_live_states_for_creators(
    state: &SharedState,
    creator_ids: impl IntoIterator<Item = String>,
) -> AppResult<()> {
    let mut unique = std::collections::BTreeSet::new();
    for creator_id in creator_ids {
        if unique.insert(creator_id.clone()) {
            publish_raw_creator_live_state(state, &creator_id).await?;
        }
    }
    Ok(())
}

pub(super) async fn sync_active_collaboration_mirror_pickups_for_session_and_publish(
    state: &SharedState,
    session_id: &str,
) -> AppResult<()> {
    sync_active_collaboration_mirror_pickups_for_session(&state.pool, session_id).await?;
    let pickups = fetch_collaboration_mirror_pickups_for_session(&state.pool, session_id).await?;
    publish_creator_live_states_for_creators(
        state,
        pickups
            .into_iter()
            .filter(|pickup| pickup.state == "active")
            .map(|pickup| pickup.guest_creator_id),
    )
    .await
}

pub(super) async fn revoke_collaboration_mirror_grants_for_participant(
    state: &SharedState,
    session_id: &str,
    participant_id: &str,
    actor_user_id: Option<String>,
    revoked_at: &str,
    reason: &str,
) -> AppResult<()> {
    let grants =
        fetch_revocable_collaboration_mirror_grants_for_participant(&state.pool, participant_id)
            .await?;
    if grants.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "UPDATE collaboration_mirror_grants SET state = 'revoked', revoked_at = COALESCE(revoked_at, ?) WHERE participant_id = ? AND state IN ('issued', 'active')",
    )
    .bind(revoked_at)
    .bind(participant_id)
    .execute(&state.pool)
    .await?;
    publish_collaboration_mirror_grant_revoked_events(
        state,
        session_id,
        actor_user_id,
        &grants,
        revoked_at,
        reason,
    )
    .await?;
    deactivate_collaboration_mirror_pickups_for_grants(state, &grants, "revoked", revoked_at)
        .await?;
    Ok(())
}

pub(super) async fn revoke_collaboration_mirror_grants_for_session(
    state: &SharedState,
    session_id: &str,
    actor_user_id: Option<String>,
    revoked_at: &str,
    reason: &str,
) -> AppResult<()> {
    let grants =
        fetch_revocable_collaboration_mirror_grants_for_session(&state.pool, session_id).await?;
    if grants.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "UPDATE collaboration_mirror_grants SET state = 'revoked', revoked_at = COALESCE(revoked_at, ?) WHERE session_id = ? AND state IN ('issued', 'active')",
    )
    .bind(revoked_at)
    .bind(session_id)
    .execute(&state.pool)
    .await?;
    publish_collaboration_mirror_grant_revoked_events(
        state,
        session_id,
        actor_user_id,
        &grants,
        revoked_at,
        reason,
    )
    .await?;
    deactivate_collaboration_mirror_pickups_for_grants(state, &grants, "revoked", revoked_at)
        .await?;
    Ok(())
}

pub(super) async fn revoke_collaboration_mirror_grants_for_session_raw(
    state: &SharedState,
    session_id: &str,
    actor_user_id: Option<String>,
    revoked_at: &str,
    reason: &str,
) -> AppResult<()> {
    let grants =
        fetch_revocable_collaboration_mirror_grants_for_session(&state.pool, session_id).await?;
    if grants.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "UPDATE collaboration_mirror_grants SET state = 'revoked', revoked_at = COALESCE(revoked_at, ?) WHERE session_id = ? AND state IN ('issued', 'active')",
    )
    .bind(revoked_at)
    .bind(session_id)
    .execute(&state.pool)
    .await?;
    publish_collaboration_mirror_grant_revoked_events_raw(
        state,
        session_id,
        actor_user_id,
        &grants,
        revoked_at,
        reason,
    )
    .await?;
    deactivate_collaboration_mirror_pickups_for_grants(state, &grants, "revoked", revoked_at)
        .await?;
    Ok(())
}

pub(super) async fn issue_mirror_grant_for_participant(
    state: &SharedState,
    session: &CollaborationSession,
    participant: &CollaborationParticipant,
    actor_user_id: &str,
) -> AppResult<CollaborationMirrorGrant> {
    if participant.role == "host" {
        return Err(AppError::BadRequest(
            "hosts do not receive collaboration mirror grants".to_string(),
        ));
    }
    if participant.state == "left" || participant.state == "removed" {
        return Err(AppError::BadRequest(
            "inactive participants cannot receive collaboration mirror grants".to_string(),
        ));
    }
    if session.status != "active" {
        return Err(AppError::BadRequest(
            "collaboration mirror grants can only be issued for active sessions".to_string(),
        ));
    }
    if participant.state != "live" {
        return Err(AppError::BadRequest(
            "collaboration mirror grants can only be issued for live participants".to_string(),
        ));
    }
    if !participant.mirror_to_guest_channel {
        return Err(AppError::BadRequest(
            "participant is not enabled for mirrored guest channel pickup".to_string(),
        ));
    }
    let guest_creator_id = participant.creator_id.clone().ok_or_else(|| {
        AppError::BadRequest(
            "collaboration mirror grants require the participant to have a creator profile"
                .to_string(),
        )
    })?;
    let issued_at = Utc::now().to_rfc3339();
    let expires_at = (Utc::now() + chrono::Duration::hours(6)).to_rfc3339();
    let raw_token = format!("colmg-{}", Uuid::new_v4().simple());
    let grant_id = format!("colm-{}", Uuid::new_v4().simple());

    revoke_collaboration_mirror_grants_for_participant(
        state,
        &session.id,
        &participant.id,
        Some(actor_user_id.to_string()),
        &issued_at,
        "grant_reissued",
    )
    .await?;
    sqlx::query(
        r#"
        INSERT INTO collaboration_mirror_grants (
            id, session_id, participant_id, host_creator_id, guest_creator_id, scope, state,
            publish_to_host, mirror_to_guest_channel, token_hash, issued_at, activated_at, revoked_at, expires_at
        ) VALUES (?, ?, ?, ?, ?, 'mirror_pickup', 'issued', ?, ?, ?, ?, NULL, NULL, ?)
        "#,
    )
    .bind(&grant_id)
    .bind(&session.id)
    .bind(&participant.id)
    .bind(&session.host_creator_id)
    .bind(&guest_creator_id)
    .bind(participant.publish_to_host as i64)
    .bind(participant.mirror_to_guest_channel as i64)
    .bind(hash_token(&raw_token))
    .bind(&issued_at)
    .bind(&expires_at)
    .execute(&state.pool)
    .await?;
    publish_collaboration_event(
        state,
        &session.id,
        Some(actor_user_id.to_string()),
        Some(participant.id.clone()),
        "mirror_grant_issued",
        json!({
            "grantId": grant_id,
            "guestCreatorId": guest_creator_id,
            "scope": "mirror_pickup",
            "publishToHost": participant.publish_to_host,
            "mirrorToGuestChannel": participant.mirror_to_guest_channel,
            "expiresAt": expires_at,
        }),
    )
    .await?;
    fetch_collaboration_mirror_grant_by_id(&state.pool, &grant_id).await
}

pub(super) async fn redeem_collaboration_mirror_grant_internal(
    state: &SharedState,
    identity: &RequestIdentity,
    grant_id: &str,
) -> AppResult<CollaborationMirrorGrant> {
    let grant = fetch_collaboration_mirror_grant_by_id(&state.pool, grant_id).await?;
    let participant =
        fetch_collaboration_participant_by_id(&state.pool, &grant.participant_id).await?;
    let session = fetch_collaboration_session_by_id(&state.pool, &grant.session_id).await?;
    if participant.user_id != identity.user_id {
        return Err(AppError::Forbidden);
    }
    validate_redeemable_collaboration_mirror_grant(&grant, &participant, &session)?;
    let guest_creator_id = identity.require_creator_scope()?;
    if Some(guest_creator_id) != participant.creator_id.as_deref() {
        return Err(AppError::Forbidden);
    }

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE collaboration_mirror_grants SET state = 'active', activated_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(grant_id)
    .execute(&state.pool)
    .await?;
    let activated_grant = fetch_collaboration_mirror_grant_by_id(&state.pool, grant_id).await?;
    let pickup = match activate_collaboration_mirror_pickup(
        state,
        &session,
        &participant,
        &activated_grant,
        &now,
    )
    .await
    {
        Ok(pickup) => pickup,
        Err(error) => {
            sqlx::query(
                "UPDATE collaboration_mirror_grants SET state = 'issued', activated_at = NULL WHERE id = ? AND state = 'active'",
            )
            .bind(grant_id)
            .execute(&state.pool)
            .await?;
            deactivate_collaboration_mirror_pickups_for_grants(
                state,
                std::slice::from_ref(&activated_grant),
                "revoked",
                &now,
            )
            .await?;
            return Err(error);
        }
    };
    publish_collaboration_event(
        state,
        &grant.session_id,
        Some(identity.user_id.clone()),
        Some(participant.id.clone()),
        "mirror_grant_redeemed",
        json!({
            "grantId": grant.id,
            "participantId": participant.id,
            "guestCreatorId": grant.guest_creator_id,
            "activatedAt": now,
            "guestBroadcastId": pickup.guest_broadcast_id,
            "pickupId": pickup.id,
        }),
    )
    .await?;

    fetch_collaboration_mirror_grant_by_id(&state.pool, grant_id).await
}
