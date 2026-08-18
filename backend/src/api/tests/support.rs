use super::*;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

pub(super) fn auth_headers(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).expect("valid auth header"),
    );
    headers
}

pub(super) async fn setup_test_state() -> AppResult<(SharedState, CreatorProfile)> {
    let test_id = Uuid::new_v4().to_string();
    let db_path = std::env::temp_dir().join(format!("lifestream-test-{test_id}.db"));
    let media_root = std::env::temp_dir().join(format!("lifestream-media-{test_id}"));
    let source_db_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    copy_sqlite_fixture(source_db_dir.join("lifestream.db"), &db_path).await?;
    let database_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;

    sqlx::raw_sql(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;
        PRAGMA busy_timeout = 5000;
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    tokio::fs::create_dir_all(&media_root)
        .await
        .map_err(AppError::Io)?;

    let state = Arc::new(AppState::new(
        pool.clone(),
        PathBuf::from(&media_root),
        vec![HeaderValue::from_static("http://localhost:3000")],
    ));
    let creator = fetch_creator_profile(&pool, "crt-deepsaint").await?;
    reset_creator_live_state(&pool, &creator).await?;
    Ok((state, creator))
}

pub(super) async fn insert_creator_auth_session(
    pool: &SqlitePool,
    creator: &CreatorProfile,
) -> AppResult<String> {
    let token = format!("test-creator-token-{}", Uuid::new_v4().simple());
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO auth_sessions (
            id, user_id, label, token_hash, scopes_json, created_at, expires_at, revoked_at, last_used_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(format!("sess-test-{}", Uuid::new_v4().simple()))
    .bind(&creator.user_id)
    .bind("test-creator-session")
    .bind(hash_token(&token))
    .bind(json!([
        "user",
        "creator",
        "creator:write",
        "admin"
    ])
    .to_string())
    .bind(&now)
    .bind((Utc::now() + chrono::Duration::hours(2)).to_rfc3339())
    .bind(Option::<String>::None)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(token)
}

pub(super) async fn insert_user_auth_session(
    pool: &SqlitePool,
    user_id: &str,
    scopes: &[&str],
) -> AppResult<String> {
    let token = format!("test-user-token-{}", Uuid::new_v4().simple());
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO auth_sessions (
            id, user_id, label, token_hash, scopes_json, created_at, expires_at, revoked_at, last_used_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(format!("sess-user-test-{}", Uuid::new_v4().simple()))
    .bind(user_id)
    .bind("test-user-session")
    .bind(hash_token(&token))
    .bind(serde_json::to_string(scopes)?)
    .bind(&now)
    .bind((Utc::now() + chrono::Duration::hours(2)).to_rfc3339())
    .bind(Option::<String>::None)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(token)
}

pub(super) async fn insert_ready_collaboration_broadcast(
    pool: &SqlitePool,
    creator: &CreatorProfile,
) -> AppResult<Broadcast> {
    let broadcast = Broadcast {
        id: format!("test-collab-broadcast-{}", Uuid::new_v4().simple()),
        title: "Collaboration Control".to_string(),
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
        thumbnail: "https://cdn.lifestream.local/thumb/collab-ready.jpg".to_string(),
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
        "UPDATE creator_profiles SET current_broadcast_id = ?, live_status = 'ready' WHERE id = ?",
    )
    .bind(&broadcast.id)
    .bind(&creator.id)
    .execute(pool)
    .await?;
    Ok(broadcast)
}

pub(super) async fn insert_shared_chat_collaboration_for_current_broadcast(
    pool: &SqlitePool,
    host_creator: &CreatorProfile,
    guest_creator_id: &str,
    guest_user_id: &str,
    can_speak_in_chat: bool,
) -> AppResult<(CollaborationSession, CollaborationParticipant)> {
    let current_host_profile = fetch_creator_profile(pool, &host_creator.id).await?;
    let broadcast_id = match current_host_profile.current_broadcast_id.clone() {
        Some(broadcast_id) => broadcast_id,
        None => {
            let broadcast = insert_ready_collaboration_broadcast(pool, host_creator).await?;
            sqlx::query(
                "UPDATE creator_profiles SET current_broadcast_id = ?, live_status = 'live' WHERE id = ?",
            )
            .bind(&broadcast.id)
            .bind(&host_creator.id)
            .execute(pool)
            .await?;
            broadcast.id
        }
    };
    let now = Utc::now().to_rfc3339();
    let session_id = format!("test-collab-live-{}", Uuid::new_v4().simple());
    sqlx::query(
        r#"
        INSERT INTO collaboration_sessions (
            id, host_creator_id, source_broadcast_id, title, status, chat_mode,
            recording_policy, last_event_seq, created_at, updated_at, activated_at, ended_at
        ) VALUES (?, ?, ?, ?, 'active', 'shared', 'host_archive', 0, ?, ?, ?, NULL)
        "#,
    )
    .bind(&session_id)
    .bind(&host_creator.id)
    .bind(&broadcast_id)
    .bind("Shared Chat Enforcement")
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    let host_participant_id = format!("test-colp-host-{}", Uuid::new_v4().simple());
    sqlx::query(
        r#"
        INSERT INTO collaboration_participants (
            id, session_id, invite_id, user_id, creator_id, role, state, publish_to_host,
            mirror_to_guest_channel, can_speak_in_chat, joined_at, left_at, created_at, updated_at
        ) VALUES (?, ?, NULL, ?, ?, 'host', 'live', 1, 0, 1, ?, NULL, ?, ?)
        "#,
    )
    .bind(&host_participant_id)
    .bind(&session_id)
    .bind(&host_creator.user_id)
    .bind(&host_creator.id)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    let guest_participant_id = format!("test-colp-guest-{}", Uuid::new_v4().simple());
    sqlx::query(
        r#"
        INSERT INTO collaboration_participants (
            id, session_id, invite_id, user_id, creator_id, role, state, publish_to_host,
            mirror_to_guest_channel, can_speak_in_chat, joined_at, left_at, created_at, updated_at
        ) VALUES (?, ?, NULL, ?, ?, 'guest', 'live', 1, 1, ?, ?, NULL, ?, ?)
        "#,
    )
    .bind(&guest_participant_id)
    .bind(&session_id)
    .bind(guest_user_id)
    .bind(guest_creator_id)
    .bind(can_speak_in_chat as i64)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok((
        fetch_collaboration_session_by_id(pool, &session_id).await?,
        fetch_collaboration_participant_by_id(pool, &guest_participant_id).await?,
    ))
}

pub(super) async fn insert_live_stream_for_creator(
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

pub(super) async fn copy_sqlite_fixture(source_db: PathBuf, target_db: &Path) -> AppResult<()> {
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

pub(super) async fn reset_creator_live_state(
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
    sqlx::query("UPDATE creator_profiles SET live_status = 'offline', current_broadcast_id = NULL WHERE id = ?")
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

pub(super) async fn write_test_media_file(
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

pub(super) async fn insert_ready_broadcast(
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

pub(super) async fn creator_live_event_count(
    pool: &SqlitePool,
    broadcast_id: &str,
) -> AppResult<i64> {
    let row = sqlx::query(
        "SELECT COUNT(*) AS count FROM notification_events WHERE kind = 'creator_live' AND payload_json LIKE ?",
    )
    .bind(format!("%{broadcast_id}%"))
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}

pub(super) async fn creator_notification_delivery_count(
    pool: &SqlitePool,
    creator_id: &str,
    kind: &str,
    broadcast_id: &str,
) -> AppResult<i64> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS count
        FROM notification_deliveries d
        JOIN notification_events e ON e.id = d.event_id
        WHERE d.recipient_creator_id = ?
          AND e.kind = ?
          AND e.payload_json LIKE ?
        "#,
    )
    .bind(creator_id)
    .bind(kind)
    .bind(format!("%{broadcast_id}%"))
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}

pub(super) async fn insert_test_notification_delivery(
    pool: &SqlitePool,
    recipient_user_id: &str,
    channel: &str,
) -> AppResult<String> {
    let event_id = format!("notev-{}", Uuid::new_v4().simple());
    let delivery_id = format!("notd-{}", Uuid::new_v4().simple());
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO notification_events (
            id, kind, body, actor_user_id, actor_label, creator_id, stream_id, amount, payload_json, created_at
        ) VALUES (?, 'test_notification', 'test body', NULL, NULL, NULL, NULL, NULL, '{}', ?)
        "#,
    )
    .bind(&event_id)
    .bind(&now)
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO notification_deliveries (
            id, event_id, recipient_user_id, recipient_creator_id, channel, state, sent_at, delivered_at, read_at,
            failed_at, last_error, retry_count, last_attempted_at, next_attempt_at
        ) VALUES (?, ?, ?, NULL, ?, 'pending', ?, NULL, NULL, NULL, NULL, 0, NULL, ?)
        "#,
    )
    .bind(&delivery_id)
    .bind(&event_id)
    .bind(recipient_user_id)
    .bind(channel)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(delivery_id)
}

pub(super) async fn live_ingest_event_count_for_session(
    pool: &SqlitePool,
    session_id: &str,
    event_type: &str,
) -> AppResult<i64> {
    let row = sqlx::query(
        "SELECT COUNT(*) AS count FROM live_ingest_events WHERE session_id = ? AND event_type = ?",
    )
    .bind(session_id)
    .bind(event_type)
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}

pub(super) async fn insert_playback_session_for_upload(
    pool: &SqlitePool,
    upload_id: &str,
    user_id: Option<&str>,
    auth_session_id: Option<&str>,
    access_scope: &str,
) -> AppResult<(String, String, MediaAsset)> {
    let target = fetch_upload_playback_target(pool, upload_id).await?;
    let session_id = format!("test-pbs-{}", Uuid::new_v4().simple());
    let playback_token = format!("test-pbt-{}", Uuid::new_v4().simple());
    let now = Utc::now();
    let now_rfc3339 = now.to_rfc3339();
    let expires_at = (now + chrono::Duration::hours(1)).to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO playback_sessions (
            id, auth_session_id, user_id, creator_id, asset_id, content_id, content_kind, token_hash,
            access_scope, created_at, expires_at, last_used_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&session_id)
    .bind(auth_session_id)
    .bind(user_id)
    .bind(Some(target.creator_id.clone()))
    .bind(&target.asset.id)
    .bind(upload_id)
    .bind(&target.asset.kind)
    .bind(hash_token(&playback_token))
    .bind(access_scope)
    .bind(&now_rfc3339)
    .bind(&expires_at)
    .bind(&now_rfc3339)
    .execute(pool)
    .await?;
    Ok((session_id, playback_token, target.asset))
}

pub(super) async fn seed_content_purchase_for_user(
    pool: &SqlitePool,
    user_id: &str,
    creator_id: &str,
    upload_id: &str,
    access_policy: &str,
    amount_cents: i64,
    currency: &str,
    purchased_at: &str,
    expires_at: Option<&str>,
    status: &str,
) -> AppResult<String> {
    sqlx::query("DELETE FROM content_purchases WHERE user_id = ? AND upload_id = ?")
        .bind(user_id)
        .bind(upload_id)
        .execute(pool)
        .await?;
    let purchase_id = format!("pur-test-{}", Uuid::new_v4().simple());
    sqlx::query(
        r#"
        INSERT INTO content_purchases (
            id, user_id, creator_id, upload_id, access_policy, amount_cents, currency,
            status, purchased_at, expires_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&purchase_id)
    .bind(user_id)
    .bind(creator_id)
    .bind(upload_id)
    .bind(access_policy)
    .bind(amount_cents)
    .bind(currency)
    .bind(status)
    .bind(purchased_at)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(purchase_id)
}

pub(super) async fn insert_active_collaboration_session(
    pool: &SqlitePool,
    host_creator: &CreatorProfile,
    guest_creator_id: &str,
    guest_user_id: &str,
) -> AppResult<(CollaborationSession, CollaborationParticipant)> {
    let broadcast = Broadcast {
        id: format!("test-collab-bc-{}", Uuid::new_v4().simple()),
        title: "Collaboration Validation".to_string(),
        category: host_creator.default_category.clone(),
        tags: host_creator.default_tags.clone(),
        status: "live".to_string(),
        started_at: Utc::now().to_rfc3339(),
        ended_at: None,
        duration_sec: None,
        peak_viewers: 0,
        average_viewers: 0,
        chat_messages: 0,
        new_followers: 0,
        new_subscribers: 0,
        revenue: 0.0,
        thumbnail: "https://cdn.lifestream.local/thumb/collab.jpg".to_string(),
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
    .bind(&host_creator.id)
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

    let now = Utc::now().to_rfc3339();
    let session_id = format!("test-collab-{}", Uuid::new_v4().simple());
    sqlx::query(
        r#"
        INSERT INTO collaboration_sessions (
            id, host_creator_id, source_broadcast_id, title, status, chat_mode,
            recording_policy, last_event_seq, created_at, updated_at, activated_at, ended_at
        ) VALUES (?, ?, ?, ?, 'active', 'shared', 'host_archive', 0, ?, ?, ?, NULL)
        "#,
    )
    .bind(&session_id)
    .bind(&host_creator.id)
    .bind(&broadcast.id)
    .bind("Validation Session")
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    let host_participant_id = format!("test-colp-host-{}", Uuid::new_v4().simple());
    sqlx::query(
        r#"
        INSERT INTO collaboration_participants (
            id, session_id, invite_id, user_id, creator_id, role, state, publish_to_host,
            mirror_to_guest_channel, can_speak_in_chat, joined_at, left_at, created_at, updated_at
        ) VALUES (?, ?, NULL, ?, ?, 'host', 'live', 1, 0, 1, ?, NULL, ?, ?)
        "#,
    )
    .bind(&host_participant_id)
    .bind(&session_id)
    .bind(&host_creator.user_id)
    .bind(&host_creator.id)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    let guest_participant_id = format!("test-colp-guest-{}", Uuid::new_v4().simple());
    sqlx::query(
        r#"
        INSERT INTO collaboration_participants (
            id, session_id, invite_id, user_id, creator_id, role, state, publish_to_host,
            mirror_to_guest_channel, can_speak_in_chat, joined_at, left_at, created_at, updated_at
        ) VALUES (?, ?, NULL, ?, ?, 'guest', 'live', 1, 1, 1, ?, NULL, ?, ?)
        "#,
    )
    .bind(&guest_participant_id)
    .bind(&session_id)
    .bind(guest_user_id)
    .bind(guest_creator_id)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok((
        fetch_collaboration_session_by_id(pool, &session_id).await?,
        fetch_collaboration_participant_by_id(pool, &guest_participant_id).await?,
    ))
}

pub(super) async fn insert_mirror_grant(
    pool: &SqlitePool,
    session: &CollaborationSession,
    participant: &CollaborationParticipant,
    expires_at: &str,
) -> AppResult<CollaborationMirrorGrant> {
    let grant_id = format!("test-colm-{}", Uuid::new_v4().simple());
    let issued_at = (Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO collaboration_mirror_grants (
            id, session_id, participant_id, host_creator_id, guest_creator_id, scope, state,
            publish_to_host, mirror_to_guest_channel, token_hash, issued_at, activated_at, revoked_at, expires_at
        ) VALUES (?, ?, ?, ?, ?, 'mirror_pickup', 'issued', ?, ?, ?, ?, NULL, NULL, ?)
        "#
    )
    .bind(&grant_id)
    .bind(&session.id)
    .bind(&participant.id)
    .bind(&session.host_creator_id)
    .bind(participant.creator_id.as_deref())
    .bind(participant.publish_to_host as i64)
    .bind(participant.mirror_to_guest_channel as i64)
    .bind(hash_token(&format!("grant-token-{grant_id}")))
    .bind(&issued_at)
    .bind(expires_at)
    .execute(pool)
    .await?;
    fetch_collaboration_mirror_grant_by_id(pool, &grant_id).await
}

pub(super) async fn insert_collaboration_participant(
    pool: &SqlitePool,
    session_id: &str,
    user_id: &str,
    creator_id: Option<&str>,
    role: &str,
    state: &str,
    publish_to_host: bool,
    mirror_to_guest_channel: bool,
    can_speak_in_chat: bool,
) -> AppResult<CollaborationParticipant> {
    let participant_id = format!("test-colp-extra-{}", Uuid::new_v4().simple());
    let now = Utc::now().to_rfc3339();
    let joined_at = if matches!(state, "accepted" | "backstage" | "live") {
        Some(now.clone())
    } else {
        None
    };
    let left_at = if matches!(state, "left" | "removed") {
        Some(now.clone())
    } else {
        None
    };
    sqlx::query(
        r#"
        INSERT INTO collaboration_participants (
            id, session_id, invite_id, user_id, creator_id, role, state, publish_to_host,
            mirror_to_guest_channel, can_speak_in_chat, joined_at, left_at, created_at, updated_at
        ) VALUES (?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&participant_id)
    .bind(session_id)
    .bind(user_id)
    .bind(creator_id)
    .bind(role)
    .bind(state)
    .bind(publish_to_host as i64)
    .bind(mirror_to_guest_channel as i64)
    .bind(can_speak_in_chat as i64)
    .bind(joined_at)
    .bind(left_at)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    fetch_collaboration_participant_by_id(pool, &participant_id).await
}

pub(super) async fn insert_test_user_with_creator_profile(
    pool: &SqlitePool,
    user_id: &str,
    handle: &str,
    display_name: &str,
    creator_id: &str,
    creator_handle: &str,
    creator_display_name: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO users (id, handle, display_name, avatar, tier, joined_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(handle)
    .bind(display_name)
    .bind(format!("https://cdn.lifestream.local/avatar/{handle}.jpg"))
    .bind("free")
    .bind(&now)
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO creator_profiles (
            id, user_id, handle, display_name, avatar, banner, tagline, bio, partner_status,
            joined_at, stream_key, rtmp_url, default_category, default_tags_json, followers,
            subscribers, monthly_viewers, total_watch_hours, live_status, current_broadcast_id
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(creator_id)
    .bind(user_id)
    .bind(creator_handle)
    .bind(creator_display_name)
    .bind(format!(
        "https://cdn.lifestream.local/avatar/{creator_handle}.jpg"
    ))
    .bind(format!(
        "https://cdn.lifestream.local/banner/{creator_handle}.jpg"
    ))
    .bind("Co-stream everything")
    .bind("Extra guest creator")
    .bind("affiliate")
    .bind(&now)
    .bind(format!("sk_{creator_handle}"))
    .bind("rtmp://ingest.lifestream.local/live")
    .bind("Gaming")
    .bind(json!(["co-stream"]).to_string())
    .bind(0_i64)
    .bind(0_i64)
    .bind(0_i64)
    .bind(0_i64)
    .bind("offline")
    .bind(Option::<String>::None)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn insert_collaboration_socket_session(
    pool: &SqlitePool,
    session_id: &str,
    user_id: &str,
    creator_id: Option<&str>,
    participant_id: &str,
    connected_at: &str,
    last_seen_at: &str,
    disconnected_at: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO collaboration_socket_sessions (
            id, collaboration_session_id, user_id, creator_id, participant_id,
            session_token_hash, connected_at, last_seen_at, disconnected_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(format!("css-test-{}", Uuid::new_v4().simple()))
    .bind(session_id)
    .bind(user_id)
    .bind(creator_id)
    .bind(participant_id)
    .bind(hash_token(&format!("socket-{}", Uuid::new_v4().simple())))
    .bind(connected_at)
    .bind(last_seen_at)
    .bind(disconnected_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn publish_test_collaboration_event(
    state: &SharedState,
    session_id: &str,
    participant_id: &str,
    actor_user_id: &str,
    event_type: &str,
) -> AppResult<CollaborationEvent> {
    publish_collaboration_event(
        state,
        session_id,
        Some(actor_user_id.to_string()),
        Some(participant_id.to_string()),
        event_type,
        json!({
            "participantId": participant_id,
            "eventType": event_type,
        }),
    )
    .await
}
