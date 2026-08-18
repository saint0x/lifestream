use super::*;

pub(crate) async fn fetch_admin_live_ingest_sessions(
    pool: &SqlitePool,
    creator_filter: Option<&str>,
    status_filter: Option<&str>,
    limit: i64,
) -> AppResult<Vec<AdminLiveIngestSessionRecord>> {
    reconcile_stale_live_ingest_sessions_for_read(pool, creator_filter, None).await?;
    let limit = limit.clamp(1, 250);
    let rows = match (creator_filter, status_filter) {
        (Some(creator_id), Some(status)) => {
            sqlx::query(
                "SELECT id FROM live_ingest_sessions WHERE creator_id = ? AND status = ? ORDER BY connected_at DESC LIMIT ?",
            )
            .bind(creator_id)
            .bind(status)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        (Some(creator_id), None) => {
            sqlx::query(
                "SELECT id FROM live_ingest_sessions WHERE creator_id = ? ORDER BY connected_at DESC LIMIT ?",
            )
            .bind(creator_id)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        (None, Some(status)) => {
            sqlx::query(
                "SELECT id FROM live_ingest_sessions WHERE status = ? ORDER BY connected_at DESC LIMIT ?",
            )
            .bind(status)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        (None, None) => {
            sqlx::query("SELECT id FROM live_ingest_sessions ORDER BY connected_at DESC LIMIT ?")
                .bind(limit)
                .fetch_all(pool)
                .await?
        }
    };

    let mut records = Vec::with_capacity(rows.len());
    for row in rows {
        let session_id: String = row.get("id");
        records.push(fetch_admin_live_ingest_session_record(pool, &session_id).await?);
    }
    Ok(records)
}

pub(crate) async fn fetch_admin_live_ingest_session_record(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<AdminLiveIngestSessionRecord> {
    let session = fetch_live_ingest_session_by_id_global(pool, session_id).await?;
    let recent_events = fetch_live_ingest_events_for_session(pool, session_id, 20).await?;
    Ok(AdminLiveIngestSessionRecord {
        stale_connection: is_live_ingest_session_stale(&session),
        session,
        recent_events,
    })
}

pub(crate) async fn fetch_creator_live_ingest_session_record(
    pool: &SqlitePool,
    creator_id: &str,
    session_id: &str,
) -> AppResult<AdminLiveIngestSessionRecord> {
    let session = fetch_live_ingest_session_by_id(pool, creator_id, session_id).await?;
    let recent_events = fetch_live_ingest_events_for_session(pool, session_id, 20).await?;
    Ok(AdminLiveIngestSessionRecord {
        stale_connection: is_live_ingest_session_stale(&session),
        session,
        recent_events,
    })
}
