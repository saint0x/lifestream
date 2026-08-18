use super::*;

pub(crate) async fn activate_collaboration_mirror_pickup(
    state: &SharedState,
    session: &CollaborationSession,
    participant: &CollaborationParticipant,
    grant: &CollaborationMirrorGrant,
    activated_at: &str,
) -> AppResult<CollaborationMirrorPickup> {
    let guest_creator_id = participant.creator_id.clone().ok_or_else(|| {
        AppError::BadRequest(
            "collaboration mirror pickup requires the participant to have a creator profile"
                .to_string(),
        )
    })?;
    let guest_broadcast = ensure_guest_broadcast_available_for_mirror_pickup(
        &state.pool,
        session,
        participant,
        &guest_creator_id,
    )
    .await?;
    let pickup_id = format!("colmp-{}", Uuid::new_v4().simple());
    sqlx::query(
        r#"
        INSERT INTO collaboration_mirror_pickups (
            id, session_id, participant_id, grant_id, host_creator_id, guest_creator_id,
            source_broadcast_id, guest_broadcast_id, state, activated_at, updated_at, ended_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, NULL)
        "#,
    )
    .bind(&pickup_id)
    .bind(&session.id)
    .bind(&participant.id)
    .bind(&grant.id)
    .bind(&session.host_creator_id)
    .bind(&guest_creator_id)
    .bind(&session.source_broadcast_id)
    .bind(&guest_broadcast.id)
    .bind(activated_at)
    .bind(activated_at)
    .execute(&state.pool)
    .await?;

    let pickup = fetch_active_collaboration_mirror_pickup_for_grant(&state.pool, &grant.id)
        .await?
        .ok_or_else(|| {
            AppError::Internal("activated collaboration mirror pickup is missing".to_string())
        })?;
    sync_collaboration_mirror_pickup_broadcast_state(&state.pool, &pickup).await?;
    publish_authoritative_creator_live_state(state, &pickup.guest_creator_id).await?;
    Ok(pickup)
}

pub(crate) async fn deactivate_collaboration_mirror_pickups_for_grants(
    state: &SharedState,
    grants: &[CollaborationMirrorGrant],
    terminal_state: &str,
    ended_at: &str,
) -> AppResult<()> {
    let pickups = fetch_active_collaboration_mirror_pickups_for_grants(&state.pool, grants).await?;
    let mut guest_creator_ids = Vec::new();
    for pickup in pickups {
        guest_creator_ids.push(pickup.guest_creator_id.clone());
        sqlx::query(
            "UPDATE collaboration_mirror_pickups SET state = ?, updated_at = ?, ended_at = COALESCE(ended_at, ?) WHERE id = ?",
        )
        .bind(terminal_state)
        .bind(ended_at)
        .bind(ended_at)
        .bind(&pickup.id)
        .execute(&state.pool)
        .await?;

        let guest_creator = fetch_creator_profile(&state.pool, &pickup.guest_creator_id).await?;
        let guest_broadcast = fetch_broadcast_by_id(
            &state.pool,
            &pickup.guest_creator_id,
            &pickup.guest_broadcast_id,
        )
        .await?;
        let started_at = chrono::DateTime::parse_from_rfc3339(&guest_broadcast.started_at)
            .map_err(|_| {
                AppError::BadRequest(
                    "invalid collaboration mirror pickup broadcast timestamp".to_string(),
                )
            })?
            .with_timezone(&Utc);
        let ended = chrono::DateTime::parse_from_rfc3339(ended_at)
            .map_err(|_| {
                AppError::BadRequest(
                    "invalid collaboration mirror pickup end timestamp".to_string(),
                )
            })?
            .with_timezone(&Utc);
        let duration_sec = (ended - started_at).num_seconds().max(0);

        sqlx::query(
            "UPDATE broadcasts SET status = 'ended', ended_at = COALESCE(ended_at, ?), duration_sec = COALESCE(duration_sec, ?) WHERE id = ? AND creator_id = ?",
        )
        .bind(ended_at)
        .bind(duration_sec)
        .bind(&pickup.guest_broadcast_id)
        .bind(&pickup.guest_creator_id)
        .execute(&state.pool)
        .await?;

        sqlx::query("DELETE FROM live_streams WHERE id = ?")
            .bind(format!("lv-{}-live", guest_creator.handle))
            .execute(&state.pool)
            .await?;
        sqlx::query("UPDATE streamers SET is_live = 0 WHERE handle = ?")
            .bind(&guest_creator.handle)
            .execute(&state.pool)
            .await?;
        reset_creator_live_operational_metrics(&state.pool, &pickup.guest_creator_id).await?;
        let guest_broadcasts = fetch_broadcasts(&state.pool, &pickup.guest_creator_id).await?;
        let _ =
            normalize_creator_live_profile(&state.pool, &pickup.guest_creator_id, guest_broadcasts)
                .await?;
    }
    publish_creator_live_states_for_creators(state, guest_creator_ids).await?;
    Ok(())
}
