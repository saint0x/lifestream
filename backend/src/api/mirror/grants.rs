use super::collaboration_events::{
    publish_collaboration_event, publish_collaboration_mirror_grant_revoked_events,
    publish_collaboration_mirror_grant_revoked_events_raw,
};
use super::pickups::activate_collaboration_mirror_pickup;
use super::queries::{
    fetch_collaboration_mirror_grant_by_id,
    fetch_revocable_collaboration_mirror_grants_for_participant,
    fetch_revocable_collaboration_mirror_grants_for_session,
};
use super::*;

pub(crate) async fn revoke_collaboration_mirror_grants_for_participant(
    state: &SharedState,
    session_id: &str,
    participant_id: &str,
    actor_user_id: Option<String>,
    revoked_at: &str,
    reason: &str,
) -> AppResult<()> {
    let grants = fetch_revocable_collaboration_mirror_grants_for_participant(
        state.db.sqlite_adapter(),
        participant_id,
    )
    .await?;
    if grants.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "UPDATE collaboration_mirror_grants SET state = 'revoked', revoked_at = COALESCE(revoked_at, ?) WHERE participant_id = ? AND state IN ('issued', 'active')",
    )
    .bind(revoked_at)
    .bind(participant_id)
    .execute(state.db.sqlite_adapter())
    .await?;
    deactivate_collaboration_mirror_pickups_for_grants(state, &grants, "revoked", revoked_at)
        .await?;
    publish_collaboration_mirror_grant_revoked_events(
        state,
        session_id,
        actor_user_id,
        &grants,
        revoked_at,
        reason,
    )
    .await?;
    Ok(())
}

pub(crate) async fn revoke_collaboration_mirror_grants_for_session(
    state: &SharedState,
    session_id: &str,
    actor_user_id: Option<String>,
    revoked_at: &str,
    reason: &str,
) -> AppResult<()> {
    let grants = fetch_revocable_collaboration_mirror_grants_for_session(
        state.db.sqlite_adapter(),
        session_id,
    )
    .await?;
    if grants.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "UPDATE collaboration_mirror_grants SET state = 'revoked', revoked_at = COALESCE(revoked_at, ?) WHERE session_id = ? AND state IN ('issued', 'active')",
    )
    .bind(revoked_at)
    .bind(session_id)
    .execute(state.db.sqlite_adapter())
    .await?;
    deactivate_collaboration_mirror_pickups_for_grants(state, &grants, "revoked", revoked_at)
        .await?;
    publish_collaboration_mirror_grant_revoked_events(
        state,
        session_id,
        actor_user_id,
        &grants,
        revoked_at,
        reason,
    )
    .await?;
    Ok(())
}

pub(crate) async fn revoke_collaboration_mirror_grants_for_session_raw(
    state: &SharedState,
    session_id: &str,
    actor_user_id: Option<String>,
    revoked_at: &str,
    reason: &str,
) -> AppResult<()> {
    let grants = fetch_revocable_collaboration_mirror_grants_for_session(
        state.db.sqlite_adapter(),
        session_id,
    )
    .await?;
    if grants.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "UPDATE collaboration_mirror_grants SET state = 'revoked', revoked_at = COALESCE(revoked_at, ?) WHERE session_id = ? AND state IN ('issued', 'active')",
    )
    .bind(revoked_at)
    .bind(session_id)
    .execute(state.db.sqlite_adapter())
    .await?;
    deactivate_collaboration_mirror_pickups_for_grants(state, &grants, "revoked", revoked_at)
        .await?;
    publish_collaboration_mirror_grant_revoked_events_raw(
        state,
        session_id,
        actor_user_id,
        &grants,
        revoked_at,
        reason,
    )
    .await?;
    Ok(())
}

