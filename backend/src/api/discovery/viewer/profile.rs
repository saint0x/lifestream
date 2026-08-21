use super::*;

#[derive(Clone)]
pub(crate) struct UserProfileRow {
    pub(crate) email: String,
    pub(crate) email_verified: bool,
    pub(crate) mature_content_allowed: bool,
    pub(crate) default_audio: String,
    pub(crate) subtitle_preset: String,
    pub(crate) autoplay_trailers: bool,
    pub(crate) live_chat_filter: String,
    pub(crate) hours_watched: i64,
}

#[derive(Clone)]
pub(crate) struct ViewerAccountBundle {
    pub(crate) profile: UserProfileRow,
    pub(crate) playback: PlaybackSettings,
    pub(crate) notifications: NotificationSettings,
    pub(crate) privacy: PrivacySettings,
    pub(crate) parental: ParentalControls,
    pub(crate) downloads: DownloadSettings,
    pub(crate) language: LanguageSettings,
    pub(crate) plan: BillingPlan,
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
    fetch_user_profile_details_with_user(pool, user).await
}

pub(crate) async fn fetch_user_profile_details_with_user(
    pool: &SqlitePool,
    user: User,
) -> AppResult<UserProfileDetails> {
    let user_id = user.id.clone();
    let (bundle, connected_accounts) = tokio::try_join!(
        fetch_viewer_account_bundle(pool, &user_id),
        fetch_connected_accounts(pool, &user_id),
    )?;
    Ok(user_profile_details_from_bundle(user, bundle.profile, connected_accounts))
}

