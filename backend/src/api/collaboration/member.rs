use super::super::discovery::fetch_creator_id_for_user;
use super::super::realtime::{
    reconcile_collaboration_expiry_for_participant_read,
    reconcile_collaboration_session_expiry_for_read,
};
use super::*;

pub(crate) async fn list_my_collaboration_invites(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<CollaborationInvite>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    reconcile_collaboration_expiry_for_participant_read(&state, &identity.user_id).await?;
    Ok(Json(
        fetch_collaboration_invites_for_user(&state.pool, &identity.user_id).await?,
    ))
}

pub(crate) async fn list_my_collaboration_sessions(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<CollaborationSessionView>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    reconcile_collaboration_expiry_for_participant_read(&state, &identity.user_id).await?;
    Ok(Json(
        fetch_collaboration_sessions_for_participant(&state.pool, &identity.user_id).await?,
    ))
}

pub(crate) async fn list_my_collaboration_events(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Query(query): Query<CollaborationEventsQuery>,
) -> AppResult<Json<Vec<CollaborationEvent>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    reconcile_collaboration_session_expiry_for_read(&state, &session_id).await?;
    let session =
        fetch_collaboration_session_for_participant(&state.pool, &identity.user_id, &session_id)
            .await?;
    Ok(Json(filter_visible_collaboration_events_for_session(
        &session,
        fetch_collaboration_events(
            &state.pool,
            &session_id,
            query.after_seq.unwrap_or(0),
            query.limit.unwrap_or(100),
        )
        .await?,
    )))
}

pub(crate) async fn get_my_collaboration_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<CollaborationSessionView>> {
    let identity = require_identity(&state.pool, &headers).await?;
    reconcile_collaboration_session_expiry_for_read(&state, &session_id).await?;
    Ok(Json(
        fetch_collaboration_session_for_participant(&state.pool, &identity.user_id, &session_id)
            .await?,
    ))
}

pub(crate) async fn get_my_collaboration_runtime(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<CollaborationRuntimeResponse>> {
    let identity = require_identity(&state.pool, &headers).await?;
    reconcile_collaboration_session_expiry_for_read(&state, &session_id).await?;
    let session =
        fetch_collaboration_session_for_participant(&state.pool, &identity.user_id, &session_id)
            .await?;
    Ok(Json(
        build_collaboration_runtime_response_for_participant(&state.pool, session).await?,
    ))
}

pub(crate) async fn list_my_collaboration_mirror_grants(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<Vec<CollaborationMirrorGrant>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    reconcile_collaboration_session_expiry_for_read(&state, &session_id).await?;
    let participant =
        fetch_collaboration_participant_for_user(&state.pool, &session_id, &identity.user_id)
            .await?;
    validate_collaboration_participant_access(&participant)?;
    Ok(Json(
        fetch_collaboration_mirror_grants_for_participant(&state.pool, &participant.id).await?,
    ))
}

pub(crate) async fn leave_my_collaboration_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<CollaborationParticipant>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let participant =
        fetch_collaboration_participant_for_user(&state.pool, &session_id, &identity.user_id)
            .await?;
    if participant.role == "host" {
        return Err(AppError::BadRequest(
            "hosts must end the collaboration session instead of leaving it".to_string(),
        ));
    }
    if participant.state == "left" || participant.state == "removed" {
        return Ok(Json(participant));
    }

    let session = fetch_collaboration_session_by_id(&state.pool, &session_id).await?;
    if session.status == "ended" {
        return Ok(Json(participant));
    }
    validate_collaboration_participant_transition(&participant.state, "left", false)?;

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        UPDATE collaboration_participants
        SET state = 'left', left_at = COALESCE(left_at, ?), updated_at = ?
        WHERE id = ? AND session_id = ?
        "#,
    )
    .bind(&now)
    .bind(&now)
    .bind(&participant.id)
    .bind(&session_id)
    .execute(&state.pool)
    .await?;
    revoke_collaboration_mirror_grants_for_participant(
        &state,
        &session_id,
        &participant.id,
        Some(identity.user_id.clone()),
        &now,
        "participant_left",
    )
    .await?;
    publish_collaboration_event(
        &state,
        &session_id,
        Some(identity.user_id.clone()),
        Some(participant.id.clone()),
        "participant_left",
        json!({
            "participantId": participant.id,
            "leftAt": now,
        }),
    )
    .await?;

    Ok(Json(
        fetch_collaboration_participant_by_id(&state.pool, &participant.id).await?,
    ))
}

