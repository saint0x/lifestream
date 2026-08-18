use super::*;

pub(super) fn validate_collaboration_role(role: &str) -> AppResult<()> {
    match role {
        "guest" | "co_host" | "co_streamer" => Ok(()),
        other => Err(AppError::BadRequest(format!(
            "unsupported collaboration role: {other}"
        ))),
    }
}

pub(super) fn validate_profile_update(input: &UpdateProfileRequest) -> AppResult<()> {
    if let Some(display_name) = input.display_name.as_deref() {
        let trimmed = display_name.trim();
        if trimmed.is_empty() {
            return Err(AppError::BadRequest(
                "displayName cannot be empty".to_string(),
            ));
        }
        if trimmed.len() > 64 {
            return Err(AppError::BadRequest(
                "displayName must be 64 characters or fewer".to_string(),
            ));
        }
    }
    if let Some(email) = input.email.as_deref() {
        let trimmed = email.trim();
        if trimmed.is_empty() || !trimmed.contains('@') || !trimmed.contains('.') {
            return Err(AppError::BadRequest(
                "email must be a valid email address".to_string(),
            ));
        }
        if trimmed.len() > 254 {
            return Err(AppError::BadRequest(
                "email must be 254 characters or fewer".to_string(),
            ));
        }
    }
    if let Some(default_audio) = input.default_audio.as_deref() {
        validate_allowed_value(
            "defaultAudio",
            default_audio,
            &[
                "English 5.1 (Dolby Atmos)",
                "English Stereo",
                "Original language",
            ],
        )?;
    }
    if let Some(subtitle_preset) = input.subtitle_preset.as_deref() {
        validate_allowed_value(
            "subtitlePreset",
            subtitle_preset,
            &[
                "Off",
                "English · Small",
                "English · Medium",
                "English · Large",
                "English · Large · High contrast",
            ],
        )?;
    }
    if let Some(filter) = input.live_chat_filter.as_deref() {
        validate_allowed_value("liveChatFilter", filter, &["Strict", "Standard", "Off"])?;
    }
    Ok(())
}

pub(super) fn validate_settings_update(input: &UpdateSettingsRequest) -> AppResult<()> {
    if let Some(playback) = input.playback.as_ref() {
        validate_allowed_value(
            "playback.defaultQuality",
            &playback.default_quality,
            &["Auto (up to 4K HDR)", "1080p", "720p", "Data saver"],
        )?;
        validate_allowed_value(
            "playback.audioLanguage",
            &playback.audio_language,
            &[
                "English · 5.1 (Dolby Atmos)",
                "English · Stereo",
                "Original language",
            ],
        )?;
        validate_allowed_value(
            "playback.subtitleLanguage",
            &playback.subtitle_language,
            &[
                "English", "French", "German", "Spanish", "Japanese", "Korean",
            ],
        )?;
        validate_allowed_value(
            "playback.subtitleStyle",
            &playback.subtitle_style,
            &[
                "Off",
                "English · Small",
                "English · Medium",
                "English · Large",
                "English · High contrast",
            ],
        )?;
        validate_allowed_value(
            "playback.playbackSpeed",
            &playback.playback_speed,
            &["1× (normal)", "1.25×", "1.5×", "1.75×", "2×"],
        )?;
    }
    if let Some(notifications) = input.notifications.as_ref() {
        if !notifications.security_alerts.push || !notifications.security_alerts.email {
            return Err(AppError::BadRequest(
                "securityAlerts must keep push and email enabled".to_string(),
            ));
        }
    }
    if let Some(privacy) = input.privacy.as_ref() {
        if privacy.data_export_size_mb < 0.0 {
            return Err(AppError::BadRequest(
                "privacy.dataExportSizeMb must be >= 0".to_string(),
            ));
        }
        if !(0..=365).contains(&privacy.delete_cooldown_days) {
            return Err(AppError::BadRequest(
                "privacy.deleteCooldownDays must be between 0 and 365".to_string(),
            ));
        }
    }
    if let Some(parental) = input.parental.as_ref() {
        validate_allowed_value(
            "parental.maxRating",
            &parental.max_rating,
            &["G", "PG", "PG-13", "TV-14", "TV-MA / R"],
        )?;
    }
    if let Some(downloads) = input.downloads.as_ref() {
        validate_allowed_value(
            "downloads.videoQuality",
            &downloads.video_quality,
            &["Standard (720p)", "High (1080p)", "Ultra (4K)"],
        )?;
        if downloads.storage_used_gb < 0.0 {
            return Err(AppError::BadRequest(
                "downloads.storageUsedGb must be >= 0".to_string(),
            ));
        }
        if downloads.storage_limit_gb <= 0.0 {
            return Err(AppError::BadRequest(
                "downloads.storageLimitGb must be > 0".to_string(),
            ));
        }
        if downloads.storage_used_gb > downloads.storage_limit_gb {
            return Err(AppError::BadRequest(
                "downloads.storageUsedGb cannot exceed storageLimitGb".to_string(),
            ));
        }
        if downloads.device_limit <= 0 {
            return Err(AppError::BadRequest(
                "downloads.deviceLimit must be > 0".to_string(),
            ));
        }
        if downloads.active_devices < 0 || downloads.active_devices > downloads.device_limit {
            return Err(AppError::BadRequest(
                "downloads.activeDevices must be between 0 and deviceLimit".to_string(),
            ));
        }
    }
    if let Some(language) = input.language.as_ref() {
        validate_allowed_value(
            "language.interfaceLanguage",
            &language.interface_language,
            &[
                "English (US)",
                "English (UK)",
                "Français",
                "Deutsch",
                "Español",
                "日本語",
                "한국어",
            ],
        )?;
        validate_allowed_value(
            "language.subtitleLanguage",
            &language.subtitle_language,
            &[
                "English", "French", "German", "Spanish", "Japanese", "Korean",
            ],
        )?;
        validate_allowed_value(
            "language.catalogRegion",
            &language.catalog_region,
            &[
                "United States",
                "United Kingdom",
                "Canada",
                "Germany",
                "Japan",
            ],
        )?;
        validate_allowed_value(
            "language.dateFormat",
            &language.date_format,
            &["MMM D, YYYY", "D MMM YYYY", "YYYY-MM-DD"],
        )?;
        validate_allowed_value(
            "language.clockFormat",
            &language.clock_format,
            &["Auto", "12 hour", "24 hour"],
        )?;
    }
    Ok(())
}

