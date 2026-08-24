use super::*;

pub(crate) async fn accept_collaboration_invite(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(invite_id): Path<String>,
) -> AppResult<Json<CollaborationParticipant>> {
    let identity = require_identity(&state.db, &headers).await?;
    let invite = fetch_collaboration_invite_by_id(state.db.sqlite_adapter(), &invite_id).await?;
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
    .execute(state.db.sqlite_adapter())
    .await?;
    let creator_id =
        fetch_creator_id_for_user(state.db.sqlite_adapter(), &identity.user_id).await?;
    let participant = match fetch_collaboration_participant_for_user(
        state.db.sqlite_adapter(),
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
            .execute(state.db.sqlite_adapter())
            .await?;
            let rejoined =
                fetch_collaboration_participant_by_id(state.db.sqlite_adapter(), &existing.id)
                    .await?;
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
            .execute(state.db.sqlite_adapter())
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
            fetch_collaboration_participant_by_id(state.db.sqlite_adapter(), &participant_id)
                .await?
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
    let identity = require_identity(&state.db, &headers).await?;
    let invite = fetch_collaboration_invite_by_id(state.db.sqlite_adapter(), &invite_id).await?;
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
    .execute(state.db.sqlite_adapter())
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
        fetch_collaboration_invite_by_id(state.db.sqlite_adapter(), &invite_id).await?,
    ))
}
