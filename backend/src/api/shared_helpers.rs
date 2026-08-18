use super::*;

pub(super) fn notification_delivery_record_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> NotificationDeliveryRecord {
    NotificationDeliveryRecord {
        id: row.get("id"),
        event_id: row.get("event_id"),
        kind: row.get("kind"),
        body: row.get("body"),
        channel: row.get("channel"),
        state: row.get("state"),
        actor: row.get("actor_label"),
        recipient_user_id: row.get("recipient_user_id"),
        recipient_creator_id: row.get("recipient_creator_id"),
        sent_at: row.get("sent_at"),
        delivered_at: row.get("delivered_at"),
        read_at: row.get("read_at"),
        failed_at: row.get("failed_at"),
        last_error: row.get("last_error"),
        retry_count: row.get("retry_count"),
        last_attempted_at: row.get("last_attempted_at"),
        next_attempt_at: row.get("next_attempt_at"),
    }
}

pub(super) fn stream_channel_id(stream_id: &str) -> String {
    format!("stream:{stream_id}")
}

pub(super) fn playback_content_session_api_url(content_id: &str) -> String {
    format!("/api/v1/playback/content/{content_id}/session")
}

pub(super) fn playback_live_session_api_url(stream_id: &str) -> String {
    format!("/api/v1/playback/live/{stream_id}/session")
}

pub(super) fn streamer_from_row(row: sqlx::sqlite::SqliteRow) -> Streamer {
    Streamer {
        id: row.get("id"),
        handle: row.get("handle"),
        display_name: row.get("display_name"),
        avatar: row.get("avatar"),
        bio: row.get("bio"),
        followers: row.get("followers"),
        is_partner: row.get::<i64, _>("is_partner") == 1,
        is_live: row.get::<i64, _>("is_live") == 1,
    }
}

pub(super) fn live_stream_from_row(row: sqlx::sqlite::SqliteRow) -> LiveStream {
    let playback_ready = row.get::<Option<String>, _>("playback_asset_id").is_some()
        && row
            .get::<Option<String>, _>("playback_relative_path")
            .is_some();
    LiveStream {
        id: row.get("id"),
        slug: row.get("slug"),
        title: row.get("title"),
        category: row.get("category"),
        tags: from_json(row.get::<String, _>("tags_json")).unwrap_or_default(),
        streamer: Streamer {
            id: row.get("streamer_id"),
            handle: row.get("handle"),
            display_name: row.get("display_name"),
            avatar: row.get("avatar"),
            bio: row.get("bio"),
            followers: row.get("followers"),
            is_partner: row.get::<i64, _>("is_partner") == 1,
            is_live: row.get::<i64, _>("is_live") == 1,
        },
        viewers: row.get("viewers"),
        started_at: row.get("started_at"),
        thumbnail: row.get("thumbnail"),
        language: row.get("language"),
        is_mature: row.get::<i64, _>("is_mature") == 1,
        kind: "live".to_string(),
        playback_session_url: playback_ready
            .then(|| playback_live_session_api_url(&row.get::<String, _>("id"))),
        playback_ready,
    }
}

pub(super) fn from_json<T: serde::de::DeserializeOwned>(value: String) -> AppResult<T> {
    Ok(serde_json::from_str(&value)?)
}

pub(super) fn to_json<T: serde::Serialize>(value: &T) -> AppResult<String> {
    Ok(serde_json::to_string(value)?)
}

pub(super) fn build_fts_query(input: &str) -> Option<String> {
    let normalized = input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>();

    let tokens = normalized
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .take(6)
        .map(|token| format!("{token}*"))
        .collect::<Vec<_>>();

    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" "))
    }
}
