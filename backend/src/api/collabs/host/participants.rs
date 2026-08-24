use super::*;

pub(crate) async fn update_collaboration_participant(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((session_id, participant_id)): Path<(String, String)>,
    Json(input): Json<UpdateCollaborationParticipantRequest>,
) -> AppResult<Json<CollaborationParticipant>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    ensure_creator_collaboration_enabled(state.db.sqlite_adapter(), creator_id).await?;
    let session =
        fetch_collaboration_session_for_host(state.db.sqlite_adapter(), creator_id, &session_id)
            .await?;
    Ok(Json(
        apply_collaboration_participant_update(
            &state,
            &session,
            &participant_id,
            &identity.user_id,
            &input,
        )
        .await?,
    ))
}

pub(crate) async fn apply_collaboration_participant_update(
    state: &SharedState,
    session: &CollaborationSession,
    participant_id: &str,
    actor_user_id: &str,
    input: &UpdateCollaborationParticipantRequest,
) -> AppResult<CollaborationParticipant> {
    if session.status == "ended" {
        return Err(AppError::BadRequest(
            "cannot update participants for an ended collaboration session".to_string(),
        ));
    }

    let participant =
        fetch_collaboration_participant_by_id(state.db.sqlite_adapter(), participant_id).await?;
    if participant.session_id != session.id {
        return Err(AppError::NotFound);
    }
    if participant.role == "host" {
        return Err(AppError::BadRequest(
            "the host participant cannot be mutated through collaboration controls".to_string(),
        ));
    }

    let next_state = match input.state.as_deref() {
        Some(state_value) => {
            validate_collaboration_participant_state(state_value)?;
            validate_collaboration_participant_transition(&participant.state, state_value, true)?;
            state_value.to_string()
        }
        None => participant.state.clone(),
    };
    let publish_to_host = input.publish_to_host.unwrap_or(participant.publish_to_host);
    let mirror_to_guest_channel = input
        .mirror_to_guest_channel
        .unwrap_or(participant.mirror_to_guest_channel);
    let can_speak_in_chat = input
        .can_speak_in_chat
        .unwrap_or(participant.can_speak_in_chat);
    let media_transport = input
        .media_transport
        .clone()
        .or_else(|| participant.media_transport.clone());
    let contribution_endpoint_url = input
        .contribution_endpoint_url
        .clone()
        .or_else(|| participant.contribution_endpoint_url.clone());
    let return_endpoint_url = input
        .return_endpoint_url
        .clone()
        .or_else(|| participant.return_endpoint_url.clone());

    if mirror_to_guest_channel && participant.creator_id.is_none() {
        return Err(AppError::BadRequest(
            "participant must have a creator profile to mirror to their guest channel".to_string(),
        ));
    }
    validate_collaboration_media_transport(
        &next_state,
        publish_to_host,
        media_transport.as_deref(),
        contribution_endpoint_url.as_deref(),
        return_endpoint_url.as_deref(),
    )?;

    let now = Utc::now().to_rfc3339();
    let left_at = if matches!(next_state.as_str(), "left" | "removed") {
        Some(participant.left_at.clone().unwrap_or_else(|| now.clone()))
    } else {
        None
    };
    sqlx::query(
        r#"
        UPDATE collaboration_participants
        SET state = ?, publish_to_host = ?, mirror_to_guest_channel = ?, can_speak_in_chat = ?,
            media_transport = ?, contribution_endpoint_url = ?, return_endpoint_url = ?,
            left_at = ?, updated_at = ?
        WHERE id = ? AND session_id = ?
        "#,
    )
    .bind(&next_state)
    .bind(publish_to_host as i64)
    .bind(mirror_to_guest_channel as i64)
    .bind(can_speak_in_chat as i64)
    .bind(&media_transport)
    .bind(&contribution_endpoint_url)
    .bind(&return_endpoint_url)
    .bind(left_at)
    .bind(&now)
    .bind(participant_id)
    .bind(&session.id)
    .execute(state.db.sqlite_adapter())
    .await?;

    if !matches!(next_state.as_str(), "live") || !mirror_to_guest_channel {
        revoke_collaboration_mirror_grants_for_participant(
            state,
            &session.id,
            participant_id,
            Some(actor_user_id.to_string()),
            &now,
            "participant_updated",
        )
        .await?;
    }

    publish_collaboration_event(
        state,
        &session.id,
        Some(actor_user_id.to_string()),
        Some(participant_id.to_string()),
        "participant_updated",
        json!({
            "participantId": participant_id,
            "previousState": participant.state,
            "state": next_state,
            "publishToHost": publish_to_host,
            "mirrorToGuestChannel": mirror_to_guest_channel,
            "canSpeakInChat": can_speak_in_chat,
            "mediaTransport": media_transport,
            "contributionEndpointUrl": contribution_endpoint_url,
            "returnEndpointUrl": return_endpoint_url,
            "updatedAt": now,
        }),
    )
    .await?;

    fetch_collaboration_participant_by_id(state.db.sqlite_adapter(), participant_id).await
}

