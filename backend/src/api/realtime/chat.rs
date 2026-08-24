use super::discovery::fetch_user;
use super::moderation::{
    can_bypass_live_chat_restrictions, fetch_active_live_moderation_action,
    fetch_live_stream_owner_context,
};
use super::*;
use axum::extract::ws::{Message, WebSocket};
use futures_util::sink::SinkExt;

pub(crate) async fn persist_chat_message(
    state: &SharedState,
    stream_id: &str,
    identity: &RequestIdentity,
    input: ChatInput,
) -> AppResult<PersistedChatMessage> {
    enforce_rate_limit(
        state,
        &format!("chat:{}:{}", stream_id, identity.user_id),
        20,
        Duration::from_secs(10),
    )
    .await?;
    ensure_stream_exists(state.db.try_sqlite_adapter()?, stream_id).await?;
    let body = input.body.trim();
    if body.is_empty() {
        return Err(AppError::BadRequest("message body is required".to_string()));
    }
    if body.len() > 500 {
        return Err(AppError::BadRequest(
            "message body must be 500 characters or fewer".to_string(),
        ));
    }

    let stream_owner =
        fetch_live_stream_owner_context(state.db.try_sqlite_adapter()?, stream_id).await?;
    let stream_creator_id = stream_owner.creator_id.clone();
    enforce_collaboration_chat_participation_permissions(
        state.db.try_sqlite_adapter()?,
        stream_owner.current_broadcast_id.as_deref(),
        &identity.user_id,
    )
    .await?;
    let (stream_settings, bypass_restrictions, moderation_action, has_active_membership) = tokio::try_join!(
        fetch_creator_live_settings(state.db.try_sqlite_adapter()?, &stream_creator_id),
        can_bypass_live_chat_restrictions(
            state.db.try_sqlite_adapter()?,
            &stream_creator_id,
            identity
        ),
        fetch_active_live_moderation_action(
            state.db.try_sqlite_adapter()?,
            stream_id,
            &identity.user_id
        ),
        fetch_active_creator_membership(
            state.db.try_sqlite_adapter()?,
            &identity.user_id,
            &stream_creator_id,
            None,
        ),
    )?;
    if let Some(action) = moderation_action.as_ref() {
        match action.action_type.as_str() {
            "ban" | "mute" => {
                return Err(AppError::Forbidden);
            }
            _ => {}
        }
    }

    if !bypass_restrictions {
        if stream_settings.subscriber_only && !has_active_membership {
            return Err(AppError::PaymentRequired(
                "subscriber-only chat requires an active creator membership".to_string(),
            ));
        }

        if stream_settings.slow_mode_seconds > 0 {
            enforce_live_chat_slow_mode(
                state.db.try_sqlite_adapter()?,
                stream_id,
                &identity.user_id,
                stream_settings.slow_mode_seconds,
            )
            .await?;
        }

        if let Some(reason) =
            detect_live_chat_automod_violation(&stream_settings.auto_mod_level, body)
        {
            return Err(AppError::BadRequest(format!(
                "message rejected by automod: {reason}"
            )));
        }
    }

    let user = fetch_user(state.db.try_sqlite_adapter()?, &identity.user_id).await?;
    let mut badges = Vec::new();
    if has_active_membership {
        badges.push("subscriber".to_string());
    }
    if identity.creator_id.is_some() {
        badges.push("partner".to_string());
    }

    let message = ChatMessage {
        id: Uuid::new_v4().to_string(),
        sequence: next_chat_message_sequence(state.db.try_sqlite_adapter()?, stream_id).await?,
        user_handle: user.handle,
        display_name: user.display_name,
        color: input.color.unwrap_or_else(|| "#fafafa".to_string()),
        badges,
        body: body.to_string(),
        sent_at: Utc::now().to_rfc3339(),
    };
    let hidden_by_moderation = matches!(
        moderation_action,
        Some(action) if action.action_type == "shadowban"
    );

    sqlx::query(
        "INSERT INTO chat_messages (id, stream_id, user_id, creator_id, user_handle, display_name, color, badges_json, body, sent_at, hidden_by_moderation, sequence) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&message.id)
    .bind(stream_id)
    .bind(&identity.user_id)
    .bind(identity.creator_id.as_deref())
    .bind(&message.user_handle)
    .bind(&message.display_name)
    .bind(&message.color)
    .bind(to_json(&message.badges)?)
    .bind(&message.body)
    .bind(&message.sent_at)
    .bind(hidden_by_moderation as i64)
    .bind(message.sequence)
    .execute(state.db.try_sqlite_adapter()?)
    .await?;

    if !hidden_by_moderation {
        state
            .realtime
            .publish(
                &stream_channel_id(stream_id),
                WsEvent::ChatMessage {
                    message: message.clone(),
                },
            )
            .await;
    }

    Ok(PersistedChatMessage {
        message,
        hidden_by_moderation,
    })
}

pub(crate) async fn send_chat_message_rejected(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    reason: impl Into<String>,
) -> bool {
    sender
        .send(Message::Text(
            serde_json::to_string(&WsEvent::ChatMessageRejected {
                reason: reason.into(),
            })
            .unwrap_or_default(),
        ))
        .await
        .is_ok()
}

async fn enforce_collaboration_chat_participation_permissions(
    pool: &SqlitePool,
    current_broadcast_id: Option<&str>,
    user_id: &str,
) -> AppResult<()> {
    let Some(current_broadcast_id) = current_broadcast_id else {
        return Ok(());
    };
    let row = sqlx::query(
        r#"
        SELECT p.state, p.can_speak_in_chat
        FROM collaboration_sessions s
        LEFT JOIN collaboration_participants p
          ON p.session_id = s.id
         AND p.user_id = ?
        WHERE s.source_broadcast_id = ?
          AND s.status = 'active'
          AND s.chat_mode = 'shared'
        ORDER BY s.created_at DESC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(current_broadcast_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(());
    };
    let participant_state = row.get::<Option<String>, _>("state");
    if matches!(participant_state.as_deref(), Some("left" | "removed")) {
        return Ok(());
    }
    let Some(can_speak_in_chat) = row.get::<Option<i64>, _>("can_speak_in_chat") else {
        return Ok(());
    };
    if can_speak_in_chat == 1 {
        return Ok(());
    }
    Err(AppError::Forbidden)
}

async fn enforce_live_chat_slow_mode(
    pool: &SqlitePool,
    stream_id: &str,
    user_id: &str,
    slow_mode_seconds: i64,
) -> AppResult<()> {
    let row = sqlx::query(
        "SELECT sent_at FROM chat_messages WHERE stream_id = ? AND user_id = ? ORDER BY sent_at DESC LIMIT 1",
    )
    .bind(stream_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(());
    };

    let last_sent_at = chrono::DateTime::parse_from_rfc3339(&row.get::<String, _>("sent_at"))
        .map_err(|_| AppError::BadRequest("invalid chat timestamp".to_string()))?
        .with_timezone(&Utc);
    let next_allowed_at = last_sent_at + ChronoDuration::seconds(slow_mode_seconds);
    if Utc::now() < next_allowed_at {
        return Err(AppError::BadRequest(format!(
            "slow mode is active; wait {} seconds before sending another message",
            slow_mode_seconds
        )));
    }

    Ok(())
}

fn detect_live_chat_automod_violation(level: &str, body: &str) -> Option<&'static str> {
    if level == "off" {
        return None;
    }

    let trimmed = body.trim();
    let lowercase = trimmed.to_lowercase();
    if contains_blocked_invite_or_link(&lowercase) {
        return Some("links and invite spam are blocked");
    }
    if contains_repeated_spam_pattern(&lowercase) {
        return Some("repetitive spam is blocked");
    }

    if level == "strict" {
        if is_excessive_caps(trimmed) {
            return Some("excessive capitalized shouting is blocked");
        }
        if lowercase.contains("@everyone") || lowercase.contains("@here") {
            return Some("mass-mention spam is blocked");
        }
    }

    None
}

fn contains_blocked_invite_or_link(body: &str) -> bool {
    ["http://", "https://", "www.", "discord.gg/", "bit.ly/"]
        .iter()
        .any(|needle| body.contains(needle))
}

fn contains_repeated_spam_pattern(body: &str) -> bool {
    let tokens = body
        .split_whitespace()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if tokens.len() >= 4 && tokens.windows(2).all(|pair| pair[0] == pair[1]) {
        return true;
    }

    let collapsed = body.replace(' ', "");
    collapsed
        .chars()
        .collect::<Vec<_>>()
        .windows(8)
        .any(|window| window.iter().all(|value| *value == window[0]))
}

fn is_excessive_caps(body: &str) -> bool {
    let letters = body.chars().filter(|value| value.is_ascii_alphabetic());
    let mut total = 0;
    let mut uppercase = 0;
    for letter in letters {
        total += 1;
        if letter.is_ascii_uppercase() {
            uppercase += 1;
        }
    }

    total >= 12 && uppercase * 10 >= total * 8
}