pub(crate) async fn fetch_viewer_account_bundle(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<ViewerAccountBundle> {
    let row = sqlx::query(
        r#"
        SELECT
            up.email, up.email_verified, up.mature_content_allowed, up.default_audio,
            up.subtitle_preset, up.autoplay_trailers, up.live_chat_filter, up.hours_watched,
            ups.default_quality, ups.audio_language, ups.subtitle_language, ups.subtitle_style,
            ups.autoplay_next_episode, ups.autoplay_trailers AS playback_autoplay_trailers,
            ups.reduced_motion, ups.prefer_dubbed, ups.playback_speed,
            uns.series_push, uns.series_email, uns.live_push, uns.live_email,
            uns.originals_push, uns.originals_email, uns.watchlist_push, uns.watchlist_email,
            uns.creator_push, uns.creator_email, uns.security_push, uns.security_email,
            upr.show_friend_activity, upr.improve_recommendations, upr.personalized_ads,
            upr.ab_tests, upr.data_export_size_mb, upr.delete_cooldown_days,
            upc.max_rating, upc.require_pin_for_mature, upc.hide_live_chat_for_kids,
            upc.block_mature_live_streams, upc.pin_set,
            uds.video_quality, uds.wifi_only, uds.smart_downloads, uds.storage_used_gb,
            uds.storage_limit_gb, uds.device_limit, uds.active_devices,
            uls.interface_language, uls.subtitle_language AS ui_subtitle_language,
            uls.catalog_region, uls.date_format, uls.clock_format,
            bp.plan_name, bp.monthly_price, bp.next_renewal_date, bp.payment_brand, bp.payment_last4,
            bp.billing_city, bp.billing_region, bp.billing_country, bp.invoices_count, bp.screens,
            bp.features_json, bp.average_revenue_per_user
        FROM user_profiles up
        JOIN user_playback_settings ups ON ups.user_id = up.user_id
        JOIN user_notification_settings uns ON uns.user_id = up.user_id
        JOIN user_privacy_settings upr ON upr.user_id = up.user_id
        JOIN user_parental_controls upc ON upc.user_id = up.user_id
        JOIN user_download_settings uds ON uds.user_id = up.user_id
        JOIN user_language_settings uls ON uls.user_id = up.user_id
        JOIN billing_profiles bp ON bp.user_id = up.user_id
        WHERE up.user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(ViewerAccountBundle {
        profile: UserProfileRow {
            email: row.get("email"),
            email_verified: row.get::<i64, _>("email_verified") == 1,
            mature_content_allowed: row.get::<i64, _>("mature_content_allowed") == 1,
            default_audio: row.get("default_audio"),
            subtitle_preset: row.get("subtitle_preset"),
            autoplay_trailers: row.get::<i64, _>("autoplay_trailers") == 1,
            live_chat_filter: row.get("live_chat_filter"),
            hours_watched: row.get("hours_watched"),
        },
        playback: PlaybackSettings {
            default_quality: row.get("default_quality"),
            audio_language: row.get("audio_language"),
            subtitle_language: row.get("subtitle_language"),
            subtitle_style: row.get("subtitle_style"),
            autoplay_next_episode: row.get::<i64, _>("autoplay_next_episode") == 1,
            autoplay_trailers: row.get::<i64, _>("playback_autoplay_trailers") == 1,
            reduced_motion: row.get::<i64, _>("reduced_motion") == 1,
            prefer_dubbed: row.get::<i64, _>("prefer_dubbed") == 1,
            playback_speed: row.get("playback_speed"),
        },
        notifications: NotificationSettings {
            series_releases: NotificationChannelSetting {
                label: "New episodes of series I watch".to_string(),
                push: row.get::<i64, _>("series_push") == 1,
                email: row.get::<i64, _>("series_email") == 1,
                lock: false,
            },
            live_streams: NotificationChannelSetting {
                label: "Followed streamers go live".to_string(),
                push: row.get::<i64, _>("live_push") == 1,
                email: row.get::<i64, _>("live_email") == 1,
                lock: false,
            },
            originals: NotificationChannelSetting {
                label: "LIFESTREAM Originals premieres".to_string(),
                push: row.get::<i64, _>("originals_push") == 1,
                email: row.get::<i64, _>("originals_email") == 1,
                lock: false,
            },
            watchlist_updates: NotificationChannelSetting {
                label: "Watchlist price drops".to_string(),
                push: row.get::<i64, _>("watchlist_push") == 1,
                email: row.get::<i64, _>("watchlist_email") == 1,
                lock: false,
            },
            creator_updates: NotificationChannelSetting {
                label: "Creator tools & product updates".to_string(),
                push: row.get::<i64, _>("creator_push") == 1,
                email: row.get::<i64, _>("creator_email") == 1,
                lock: false,
            },
            security_alerts: NotificationChannelSetting {
                label: "Security alerts".to_string(),
                push: row.get::<i64, _>("security_push") == 1,
                email: row.get::<i64, _>("security_email") == 1,
                lock: true,
            },
        },
        privacy: PrivacySettings {
            show_friend_activity: row.get::<i64, _>("show_friend_activity") == 1,
            improve_recommendations: row.get::<i64, _>("improve_recommendations") == 1,
            personalized_ads: row.get::<i64, _>("personalized_ads") == 1,
            ab_tests: row.get::<i64, _>("ab_tests") == 1,
            data_export_size_mb: row.get("data_export_size_mb"),
            delete_cooldown_days: row.get("delete_cooldown_days"),
        },
        parental: ParentalControls {
            max_rating: row.get("max_rating"),
            require_pin_for_mature: row.get::<i64, _>("require_pin_for_mature") == 1,
            hide_live_chat_for_kids: row.get::<i64, _>("hide_live_chat_for_kids") == 1,
            block_mature_live_streams: row.get::<i64, _>("block_mature_live_streams") == 1,
            pin_set: row.get::<i64, _>("pin_set") == 1,
        },
        downloads: DownloadSettings {
            video_quality: row.get("video_quality"),
            wifi_only: row.get::<i64, _>("wifi_only") == 1,
            smart_downloads: row.get::<i64, _>("smart_downloads") == 1,
            storage_used_gb: row.get("storage_used_gb"),
            storage_limit_gb: row.get("storage_limit_gb"),
            device_limit: row.get("device_limit"),
            active_devices: row.get("active_devices"),
        },
        language: LanguageSettings {
            interface_language: row.get("interface_language"),
            subtitle_language: row.get("ui_subtitle_language"),
            catalog_region: row.get("catalog_region"),
            date_format: row.get("date_format"),
            clock_format: row.get("clock_format"),
        },
        plan: BillingPlan {
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
        },
    })
}

pub(crate) async fn fetch_user_settings_bundle(
    pool: &SqlitePool,
    user_id: &str,
) -> AppResult<UserSettingsBundle> {
    Ok(user_settings_bundle_from_account_bundle(
        fetch_viewer_account_bundle(pool, user_id).await?,
    ))
}

pub(crate) async fn fetch_billing_plan(pool: &SqlitePool, user_id: &str) -> AppResult<BillingPlan> {
    Ok(fetch_viewer_account_bundle(pool, user_id).await?.plan)
}

pub(crate) fn user_profile_details_from_bundle(
    user: User,
    profile: UserProfileRow,
    connected_accounts: Vec<ConnectedAccount>,
) -> UserProfileDetails {
    UserProfileDetails {
        user,
        email: profile.email,
        email_verified: profile.email_verified,
        mature_content_allowed: profile.mature_content_allowed,
        default_audio: profile.default_audio,
        subtitle_preset: profile.subtitle_preset,
        autoplay_trailers: profile.autoplay_trailers,
        live_chat_filter: profile.live_chat_filter,
        hours_watched: profile.hours_watched,
        connected_accounts,
    }
}

pub(crate) fn user_settings_bundle_from_account_bundle(
    bundle: ViewerAccountBundle,
) -> UserSettingsBundle {
    UserSettingsBundle {
        playback: bundle.playback,
        notifications: bundle.notifications,
        privacy: bundle.privacy,
        parental: bundle.parental,
        downloads: bundle.downloads,
        language: bundle.language,
    }
}