fn validate_collaboration_media_transport(
    next_state: &str,
    publish_to_host: bool,
    media_transport: Option<&str>,
    contribution_endpoint_url: Option<&str>,
    return_endpoint_url: Option<&str>,
) -> AppResult<()> {
    if !publish_to_host || next_state != "live" {
        return Ok(());
    }
    if media_transport.is_none()
        && contribution_endpoint_url.is_none()
        && return_endpoint_url.is_none()
    {
        return Ok(());
    }
    match media_transport {
        Some("rtmp" | "rtmps" | "srt") => {}
        Some(_) => {
            return Err(AppError::BadRequest(
                "mediaTransport must be rtmp, rtmps, or srt when collaboration media endpoints are declared".to_string(),
            ));
        }
        None => {
            return Err(AppError::BadRequest(
                "mediaTransport is required when collaboration media endpoints are declared"
                    .to_string(),
            ));
        }
    }
    if contribution_endpoint_url.is_none_or(str::is_empty) {
        return Err(AppError::BadRequest(
            "contributionEndpointUrl is required when collaboration media transport is declared"
                .to_string(),
        ));
    }
    if return_endpoint_url.is_none_or(str::is_empty) {
        return Err(AppError::BadRequest(
            "returnEndpointUrl is required when collaboration media transport is declared"
                .to_string(),
        ));
    }
    Ok(())
}

pub(crate) async fn remove_collaboration_participant(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((session_id, participant_id)): Path<(String, String)>,
) -> AppResult<Json<CollaborationParticipant>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let session =
        fetch_collaboration_session_for_host(state.db.sqlite_adapter(), creator_id, &session_id)
            .await?;
    if session.status == "ended" {
        return Err(AppError::BadRequest(
            "cannot remove participants from an ended collaboration session".to_string(),
        ));
    }
    let participant =
        fetch_collaboration_participant_by_id(state.db.sqlite_adapter(), &participant_id).await?;
    if participant.session_id != session_id {
        return Err(AppError::NotFound);
    }
    if participant.role == "host" {
        return Err(AppError::BadRequest(
            "the host cannot be removed from a collaboration session".to_string(),
        ));
    }
    if participant.state == "removed" {
        return Ok(Json(participant));
    }
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        UPDATE collaboration_participants
        SET state = 'removed', left_at = COALESCE(left_at, ?), updated_at = ?
        WHERE id = ? AND session_id = ?
        "#,
    )
    .bind(&now)
    .bind(&now)
    .bind(&participant_id)
    .bind(&session_id)
    .execute(state.db.sqlite_adapter())
    .await?;
    revoke_collaboration_mirror_grants_for_participant(
        &state,
        &session_id,
        &participant_id,
        Some(identity.user_id.clone()),
        &now,
        "participant_removed",
    )
    .await?;
    publish_collaboration_event(
        &state,
        &session_id,
        Some(identity.user_id.clone()),
        Some(participant_id.clone()),
        "participant_removed",
        json!({
            "participantId": participant_id,
            "removedAt": now,
            "reason": "host_removed",
        }),
    )
    .await?;
    Ok(Json(
        fetch_collaboration_participant_by_id(state.db.sqlite_adapter(), &participant_id).await?,
    ))
}
