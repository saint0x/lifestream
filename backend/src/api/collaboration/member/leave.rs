use super::*;

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
