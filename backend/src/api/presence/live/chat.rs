use super::*;

pub(crate) async fn fetch_chat_messages_for_viewer(
    pool: &SqlitePool,
    stream_id: &str,
    viewer_user_id: Option<&str>,
    limit: i64,
    after_seq: Option<i64>,
) -> AppResult<Vec<ChatMessage>> {
    let rows = match after_seq {
        Some(sequence) if sequence > 0 => {
            sqlx::query(
                r#"
                SELECT id, sequence, user_handle, display_name, color, badges_json, body, sent_at
                FROM chat_messages
                WHERE stream_id = ?
                  AND sequence > ?
                  AND (hidden_by_moderation = 0 OR (user_id = ? AND hidden_by_moderation = 1))
                ORDER BY sequence ASC
                LIMIT ?
                "#,
            )
            .bind(stream_id)
            .bind(sequence)
            .bind(viewer_user_id)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        _ => {
            sqlx::query(
                r#"
                SELECT id, sequence, user_handle, display_name, color, badges_json, body, sent_at
                FROM chat_messages
                WHERE stream_id = ?
                  AND (hidden_by_moderation = 0 OR (user_id = ? AND hidden_by_moderation = 1))
                ORDER BY sequence DESC
                LIMIT ?
                "#,
            )
            .bind(stream_id)
            .bind(viewer_user_id)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
    };

    let mut messages = rows
        .into_iter()
        .map(|row| ChatMessage {
            id: row.get("id"),
            sequence: row.get("sequence"),
            user_handle: row.get("user_handle"),
            display_name: row.get("display_name"),
            color: row.get("color"),
            badges: from_json(row.get::<String, _>("badges_json")).unwrap_or_default(),
            body: row.get("body"),
            sent_at: row.get("sent_at"),
        })
        .collect::<Vec<_>>();
    if after_seq.unwrap_or(0) <= 0 {
        messages.reverse();
    }
    Ok(messages)
}

pub(crate) async fn next_chat_message_sequence(
    pool: &SqlitePool,
    stream_id: &str,
) -> AppResult<i64> {
    let row = sqlx::query(
        r#"
        INSERT INTO chat_stream_cursors (stream_id, last_sequence)
        VALUES (?, 1)
        ON CONFLICT(stream_id) DO UPDATE SET
            last_sequence = chat_stream_cursors.last_sequence + 1
        RETURNING last_sequence AS next_sequence
        "#,
    )
    .bind(stream_id)
    .fetch_one(pool)
    .await?;
    Ok(row.get("next_sequence"))
}

pub(crate) async fn ensure_stream_exists(pool: &SqlitePool, stream_id: &str) -> AppResult<()> {
    let fresh_cutoff = stale_live_ingest_cutoff();
    let exists = sqlx::query(
        r#"
        SELECT 1
        FROM live_streams ls
        JOIN streamers s ON s.id = ls.streamer_id
        WHERE ls.id = ?
          AND EXISTS (
            SELECT 1
            FROM creator_profiles cp
            JOIN live_ingest_sessions lis ON lis.creator_id = cp.id
            WHERE cp.handle = s.handle
              AND lis.status = 'connected'
              AND lis.last_heartbeat_at >= ?
          )
        "#,
    )
    .bind(stream_id)
    .bind(&fresh_cutoff)
    .fetch_optional(pool)
    .await?
    .is_some();
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound)
    }
}
