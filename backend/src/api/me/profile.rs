use super::*;

pub(crate) async fn get_my_profile(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<UserProfileDetails>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_user_profile_details(&state.pool, &identity.user_id).await?,
    ))
}

pub(crate) async fn update_my_profile(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<UpdateProfileRequest>,
) -> AppResult<Json<UserProfileDetails>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let current = fetch_user_profile_details(&state.pool, &identity.user_id).await?;
    validate_profile_update(&input)?;

    sqlx::query("UPDATE users SET display_name = ? WHERE id = ?")
        .bind(
            input
                .display_name
                .as_deref()
                .unwrap_or(current.user.display_name.as_str()),
        )
        .bind(&identity.user_id)
        .execute(&state.pool)
        .await?;

    sqlx::query(
        r#"
        UPDATE user_profiles
        SET email = ?, mature_content_allowed = ?, default_audio = ?, subtitle_preset = ?,
            autoplay_trailers = ?, live_chat_filter = ?
        WHERE user_id = ?
        "#,
    )
    .bind(input.email.as_deref().unwrap_or(current.email.as_str()))
    .bind(
        input
            .mature_content_allowed
            .unwrap_or(current.mature_content_allowed) as i64,
    )
    .bind(
        input
            .default_audio
            .as_deref()
            .unwrap_or(current.default_audio.as_str()),
    )
    .bind(
        input
            .subtitle_preset
            .as_deref()
            .unwrap_or(current.subtitle_preset.as_str()),
    )
    .bind(input.autoplay_trailers.unwrap_or(current.autoplay_trailers) as i64)
    .bind(
        input
            .live_chat_filter
            .as_deref()
            .unwrap_or(current.live_chat_filter.as_str()),
    )
    .bind(&identity.user_id)
    .execute(&state.pool)
    .await?;

    Ok(Json(
        fetch_user_profile_details(&state.pool, &identity.user_id).await?,
    ))
}

pub(crate) async fn get_my_settings(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<UserSettingsBundle>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_user_settings_bundle(&state.pool, &identity.user_id).await?,
    ))
}

pub(crate) async fn update_my_settings(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<UpdateSettingsRequest>,
) -> AppResult<Json<UserSettingsBundle>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let current = fetch_user_settings_bundle(&state.pool, &identity.user_id).await?;
    validate_settings_update(&input)?;

    if let Some(playback) = input.playback {
        sqlx::query(
            r#"
            UPDATE user_playback_settings
            SET default_quality = ?, audio_language = ?, subtitle_language = ?, subtitle_style = ?,
                autoplay_next_episode = ?, autoplay_trailers = ?, reduced_motion = ?,
                prefer_dubbed = ?, playback_speed = ?
            WHERE user_id = ?
            "#,
        )
        .bind(playback.default_quality)
        .bind(playback.audio_language)
        .bind(playback.subtitle_language)
        .bind(playback.subtitle_style)
        .bind(playback.autoplay_next_episode as i64)
        .bind(playback.autoplay_trailers as i64)
        .bind(playback.reduced_motion as i64)
        .bind(playback.prefer_dubbed as i64)
        .bind(playback.playback_speed)
        .bind(&identity.user_id)
        .execute(&state.pool)
        .await?;
    }

    if let Some(notifications) = input.notifications {
        sqlx::query(
            r#"
            UPDATE user_notification_settings
            SET series_push = ?, series_email = ?, live_push = ?, live_email = ?, originals_push = ?,
                originals_email = ?, watchlist_push = ?, watchlist_email = ?, creator_push = ?,
                creator_email = ?, security_push = ?, security_email = ?
            WHERE user_id = ?
            "#,
        )
        .bind(notifications.series_releases.push as i64)
        .bind(notifications.series_releases.email as i64)
        .bind(notifications.live_streams.push as i64)
        .bind(notifications.live_streams.email as i64)
        .bind(notifications.originals.push as i64)
        .bind(notifications.originals.email as i64)
        .bind(notifications.watchlist_updates.push as i64)
        .bind(notifications.watchlist_updates.email as i64)
        .bind(notifications.creator_updates.push as i64)
        .bind(notifications.creator_updates.email as i64)
        .bind(notifications.security_alerts.push as i64)
        .bind(notifications.security_alerts.email as i64)
        .bind(&identity.user_id)
        .execute(&state.pool)
        .await?;
    }

    if let Some(privacy) = input.privacy {
        sqlx::query(
            r#"
            UPDATE user_privacy_settings
            SET show_friend_activity = ?, improve_recommendations = ?, personalized_ads = ?,
                ab_tests = ?, data_export_size_mb = ?, delete_cooldown_days = ?
            WHERE user_id = ?
            "#,
        )
        .bind(privacy.show_friend_activity as i64)
        .bind(privacy.improve_recommendations as i64)
        .bind(privacy.personalized_ads as i64)
        .bind(privacy.ab_tests as i64)
        .bind(privacy.data_export_size_mb)
        .bind(privacy.delete_cooldown_days)
        .bind(&identity.user_id)
        .execute(&state.pool)
        .await?;
    }

    if let Some(parental) = input.parental {
        sqlx::query(
            r#"
            UPDATE user_parental_controls
            SET max_rating = ?, require_pin_for_mature = ?, hide_live_chat_for_kids = ?,
                block_mature_live_streams = ?, pin_set = ?
            WHERE user_id = ?
            "#,
        )
        .bind(parental.max_rating)
        .bind(parental.require_pin_for_mature as i64)
        .bind(parental.hide_live_chat_for_kids as i64)
        .bind(parental.block_mature_live_streams as i64)
        .bind(parental.pin_set as i64)
        .bind(&identity.user_id)
        .execute(&state.pool)
        .await?;
    }

    if let Some(downloads) = input.downloads {
        sqlx::query(
            r#"
            UPDATE user_download_settings
            SET video_quality = ?, wifi_only = ?, smart_downloads = ?, storage_used_gb = ?,
                storage_limit_gb = ?, device_limit = ?, active_devices = ?
            WHERE user_id = ?
            "#,
        )
        .bind(downloads.video_quality)
        .bind(downloads.wifi_only as i64)
        .bind(downloads.smart_downloads as i64)
        .bind(downloads.storage_used_gb)
        .bind(downloads.storage_limit_gb)
        .bind(downloads.device_limit)
        .bind(downloads.active_devices)
        .bind(&identity.user_id)
        .execute(&state.pool)
        .await?;
    }

    if let Some(language) = input.language {
        sqlx::query(
            r#"
            UPDATE user_language_settings
            SET interface_language = ?, subtitle_language = ?, catalog_region = ?,
                date_format = ?, clock_format = ?
            WHERE user_id = ?
            "#,
        )
        .bind(language.interface_language)
        .bind(language.subtitle_language)
        .bind(language.catalog_region)
        .bind(language.date_format)
        .bind(language.clock_format)
        .bind(&identity.user_id)
        .execute(&state.pool)
        .await?;
    }

    let _ = current;
    Ok(Json(
        fetch_user_settings_bundle(&state.pool, &identity.user_id).await?,
    ))
}
