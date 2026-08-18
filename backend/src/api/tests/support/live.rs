use super::*;

pub(crate) async fn insert_live_stream_for_creator(
    pool: &SqlitePool,
    creator: &CreatorProfile,
) -> AppResult<String> {
    let current_creator = fetch_creator_profile(pool, &creator.id).await?;
    let broadcast = match current_creator.current_broadcast_id.clone() {
        Some(broadcast_id) => fetch_broadcast_by_id(pool, &creator.id, &broadcast_id).await?,
        None => insert_ready_broadcast(pool, creator).await?,
    };
    sqlx::query("UPDATE broadcasts SET status = 'live' WHERE id = ?")
        .bind(&broadcast.id)
        .execute(pool)
        .await?;
    sqlx::query(
        "UPDATE creator_profiles SET live_status = 'live', current_broadcast_id = ? WHERE id = ?",
    )
    .bind(&broadcast.id)
    .bind(&creator.id)
    .execute(pool)
    .await?;

    let now = Utc::now().to_rfc3339();
    let existing_connected = sqlx::query(
        "SELECT 1 FROM live_ingest_sessions WHERE creator_id = ? AND status = 'connected' LIMIT 1",
    )
    .bind(&creator.id)
    .fetch_optional(pool)
    .await?
    .is_some();
    if !existing_connected {
        sqlx::query(
            r#"
            INSERT INTO live_ingest_sessions (
                id, creator_id, broadcast_id, stream_key_hash, ingest_token_hash, protocol,
                ingest_server, status, bitrate_kbps, viewers, dropped_frames, connected_at,
                last_heartbeat_at, disconnected_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, 'connected', 0, 0, 0, ?, ?, NULL)
            "#,
        )
        .bind(format!("ing-test-{}", Uuid::new_v4().simple()))
        .bind(&creator.id)
        .bind(&broadcast.id)
        .bind(hash_token(&creator.stream_key))
        .bind(hash_token(&format!(
            "fixture-ingest-token-{}",
            Uuid::new_v4().simple()
        )))
        .bind("rtmp")
        .bind("test-ingest-fixture")
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;
    }

    ensure_live_stream_row(pool, creator, &broadcast, 0).await?;
    Ok(format!("lv-{}-live", creator.handle))
}

pub(crate) async fn copy_sqlite_fixture(source_db: PathBuf, target_db: &Path) -> AppResult<()> {
    tokio::fs::copy(&source_db, target_db).await?;
    for suffix in ["-wal", "-shm"] {
        let source_sidecar = PathBuf::from(format!("{}{}", source_db.display(), suffix));
        if tokio::fs::try_exists(&source_sidecar).await? {
            let target_sidecar = PathBuf::from(format!("{}{}", target_db.display(), suffix));
            tokio::fs::copy(source_sidecar, target_sidecar).await?;
        }
    }
    Ok(())
}

pub(crate) async fn reset_creator_live_state(
    pool: &SqlitePool,
    creator: &CreatorProfile,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE live_ingest_sessions SET status = 'ended', contribution_state = 'disconnected', disconnected_at = COALESCE(disconnected_at, ?), last_heartbeat_at = ? WHERE creator_id = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(&creator.id)
    .execute(pool)
    .await?;
    sqlx::query(
        "UPDATE broadcasts SET status = 'ended', ended_at = COALESCE(ended_at, ?), duration_sec = COALESCE(duration_sec, 0) WHERE creator_id = ? AND status IN ('ready', 'live')",
    )
    .bind(&now)
    .bind(&creator.id)
    .execute(pool)
    .await?;
    sqlx::query(
        "DELETE FROM live_streams WHERE streamer_id = (SELECT id FROM streamers WHERE handle = ?)",
    )
    .bind(&creator.handle)
    .execute(pool)
    .await?;
    sqlx::query(
        "UPDATE creator_profiles SET live_status = 'offline', current_broadcast_id = NULL WHERE id = ?",
    )
    .bind(&creator.id)
    .execute(pool)
    .await?;
    reset_creator_live_operational_metrics(pool, &creator.id).await?;
    sqlx::query("UPDATE streamers SET is_live = 0 WHERE handle = ?")
        .bind(&creator.handle)
        .execute(pool)
        .await?;
    Ok(())
}

pub(crate) async fn write_test_media_file(
    state: &SharedState,
    relative_path: &str,
    body: impl AsRef<[u8]>,
) -> AppResult<()> {
    let full_path = media_path_for_relative(state, relative_path);
    ensure_parent_dir(&full_path).await?;
    tokio::fs::write(full_path, body)
        .await
        .map_err(AppError::Io)?;
    Ok(())
}

pub(crate) async fn insert_ready_broadcast(
    pool: &SqlitePool,
    creator: &CreatorProfile,
) -> AppResult<Broadcast> {
    let broadcast = Broadcast {
        id: format!("test-bc-{}", Uuid::new_v4().simple()),
        title: "Reconnect Validation".to_string(),
        category: creator.default_category.clone(),
        tags: creator.default_tags.clone(),
        status: "ready".to_string(),
        started_at: Utc::now().to_rfc3339(),
        ended_at: None,
        duration_sec: None,
        peak_viewers: 0,
        average_viewers: 0,
        chat_messages: 0,
        new_followers: 0,
        new_subscribers: 0,
        revenue: 0.0,
        thumbnail: "https://cdn.lifestream.local/thumb/test.jpg".to_string(),
        is_mature: false,
    };

    sqlx::query(
        r#"
        INSERT INTO broadcasts (
            id, creator_id, title, category, tags_json, status, started_at, ended_at, duration_sec,
            peak_viewers, average_viewers, chat_messages, new_followers, new_subscribers, revenue,
            thumbnail, is_mature
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&broadcast.id)
    .bind(&creator.id)
    .bind(&broadcast.title)
    .bind(&broadcast.category)
    .bind(to_json(&broadcast.tags)?)
    .bind(&broadcast.status)
    .bind(&broadcast.started_at)
    .bind(&broadcast.ended_at)
    .bind(&broadcast.duration_sec)
    .bind(broadcast.peak_viewers)
    .bind(broadcast.average_viewers)
    .bind(broadcast.chat_messages)
    .bind(broadcast.new_followers)
    .bind(broadcast.new_subscribers)
    .bind(broadcast.revenue)
    .bind(&broadcast.thumbnail)
    .bind(broadcast.is_mature as i64)
    .execute(pool)
    .await?;
    sqlx::query(
        "UPDATE creator_profiles SET live_status = 'ready', current_broadcast_id = ? WHERE id = ?",
    )
    .bind(&broadcast.id)
    .bind(&creator.id)
    .execute(pool)
    .await?;

    Ok(broadcast)
}
