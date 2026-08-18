use super::*;

pub(crate) fn validate_profile_update(input: &UpdateProfileRequest) -> AppResult<()> {
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

pub(crate) fn validate_settings_update(input: &UpdateSettingsRequest) -> AppResult<()> {
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

fn validate_allowed_value(field: &str, value: &str, allowed: &[&str]) -> AppResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(AppError::BadRequest(format!(
            "{field} contains an unsupported value"
        )))
    }
}
