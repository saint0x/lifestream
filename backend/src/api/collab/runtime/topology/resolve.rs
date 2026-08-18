use super::*;

pub(super) async fn resolve_host_source_ingest_session(
    pool: &SqlitePool,
    session: &CollaborationSessionView,
) -> AppResult<Option<LiveIngestSession>> {
    let Some(active) =
        fetch_active_live_ingest_session_unreconciled(pool, &session.host_creator_id).await?
    else {
        return Ok(None);
    };
    if active.broadcast_id == session.source_broadcast_id {
        return Ok(Some(active));
    }
    Ok(None)
}
