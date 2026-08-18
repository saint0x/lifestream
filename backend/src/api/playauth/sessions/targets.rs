use super::*;
use crate::api::control::{
    fetch_live_runtime_output_for_session, fetch_live_runtime_targets_for_session,
};

pub(crate) async fn fetch_upload_playback_target(
    pool: &SqlitePool,
    upload_id: &str,
) -> AppResult<UploadPlaybackTarget> {
    let row = sqlx::query(
        r#"
        SELECT creator_id
        FROM uploads
        WHERE id = ?
        "#,
    )
    .bind(upload_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let creator_id: String = row.get("creator_id");
    let upload = fetch_upload_by_id(pool, &creator_id, upload_id).await?;
    let asset = fetch_media_asset_by_upload_id(pool, &creator_id, upload_id).await?;
    if asset.status != "ready" && asset.status != "published" {
        return Err(AppError::BadRequest(
            "asset is not ready for playback".to_string(),
        ));
    }
    Ok(UploadPlaybackTarget {
        creator_id,
        upload,
        asset,
    })
}

pub(crate) async fn fetch_live_stream_playback_target(
    pool: &SqlitePool,
    stream_id: &str,
) -> AppResult<LivePlaybackTarget> {
    let fresh_cutoff = stale_live_ingest_cutoff();
    let row = sqlx::query(
        r#"
        SELECT ls.id, ls.title, ls.playback_asset_id, ls.poster_relative_path, ls.playback_relative_path,
               cp.id AS creator_id, cp.current_broadcast_id
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

    let playback_asset_id = row
        .get::<Option<String>, _>("playback_asset_id")
        .ok_or_else(|| AppError::BadRequest("live playback asset unavailable".to_string()))?;
    let playback_relative_path = row
        .get::<Option<String>, _>("playback_relative_path")
        .ok_or_else(|| AppError::BadRequest("live playback manifest unavailable".to_string()))?;

    let asset_exists =
        sqlx::query("SELECT 1 FROM media_assets WHERE id = ? AND status IN ('ready', 'published')")
            .bind(&playback_asset_id)
            .fetch_optional(pool)
            .await?
            .is_some();
    if !asset_exists {
        return Err(AppError::BadRequest(
            "live playback asset is not ready".to_string(),
        ));
    }

    let creator_id: String = row.get("creator_id");
    let current_broadcast_id = row.get::<Option<String>, _>("current_broadcast_id");
    let source_session =
        resolve_live_stream_runtime_source_session(pool, &creator_id, &fresh_cutoff).await?;
    let runtime_output: LiveRuntimeOutput =
        fetch_live_runtime_output_for_session(pool, &source_session.id)
            .await?
            .ok_or_else(|| AppError::BadRequest("live runtime output unavailable".to_string()))?;
    if !matches!(
        runtime_output.packaging_status.as_str(),
        "ready" | "complete"
    ) {
        return Err(AppError::BadRequest(
            "live runtime has not confirmed playback readiness".to_string(),
        ));
    }
    if !live_runtime_path_matches_playback_target(
        pool,
        &source_session.id,
        &runtime_output,
        playback_relative_path.as_str(),
        creator_id.as_str(),
        current_broadcast_id.as_deref(),
    )
    .await?
    {
        return Err(AppError::BadRequest(
            "live runtime manifest is not aligned with the published playback path".to_string(),
        ));
    }
    let asset = fetch_media_asset_by_id_any_creator(pool, &playback_asset_id).await?;

    Ok(LivePlaybackTarget {
        creator_id,
        asset_id: playback_asset_id,
        title: row.get("title"),
        poster_relative_path: row.get("poster_relative_path"),
        playback_relative_path,
        runtime_output,
        asset,
    })
}

async fn resolve_live_stream_runtime_source_session(
    pool: &SqlitePool,
    creator_id: &str,
    fresh_cutoff: &str,
) -> AppResult<LiveIngestSession> {
    if let Some(session) = fetch_active_live_ingest_session_unreconciled(pool, creator_id).await? {
        if session.last_heartbeat_at.as_str() >= fresh_cutoff {
            return Ok(session);
        }
    }

    let source_row = sqlx::query(
        r#"
        SELECT lis.id
        FROM collaboration_mirror_pickups cmp
        JOIN live_ingest_sessions lis
          ON lis.creator_id = cmp.host_creator_id
         AND lis.broadcast_id = cmp.source_broadcast_id
        WHERE cmp.guest_creator_id = ?
          AND cmp.state = 'active'
          AND lis.status = 'connected'
          AND lis.last_heartbeat_at >= ?
        ORDER BY lis.connected_at DESC
        LIMIT 1
        "#,
    )
    .bind(creator_id)
    .bind(fresh_cutoff)
    .fetch_optional(pool)
    .await?;

    let session_id: String = source_row
        .map(|row| row.get("id"))
        .ok_or_else(|| AppError::BadRequest("live runtime session unavailable".to_string()))?;
    fetch_live_ingest_session_by_id_global_unreconciled(pool, &session_id).await
}

async fn live_runtime_path_matches_playback_target(
    pool: &SqlitePool,
    source_session_id: &str,
    runtime_output: &LiveRuntimeOutput,
    playback_relative_path: &str,
    creator_id: &str,
    current_broadcast_id: Option<&str>,
) -> AppResult<bool> {
    if runtime_output.manifest_relative_path.as_deref() == Some(playback_relative_path) {
        return Ok(true);
    }

    let Some(current_broadcast_id) = current_broadcast_id else {
        return Ok(false);
    };
    let targets = fetch_live_runtime_targets_for_session(pool, source_session_id).await?;
    Ok(targets.into_iter().any(|target| {
        target.playback_enabled
            && target.relative_path.as_deref() == Some(playback_relative_path)
            && target.target_creator_id.as_deref() == Some(creator_id)
            && target.target_broadcast_id.as_deref() == Some(current_broadcast_id)
    }))
}

pub(crate) fn playback_session_from_record(session: &PlaybackSessionRecord) -> PlaybackSession {
    PlaybackSession {
        id: session.id.clone(),
        content_id: session.content_id.clone(),
        content_kind: session.content_kind.clone(),
        access_scope: session.access_scope.clone(),
        created_at: session.created_at.clone(),
        expires_at: session.expires_at.clone(),
        last_used_at: session.last_used_at.clone(),
    }
}
