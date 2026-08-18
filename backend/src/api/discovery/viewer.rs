use super::*;

pub(crate) async fn fetch_user(pool: &SqlitePool, user_id: &str) -> AppResult<User> {
    let row = sqlx::query(
        "SELECT id, handle, display_name, avatar, tier, joined_at FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let watchlist = sqlx::query("SELECT content_id FROM user_watchlist WHERE user_id = ?")
        .bind(user_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|item| item.get("content_id"))
        .collect();

    let following = sqlx::query("SELECT streamer_id FROM user_following WHERE user_id = ?")
        .bind(user_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|item| item.get("streamer_id"))
        .collect();

    let continue_watching = sqlx::query(
        "SELECT content_id, kind, episode_id, progress_sec, duration_sec, last_watched_at FROM continue_watching WHERE user_id = ? ORDER BY last_watched_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|item| ContinueWatchingEntry {
        content_id: item.get("content_id"),
        kind: item.get("kind"),
        episode_id: item.get("episode_id"),
        progress_sec: item.get("progress_sec"),
        duration_sec: item.get("duration_sec"),
        last_watched_at: item.get("last_watched_at"),
    })
    .collect();

    Ok(User {
        id: row.get("id"),
        handle: row.get("handle"),
        display_name: row.get("display_name"),
        avatar: row.get("avatar"),
        tier: row.get("tier"),
        joined_at: row.get("joined_at"),
        watchlist,
        following,
        continue_watching,
    })
}

