use super::*;

pub(crate) async fn insert_ready_collaboration_broadcast(
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

pub(crate) async fn insert_shared_chat_collaboration_for_current_broadcast(
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

pub(crate) async fn insert_active_collaboration_session(
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

pub(crate) async fn insert_mirror_grant(
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
        "#,
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

pub(crate) async fn insert_collaboration_participant(
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

pub(crate) async fn insert_test_user_with_creator_profile(
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

pub(crate) async fn insert_collaboration_socket_session(
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

pub(crate) async fn publish_test_collaboration_event(
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
