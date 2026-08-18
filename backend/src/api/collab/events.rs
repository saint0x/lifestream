use super::*;

pub(crate) async fn fetch_collaboration_events(
    pool: &SqlitePool,
    session_id: &str,
    after_seq: i64,
    limit: i64,
) -> AppResult<Vec<CollaborationEvent>> {
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, sequence, actor_user_id, participant_id, event_type, payload_json, created_at
        FROM collaboration_events
        WHERE session_id = ? AND sequence > ?
        ORDER BY sequence ASC
        LIMIT ?
        "#,
    )
    .bind(session_id)
    .bind(after_seq.max(0))
    .bind(limit.clamp(1, 500))
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| CollaborationEvent {
            id: row.get("id"),
            session_id: row.get("session_id"),
            sequence: row.get("sequence"),
            actor_user_id: row.get("actor_user_id"),
            participant_id: row.get("participant_id"),
            event_type: row.get("event_type"),
            payload: serde_json::from_str(&row.get::<String, _>("payload_json"))
                .unwrap_or(Value::Null),
            created_at: row.get("created_at"),
        })
        .collect())
}

pub(crate) async fn load_collaboration_socket_event_bootstrap(
    pool: &SqlitePool,
    session_id: &str,
    after_seq: i64,
) -> AppResult<(Vec<CollaborationEvent>, Vec<CollaborationEvent>)> {
    if after_seq > 0 {
        let replay_events =
            fetch_collaboration_events(pool, session_id, after_seq.max(0), 100).await?;
        Ok((Vec::new(), replay_events))
    } else {
        let snapshot_events = fetch_collaboration_events(pool, session_id, 0, 100).await?;
        Ok((snapshot_events, Vec::new()))
    }
}

pub(crate) fn collaboration_event_is_visible_to_session(
    session: &CollaborationSessionView,
    event: &CollaborationEvent,
) -> bool {
    if session.participant.role == "host" {
        return true;
    }

    match event.event_type.as_str() {
        "invite_created" | "invite_revoked" | "invite_expired" => {
            event.payload["inviteeUserId"] == Value::String(session.participant.user_id.clone())
        }
        "mirror_grant_issued"
        | "mirror_grant_redeemed"
        | "mirror_grant_revoked"
        | "mirror_grant_expired"
        | "participant_state_requested" => {
            event.participant_id.as_deref() == Some(session.participant.id.as_str())
                || event.payload["participantId"] == Value::String(session.participant.id.clone())
        }
        _ => true,
    }
}

pub(crate) fn filter_visible_collaboration_events_for_session(
    session: &CollaborationSessionView,
    events: Vec<CollaborationEvent>,
) -> Vec<CollaborationEvent> {
    events
        .into_iter()
        .filter(|event| collaboration_event_is_visible_to_session(session, event))
        .collect()
}