pub(crate) async fn issue_mirror_grant_for_participant(
    state: &SharedState,
    session: &CollaborationSession,
    participant: &CollaborationParticipant,
    actor_user_id: &str,
) -> AppResult<CollaborationMirrorGrant> {
    if participant.role == "host" {
        return Err(AppError::BadRequest(
            "hosts do not receive collaboration mirror grants".to_string(),
        ));
    }
    if participant.state == "left" || participant.state == "removed" {
        return Err(AppError::BadRequest(
            "inactive participants cannot receive collaboration mirror grants".to_string(),
        ));
    }
    if session.status != "active" {
        return Err(AppError::BadRequest(
            "collaboration mirror grants can only be issued for active sessions".to_string(),
        ));
    }
    if participant.state != "live" {
        return Err(AppError::BadRequest(
            "collaboration mirror grants can only be issued for live participants".to_string(),
        ));
    }
    if !participant.mirror_to_guest_channel {
        return Err(AppError::BadRequest(
            "participant is not enabled for mirrored guest channel pickup".to_string(),
        ));
    }
    let guest_creator_id = participant.creator_id.clone().ok_or_else(|| {
        AppError::BadRequest(
            "collaboration mirror grants require the participant to have a creator profile"
                .to_string(),
        )
    })?;
    let issued_at = Utc::now().to_rfc3339();
    let expires_at = (Utc::now() + chrono::Duration::hours(6)).to_rfc3339();
    let raw_token = format!("colmg-{}", Uuid::new_v4().simple());
    let grant_id = format!("colm-{}", Uuid::new_v4().simple());

    revoke_collaboration_mirror_grants_for_participant(
        state,
        &session.id,
        &participant.id,
        Some(actor_user_id.to_string()),
        &issued_at,
        "grant_reissued",
    )
    .await?;
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
    .bind(&guest_creator_id)
    .bind(participant.publish_to_host as i64)
    .bind(participant.mirror_to_guest_channel as i64)
    .bind(hash_token(&raw_token))
    .bind(&issued_at)
    .bind(&expires_at)
    .execute(state.db.sqlite_adapter())
    .await?;
    publish_collaboration_event(
        state,
        &session.id,
        Some(actor_user_id.to_string()),
        Some(participant.id.clone()),
        "mirror_grant_issued",
        json!({
            "grantId": grant_id,
            "guestCreatorId": guest_creator_id,
            "scope": "mirror_pickup",
            "publishToHost": participant.publish_to_host,
            "mirrorToGuestChannel": participant.mirror_to_guest_channel,
            "expiresAt": expires_at,
        }),
    )
    .await?;
    fetch_collaboration_mirror_grant_by_id(state.db.sqlite_adapter(), &grant_id).await
}

pub(crate) async fn redeem_collaboration_mirror_grant_internal(
    state: &SharedState,
    identity: &RequestIdentity,
    grant_id: &str,
) -> AppResult<CollaborationMirrorGrant> {
    let grant = fetch_collaboration_mirror_grant_by_id(state.db.sqlite_adapter(), grant_id).await?;
    let participant =
        fetch_collaboration_participant_by_id(state.db.sqlite_adapter(), &grant.participant_id)
            .await?;
    let session =
        fetch_collaboration_session_by_id(state.db.sqlite_adapter(), &grant.session_id).await?;
    if participant.user_id != identity.user_id {
        return Err(AppError::Forbidden);
    }
    validate_redeemable_collaboration_mirror_grant(&grant, &participant, &session)?;
    let guest_creator_id = identity.require_creator_scope()?;
    if Some(guest_creator_id) != participant.creator_id.as_deref() {
        return Err(AppError::Forbidden);
    }

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE collaboration_mirror_grants SET state = 'active', activated_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(grant_id)
    .execute(state.db.sqlite_adapter())
    .await?;
    let activated_grant =
        fetch_collaboration_mirror_grant_by_id(state.db.sqlite_adapter(), grant_id).await?;
    let pickup = match activate_collaboration_mirror_pickup(
        state,
        &session,
        &participant,
        &activated_grant,
        &now,
    )
    .await
    {
        Ok(pickup) => pickup,
        Err(error) => {
            sqlx::query(
                "UPDATE collaboration_mirror_grants SET state = 'issued', activated_at = NULL WHERE id = ? AND state = 'active'",
            )
            .bind(grant_id)
            .execute(state.db.sqlite_adapter())
            .await?;
            deactivate_collaboration_mirror_pickups_for_grants(
                state,
                std::slice::from_ref(&activated_grant),
                "revoked",
                &now,
            )
            .await?;
            return Err(error);
        }
    };
    publish_collaboration_event(
        state,
        &grant.session_id,
        Some(identity.user_id.clone()),
        Some(participant.id.clone()),
        "mirror_grant_redeemed",
        json!({
            "grantId": grant.id,
            "participantId": participant.id,
            "guestCreatorId": grant.guest_creator_id,
            "activatedAt": now,
            "guestBroadcastId": pickup.guest_broadcast_id,
            "pickupId": pickup.id,
        }),
    )
    .await?;

    fetch_collaboration_mirror_grant_by_id(state.db.sqlite_adapter(), grant_id).await
}