pub(crate) async fn accept_collaboration_invite(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(invite_id): Path<String>,
) -> AppResult<Json<CollaborationParticipant>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let invite = fetch_collaboration_invite_by_id(&state.pool, &invite_id).await?;
    if invite.invitee_user_id != identity.user_id {
        return Err(AppError::Forbidden);
    }
    validate_pending_collaboration_invite(&invite)?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE collaboration_invites SET state = 'accepted', responded_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(&invite_id)
    .execute(&state.pool)
    .await?;
    let creator_id = fetch_creator_id_for_user(&state.pool, &identity.user_id).await?;
    let participant = match fetch_collaboration_participant_for_user(
        &state.pool,
        &invite.session_id,
        &identity.user_id,
    )
    .await
    {
        Ok(existing) => {
            validate_collaboration_participant_transition(&existing.state, "backstage", false)?;
            sqlx::query(
                r#"
                UPDATE collaboration_participants
                SET invite_id = ?, creator_id = ?, role = ?, state = 'backstage',
                    publish_to_host = 1, mirror_to_guest_channel = ?, can_speak_in_chat = 1,
                    joined_at = ?, left_at = NULL, updated_at = ?
                WHERE id = ? AND session_id = ?
                "#,
            )
            .bind(&invite.id)
            .bind(creator_id)
            .bind(&invite.role)
            .bind(invite.mirror_to_guest_channel as i64)
            .bind(&now)
            .bind(&now)
            .bind(&existing.id)
            .bind(&invite.session_id)
            .execute(&state.pool)
            .await?;
            let rejoined = fetch_collaboration_participant_by_id(&state.pool, &existing.id).await?;
            publish_collaboration_event(
                &state,
                &invite.session_id,
                Some(identity.user_id.clone()),
                Some(existing.id.clone()),
                "participant_rejoined",
                json!({
                    "inviteId": invite.id,
                    "participantId": existing.id,
                    "role": invite.role,
                    "mirrorToGuestChannel": invite.mirror_to_guest_channel,
                    "rejoinedAt": now,
                }),
            )
            .await?;
            rejoined
        }
        Err(AppError::NotFound) => {
            let participant_id = format!("colp-{}", Uuid::new_v4().simple());
            sqlx::query(
                r#"
                INSERT INTO collaboration_participants (
                    id, session_id, invite_id, user_id, creator_id, role, state, publish_to_host,
                    mirror_to_guest_channel, can_speak_in_chat, joined_at, left_at, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, 'backstage', 1, ?, 1, ?, NULL, ?, ?)
                "#,
            )
            .bind(&participant_id)
            .bind(&invite.session_id)
            .bind(&invite.id)
            .bind(&identity.user_id)
            .bind(creator_id)
            .bind(&invite.role)
            .bind(invite.mirror_to_guest_channel as i64)
            .bind(&now)
            .bind(&now)
            .bind(&now)
            .execute(&state.pool)
            .await?;
            publish_collaboration_event(
                &state,
                &invite.session_id,
                Some(identity.user_id.clone()),
                Some(participant_id.clone()),
                "invite_accepted",
                json!({
                    "inviteId": invite.id,
                    "participantId": participant_id,
                    "role": invite.role,
                    "mirrorToGuestChannel": invite.mirror_to_guest_channel,
                }),
            )
            .await?;
            fetch_collaboration_participant_by_id(&state.pool, &participant_id).await?
        }
        Err(error) => return Err(error),
    };
    Ok(Json(participant))
}

pub(crate) async fn decline_collaboration_invite(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(invite_id): Path<String>,
) -> AppResult<Json<CollaborationInvite>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let invite = fetch_collaboration_invite_by_id(&state.pool, &invite_id).await?;
    if invite.invitee_user_id != identity.user_id {
        return Err(AppError::Forbidden);
    }
    validate_pending_collaboration_invite(&invite)?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE collaboration_invites SET state = 'declined', responded_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(&invite_id)
    .execute(&state.pool)
    .await?;
    publish_collaboration_event(
        &state,
        &invite.session_id,
        Some(identity.user_id.clone()),
        None,
        "invite_declined",
        json!({
            "inviteId": invite.id,
            "inviteeUserId": identity.user_id,
        }),
    )
    .await?;
    Ok(Json(
        fetch_collaboration_invite_by_id(&state.pool, &invite_id).await?,
    ))
}

pub(crate) async fn redeem_collaboration_mirror_grant(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(grant_id): Path<String>,
) -> AppResult<Json<CollaborationMirrorGrant>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        redeem_collaboration_mirror_grant_internal(&state, &identity, &grant_id).await?,
    ))
}
