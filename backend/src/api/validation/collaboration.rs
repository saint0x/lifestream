use super::*;

pub(crate) fn validate_collaboration_role(role: &str) -> AppResult<()> {
    match role {
        "guest" | "co_host" | "co_streamer" => Ok(()),
        other => Err(AppError::BadRequest(format!(
            "unsupported collaboration role: {other}"
        ))),
    }
}

pub(crate) fn validate_collaboration_participant_state(state: &str) -> AppResult<()> {
    match state {
        "accepted" | "backstage" | "live" | "removed" | "left" => Ok(()),
        other => Err(AppError::BadRequest(format!(
            "unsupported collaboration participant state: {other}"
        ))),
    }
}

pub(crate) fn validate_collaboration_chat_mode(chat_mode: &str) -> AppResult<()> {
    match chat_mode {
        "shared" | "host_only" => Ok(()),
        other => Err(AppError::BadRequest(format!(
            "unsupported collaboration chat mode: {other}"
        ))),
    }
}

pub(crate) fn validate_collaboration_recording_policy(recording_policy: &str) -> AppResult<()> {
    match recording_policy {
        "host_archive" | "split_archive" => Ok(()),
        other => Err(AppError::BadRequest(format!(
            "unsupported collaboration recording policy: {other}"
        ))),
    }
}

pub(crate) fn validate_collaboration_participant_transition(
    current: &str,
    next: &str,
    host_action: bool,
) -> AppResult<()> {
    if current == next {
        return Ok(());
    }

    let allowed = if host_action {
        matches!(
            (current, next),
            ("accepted", "backstage")
                | ("accepted", "live")
                | ("accepted", "removed")
                | ("backstage", "live")
                | ("backstage", "removed")
                | ("live", "backstage")
                | ("live", "removed")
                | ("left", "backstage")
                | ("removed", "backstage")
        )
    } else {
        matches!(
            (current, next),
            ("accepted", "backstage")
                | ("accepted", "left")
                | ("backstage", "left")
                | ("live", "left")
                | ("left", "backstage")
                | ("removed", "backstage")
        )
    };

    if allowed {
        Ok(())
    } else {
        Err(AppError::BadRequest(format!(
            "illegal collaboration participant transition: {current} -> {next}"
        )))
    }
}

pub(crate) fn validate_pending_collaboration_invite(invite: &CollaborationInvite) -> AppResult<()> {
    if invite.state != "pending" {
        return Err(AppError::BadRequest(
            "collaboration invite is no longer pending".to_string(),
        ));
    }
    let now = Utc::now().to_rfc3339();
    if invite.expires_at <= now {
        return Err(AppError::BadRequest(
            "collaboration invite has expired".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_redeemable_collaboration_mirror_grant(
    grant: &CollaborationMirrorGrant,
    participant: &CollaborationParticipant,
    session: &CollaborationSession,
) -> AppResult<()> {
    if grant.state != "issued" {
        return Err(AppError::BadRequest(
            "collaboration mirror grant is not redeemable".to_string(),
        ));
    }
    if grant.scope != "mirror_pickup" {
        return Err(AppError::BadRequest(
            "unsupported collaboration mirror grant scope".to_string(),
        ));
    }
    if !grant.mirror_to_guest_channel || !participant.mirror_to_guest_channel {
        return Err(AppError::BadRequest(
            "participant is not enabled for mirrored guest channel pickup".to_string(),
        ));
    }
    if session.status != "active" {
        return Err(AppError::BadRequest(
            "collaboration mirror grant can only be redeemed for an active session".to_string(),
        ));
    }
    let now = Utc::now().to_rfc3339();
    if grant.expires_at <= now {
        return Err(AppError::BadRequest(
            "collaboration mirror grant has expired".to_string(),
        ));
    }
    if participant.state != "live" {
        return Err(AppError::BadRequest(
            "collaboration mirror grants can only be redeemed by live participants".to_string(),
        ));
    }
    if participant.creator_id.as_deref() != Some(grant.guest_creator_id.as_str()) {
        return Err(AppError::Forbidden);
    }
    Ok(())
}