pub(crate) async fn fetch_watch_history(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<Vec<WatchHistoryEntry>> {
    Ok(sqlx::query(
        r#"
        SELECT content_id, kind, episode_id, progress_sec, duration_sec,
               completed, completed_at, last_watched_at
        FROM user_watch_history
        WHERE user_id = ?
        ORDER BY last_watched_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|item| WatchHistoryEntry {
        content_id: item.get("content_id"),
        kind: item.get("kind"),
        episode_id: item.get("episode_id"),
        progress_sec: item.get("progress_sec"),
        duration_sec: item.get("duration_sec"),
        completed: item.get::<i64, _>("completed") == 1,
        completed_at: item.get("completed_at"),
        last_watched_at: item.get("last_watched_at"),
    })
    .collect())
}

pub(crate) async fn fetch_user_library(pool: &SqlitePool, user_id: &str) -> AppResult<UserLibrary> {
    let user = fetch_user(pool, user_id).await?;
    let entitlements = fetch_user_entitlements(pool, user_id).await?;
    Ok(UserLibrary {
        continue_watching: user.continue_watching,
        history: fetch_watch_history(pool, user_id).await?,
        memberships: entitlements.memberships,
        purchases: entitlements.purchases,
    })
}

pub(crate) async fn fetch_watchlist_response(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<WatchlistResponse> {
    let watchlist_ids: Vec<String> = sqlx::query(
        "SELECT content_id FROM user_watchlist WHERE user_id = ? ORDER BY content_id ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|item| item.get("content_id"))
    .collect();

    let mut series = Vec::new();
    let mut films = Vec::new();
    for content_id in watchlist_ids {
        if let Ok(item) = fetch_series_by_id(pool, &content_id, None).await {
            series.push(item);
            continue;
        }
        if let Ok(item) = fetch_film_by_id(pool, &content_id, None).await {
            films.push(item);
        }
    }

    Ok(WatchlistResponse {
        total_titles: (series.len() + films.len()) as i64,
        series,
        films,
    })
}

pub(crate) async fn fetch_viewer_app_state(
    pool: &SqlitePool,
    user_id: &str,
    current_session_id: &str,
) -> AppResult<ViewerAppState> {
    let user = fetch_user(pool, user_id).await?;
    let library = fetch_user_library(pool, user_id).await?;
    let watchlist = fetch_watchlist_response(pool, user_id).await?;

    let followed_streamer_ids = fetch_followed_streamer_ids(pool, user_id).await?;
    let mut followed_streamers = Vec::with_capacity(followed_streamer_ids.len());
    for streamer_id in &followed_streamer_ids {
        followed_streamers.push(fetch_streamer_by_id(pool, streamer_id).await?);
    }
    let followed_streamer_id_set: std::collections::HashSet<_> =
        followed_streamer_ids.into_iter().collect();
    let live_streams: Vec<LiveStream> = fetch_live_streams(pool, None)
        .await?
        .into_iter()
        .filter(|stream| followed_streamer_id_set.contains(&stream.streamer.id))
        .collect();
    let following = FollowingFeedResponse {
        total_followed_streamers: followed_streamers.len() as i64,
        live_now_count: live_streams.len() as i64,
        followed_streamers,
        live_streams,
    };

    Ok(ViewerAppState {
        user,
        library,
        watchlist,
        following,
        entitlements: fetch_user_entitlements(pool, user_id).await?,
        profile: fetch_user_profile_details(pool, user_id).await?,
        settings: fetch_user_settings_bundle(pool, user_id).await?,
        plan: fetch_billing_plan(pool, user_id).await?,
        notifications: fetch_user_notifications(pool, user_id).await?,
        sessions: fetch_auth_sessions(pool, user_id, current_session_id).await?,
    })
}

pub(crate) async fn fetch_followed_streamer_ids(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<Vec<String>> {
    Ok(
        sqlx::query(
            "SELECT streamer_id FROM user_following WHERE user_id = ? ORDER BY streamer_id",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|item| item.get("streamer_id"))
        .collect(),
    )
}

pub(crate) async fn fetch_creator_id_for_user(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<Option<String>> {
    let row = sqlx::query("SELECT id FROM creator_profiles WHERE user_id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| row.get("id")))
}

pub(crate) async fn fetch_connected_accounts(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<Vec<ConnectedAccount>> {
    let rows = sqlx::query(
        "SELECT id, provider, display_name, connected_at FROM connected_accounts WHERE user_id = ? ORDER BY connected_at ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| ConnectedAccount {
            id: row.get("id"),
            provider: row.get("provider"),
            display_name: row.get("display_name"),
            connected_at: row.get("connected_at"),
        })
        .collect())
}

pub(crate) async fn fetch_user_profile_details(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<UserProfileDetails> {
    let user = fetch_user(pool, user_id).await?;
    let row = sqlx::query(
        r#"
        SELECT email, email_verified, mature_content_allowed, default_audio,
               subtitle_preset, autoplay_trailers, live_chat_filter, hours_watched
        FROM user_profiles
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(UserProfileDetails {
        user,
        email: row.get("email"),
        email_verified: row.get::<i64, _>("email_verified") == 1,
        mature_content_allowed: row.get::<i64, _>("mature_content_allowed") == 1,
        default_audio: row.get("default_audio"),
        subtitle_preset: row.get("subtitle_preset"),
        autoplay_trailers: row.get::<i64, _>("autoplay_trailers") == 1,
        live_chat_filter: row.get("live_chat_filter"),
        hours_watched: row.get("hours_watched"),
        connected_accounts: fetch_connected_accounts(pool, user_id).await?,
    })
}

pub(crate) async fn fetch_user_settings_bundle(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<UserSettingsBundle> {
    let playback_row = sqlx::query(
        r#"
        SELECT default_quality, audio_language, subtitle_language, subtitle_style,
               autoplay_next_episode, autoplay_trailers, reduced_motion, prefer_dubbed, playback_speed
        FROM user_playback_settings
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let notification_row = sqlx::query(
        r#"
        SELECT series_push, series_email, live_push, live_email, originals_push, originals_email,
               watchlist_push, watchlist_email, creator_push, creator_email, security_push, security_email
        FROM user_notification_settings
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let privacy_row = sqlx::query(
        r#"
        SELECT show_friend_activity, improve_recommendations, personalized_ads,
               ab_tests, data_export_size_mb, delete_cooldown_days
        FROM user_privacy_settings
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let parental_row = sqlx::query(
        r#"
        SELECT max_rating, require_pin_for_mature, hide_live_chat_for_kids,
               block_mature_live_streams, pin_set
        FROM user_parental_controls
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let download_row = sqlx::query(
        r#"
        SELECT video_quality, wifi_only, smart_downloads, storage_used_gb,
               storage_limit_gb, device_limit, active_devices
        FROM user_download_settings
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let language_row = sqlx::query(
        r#"
        SELECT interface_language, subtitle_language, catalog_region, date_format, clock_format
        FROM user_language_settings
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(UserSettingsBundle {
        playback: PlaybackSettings {
            default_quality: playback_row.get("default_quality"),
            audio_language: playback_row.get("audio_language"),
            subtitle_language: playback_row.get("subtitle_language"),
            subtitle_style: playback_row.get("subtitle_style"),
            autoplay_next_episode: playback_row.get::<i64, _>("autoplay_next_episode") == 1,
            autoplay_trailers: playback_row.get::<i64, _>("autoplay_trailers") == 1,
            reduced_motion: playback_row.get::<i64, _>("reduced_motion") == 1,
            prefer_dubbed: playback_row.get::<i64, _>("prefer_dubbed") == 1,
            playback_speed: playback_row.get("playback_speed"),
        },
        notifications: NotificationSettings {
            series_releases: NotificationChannelSetting {
                label: "New episodes of series I watch".to_string(),
                push: notification_row.get::<i64, _>("series_push") == 1,
                email: notification_row.get::<i64, _>("series_email") == 1,
                lock: false,
            },
            live_streams: NotificationChannelSetting {
                label: "Followed streamers go live".to_string(),
                push: notification_row.get::<i64, _>("live_push") == 1,
                email: notification_row.get::<i64, _>("live_email") == 1,
                lock: false,
            },
            originals: NotificationChannelSetting {
                label: "LIFESTREAM Originals premieres".to_string(),
                push: notification_row.get::<i64, _>("originals_push") == 1,
                email: notification_row.get::<i64, _>("originals_email") == 1,
                lock: false,
            },
            watchlist_updates: NotificationChannelSetting {
                label: "Watchlist price drops".to_string(),
                push: notification_row.get::<i64, _>("watchlist_push") == 1,
                email: notification_row.get::<i64, _>("watchlist_email") == 1,
                lock: false,
            },
            creator_updates: NotificationChannelSetting {
                label: "Creator tools & product updates".to_string(),
                push: notification_row.get::<i64, _>("creator_push") == 1,
                email: notification_row.get::<i64, _>("creator_email") == 1,
                lock: false,
            },
            security_alerts: NotificationChannelSetting {
                label: "Security alerts".to_string(),
                push: notification_row.get::<i64, _>("security_push") == 1,
                email: notification_row.get::<i64, _>("security_email") == 1,
                lock: true,
            },
        },
        privacy: PrivacySettings {
            show_friend_activity: privacy_row.get::<i64, _>("show_friend_activity") == 1,
            improve_recommendations: privacy_row.get::<i64, _>("improve_recommendations") == 1,
            personalized_ads: privacy_row.get::<i64, _>("personalized_ads") == 1,
            ab_tests: privacy_row.get::<i64, _>("ab_tests") == 1,
            data_export_size_mb: privacy_row.get("data_export_size_mb"),
            delete_cooldown_days: privacy_row.get("delete_cooldown_days"),
        },
        parental: ParentalControls {
            max_rating: parental_row.get("max_rating"),
            require_pin_for_mature: parental_row.get::<i64, _>("require_pin_for_mature") == 1,
            hide_live_chat_for_kids: parental_row.get::<i64, _>("hide_live_chat_for_kids") == 1,
            block_mature_live_streams: parental_row.get::<i64, _>("block_mature_live_streams") == 1,
            pin_set: parental_row.get::<i64, _>("pin_set") == 1,
        },
        downloads: DownloadSettings {
            video_quality: download_row.get("video_quality"),
            wifi_only: download_row.get::<i64, _>("wifi_only") == 1,
            smart_downloads: download_row.get::<i64, _>("smart_downloads") == 1,
            storage_used_gb: download_row.get("storage_used_gb"),
            storage_limit_gb: download_row.get("storage_limit_gb"),
            device_limit: download_row.get("device_limit"),
            active_devices: download_row.get("active_devices"),
        },
        language: LanguageSettings {
            interface_language: language_row.get("interface_language"),
            subtitle_language: language_row.get("subtitle_language"),
            catalog_region: language_row.get("catalog_region"),
            date_format: language_row.get("date_format"),
            clock_format: language_row.get("clock_format"),
        },
    })
}

pub(crate) async fn fetch_billing_plan(pool: &SqlitePool, user_id: &str) -> AppResult<BillingPlan> {
    let row = sqlx::query(
        r#"
        SELECT plan_name, monthly_price, next_renewal_date, payment_brand, payment_last4,
               billing_city, billing_region, billing_country, invoices_count, screens,
               features_json, average_revenue_per_user
        FROM billing_profiles
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(BillingPlan {
        plan_name: row.get("plan_name"),
        monthly_price: row.get("monthly_price"),
        next_renewal_date: row.get("next_renewal_date"),
        payment_brand: row.get("payment_brand"),
        payment_last4: row.get("payment_last4"),
        billing_city: row.get("billing_city"),
        billing_region: row.get("billing_region"),
        billing_country: row.get("billing_country"),
        invoices_count: row.get("invoices_count"),
        screens: row.get("screens"),
        features: from_json(row.get::<String, _>("features_json"))?,
        average_revenue_per_user: row.get("average_revenue_per_user"),
    })
}
