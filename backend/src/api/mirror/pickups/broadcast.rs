use super::*;
use crate::api::ingestctl::fetch_live_runtime_targets_for_session;

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
        let mirrored_playback_relative_path =
            resolve_mirror_pickup_playback_relative_path(pool, pickup).await?;
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
        .bind(mirrored_playback_relative_path.or_else(|| {
            row.get::<Option<String>, _>("playback_relative_path")
        }))
        .bind(format!("lv-{}-live", guest_handle))
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn resolve_mirror_pickup_playback_relative_path(
    pool: &SqlitePool,
    pickup: &CollaborationMirrorPickup,
) -> AppResult<Option<String>> {
    let Some(source_session) =
        fetch_active_live_ingest_session_unreconciled(pool, &pickup.host_creator_id).await?
    else {
        return Ok(None);
    };
    if source_session.broadcast_id != pickup.source_broadcast_id {
        return Ok(None);
    }

    let targets = fetch_live_runtime_targets_for_session(pool, &source_session.id).await?;
    Ok(targets
        .into_iter()
        .find(|target| {
            target.target_kind == "mirror_channel"
                && target.playback_enabled
                && target.target_creator_id.as_deref() == Some(pickup.guest_creator_id.as_str())
                && target.target_broadcast_id.as_deref()
                    == Some(pickup.guest_broadcast_id.as_str())
        })
        .and_then(|target| target.relative_path))
}

pub(crate) async fn ensure_guest_broadcast_available_for_mirror_pickup(
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

pub(crate) async fn sync_collaboration_mirror_pickup_broadcast_state(
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