pub(super) fn validate_allowed_value(field: &str, value: &str, allowed: &[&str]) -> AppResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(AppError::BadRequest(format!(
            "{field} contains an unsupported value"
        )))
    }
}

pub(super) fn validate_collaboration_participant_state(state: &str) -> AppResult<()> {
    match state {
        "accepted" | "backstage" | "live" | "removed" | "left" => Ok(()),
        other => Err(AppError::BadRequest(format!(
            "unsupported collaboration participant state: {other}"
        ))),
    }
}

pub(super) fn validate_collaboration_chat_mode(chat_mode: &str) -> AppResult<()> {
    match chat_mode {
        "shared" | "host_only" => Ok(()),
        other => Err(AppError::BadRequest(format!(
            "unsupported collaboration chat mode: {other}"
        ))),
    }
}

pub(super) fn validate_collaboration_recording_policy(recording_policy: &str) -> AppResult<()> {
    match recording_policy {
        "host_archive" | "split_archive" => Ok(()),
        other => Err(AppError::BadRequest(format!(
            "unsupported collaboration recording policy: {other}"
        ))),
    }
}

pub(super) fn validate_collaboration_participant_transition(
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

pub(super) fn validate_pending_collaboration_invite(invite: &CollaborationInvite) -> AppResult<()> {
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

pub(super) fn validate_redeemable_collaboration_mirror_grant(
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

pub(super) fn transition_creator_operational_status(
    current: &str,
    submit_requested: bool,
    terminal_approved: &str,
    terminal_blocked: &str,
) -> AppResult<String> {
    if current == terminal_approved {
        return Ok(current.to_string());
    }
    if current == terminal_blocked && submit_requested {
        return Ok("submitted".to_string());
    }
    if submit_requested {
        return Ok(match current {
            "pending" | "rejected" | "disabled" => "submitted".to_string(),
            "in_review" => "in_review".to_string(),
            "submitted" => "submitted".to_string(),
            other => other.to_string(),
        });
    }
    Ok(current.to_string())
}

pub(super) fn monetized_access_policy(access_policy: &str) -> bool {
    matches!(
        access_policy,
        "subscription" | "purchase" | "subscription_or_purchase"
    )
}

pub(super) fn parse_optional_future_timestamp(value: Option<&str>) -> AppResult<Option<String>> {
    let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let parsed = chrono::DateTime::parse_from_rfc3339(raw)
        .map_err(|_| {
            AppError::BadRequest("expiresAt must be a valid RFC3339 timestamp".to_string())
        })?
        .with_timezone(&Utc);
    if parsed <= Utc::now() {
        return Err(AppError::BadRequest(
            "expiresAt must be in the future".to_string(),
        ));
    }
    Ok(Some(parsed.to_rfc3339()))
}
