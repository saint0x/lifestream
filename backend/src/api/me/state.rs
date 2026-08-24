use super::*;
use crate::api::public::{
    postgres_fetch_film_by_id, postgres_fetch_live_streams, postgres_fetch_series_by_id,
    postgres_fetch_streamer_by_id,
};
use sqlx::Row;

const VIEWER_APP_STATE_RESPONSE_CACHE_TTL: Duration = Duration::from_millis(2_000);

pub(crate) async fn get_me(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.db, &headers).await?;
    Ok(Json(state.db.fetch_user(&identity.user_id).await?))
}

pub(crate) async fn get_my_state(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let identity = require_identity(&state.db, &headers).await?;
    if state.database_kind == crate::config::DatabaseKind::Postgres {
        return Ok(Json(
            build_postgres_viewer_app_state(&state.db, &identity.user_id, &identity.session_id)
                .await?,
        )
        .into_response());
    }
    let cache_key = format!("viewer-state:session:{}", identity.session_id);
    if let Some(cached) = state
        .bootstrap_cache
        .get(&cache_key, VIEWER_APP_STATE_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok((
            [(header::CONTENT_TYPE, "application/json")],
            Body::from(cached),
        )
            .into_response());
    }
    let _coalesced = state.request_coalescer.acquire(&cache_key).await;
    if let Some(cached) = state
        .bootstrap_cache
        .get(&cache_key, VIEWER_APP_STATE_RESPONSE_CACHE_TTL)
        .await
    {
        return Ok((
            [(header::CONTENT_TYPE, "application/json")],
            Body::from(cached),
        )
            .into_response());
    }
    let response = fetch_viewer_app_state(
        &state.db,
        state.db.sqlite_adapter(),
        &identity.user_id,
        &identity.session_id,
    )
    .await?;
    let response_body = Bytes::from(serde_json::to_vec(&response)?);
    state
        .bootstrap_cache
        .put(&cache_key, response_body.clone())
        .await;
    Ok((
        [(header::CONTENT_TYPE, "application/json")],
        Body::from(response_body),
    )
        .into_response())
}

pub(crate) async fn get_my_library(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<UserLibrary>> {
    let identity = require_identity(&state.db, &headers).await?;
    if state.database_kind == crate::config::DatabaseKind::Postgres {
        return Ok(Json(
            build_postgres_viewer_app_state(&state.db, &identity.user_id, &identity.session_id)
                .await?
                .library,
        ));
    }
    Ok(Json(
        fetch_user_library(state.db.sqlite_adapter(), &identity.user_id).await?,
    ))
}

pub(crate) async fn get_my_watchlist(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<WatchlistResponse>> {
    let identity = require_identity(&state.db, &headers).await?;
    if state.database_kind == crate::config::DatabaseKind::Postgres {
        return Ok(Json(
            build_postgres_viewer_app_state(&state.db, &identity.user_id, &identity.session_id)
                .await?
                .watchlist,
        ));
    }
    Ok(Json(
        fetch_watchlist_response(state.db.sqlite_adapter(), &identity.user_id).await?,
    ))
}

pub(crate) async fn get_my_entitlements(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<UserEntitlements>> {
    let identity = require_identity(&state.db, &headers).await?;
    if state.database_kind == crate::config::DatabaseKind::Postgres {
        return Ok(Json(UserEntitlements {
            memberships: Vec::new(),
            purchases: Vec::new(),
        }));
    }
    Ok(Json(
        fetch_user_entitlements(state.db.sqlite_adapter(), &identity.user_id).await?,
    ))
}

pub(crate) async fn build_postgres_viewer_app_state(
    database: &crate::db::Database,
    user_id: &str,
    session_id: &str,
) -> AppResult<ViewerAppState> {
    let user = database.fetch_user(user_id).await?;
    let pool = database.try_postgres_adapter()?;
    let sessions = database
        .list_auth_sessions(user_id, session_id, Some(8))
        .await?;
    let (watchlist, following, account) = tokio::try_join!(
        fetch_postgres_watchlist_response(pool, &user.watchlist),
        fetch_postgres_following_feed_response(pool, &user.following),
        fetch_postgres_viewer_account_bundle(pool, user_id),
    )?;
    let connected_accounts = fetch_postgres_connected_accounts(pool, user_id).await?;
    let entitlements = UserEntitlements {
        memberships: Vec::new(),
        purchases: Vec::new(),
    };
    let library = UserLibrary {
        continue_watching: user.continue_watching.clone(),
        history: Vec::new(),
        memberships: entitlements.memberships.clone(),
        purchases: entitlements.purchases.clone(),
    };
    let profile = UserProfileDetails {
        user: user.clone(),
        email: account.profile.email.clone(),
        email_verified: account.profile.email_verified,
        mature_content_allowed: account.profile.mature_content_allowed,
        default_audio: account.profile.default_audio.clone(),
        subtitle_preset: account.profile.subtitle_preset.clone(),
        autoplay_trailers: account.profile.autoplay_trailers,
        live_chat_filter: account.profile.live_chat_filter.clone(),
        hours_watched: account.profile.hours_watched,
        connected_accounts,
    };
    let settings = account.settings;
    let plan = account.plan;

    Ok(ViewerAppState {
        user,
        library,
        watchlist,
        following,
        entitlements,
        profile,
        settings,
        plan,
        notifications: Vec::new(),
        sessions,
    })
}

struct PostgresUserProfileRow {
    email: String,
    email_verified: bool,
    mature_content_allowed: bool,
    default_audio: String,
    subtitle_preset: String,
    autoplay_trailers: bool,
    live_chat_filter: String,
    hours_watched: i64,
}

struct PostgresViewerAccountBundle {
    profile: PostgresUserProfileRow,
    settings: UserSettingsBundle,
    plan: BillingPlan,
}

async fn fetch_postgres_watchlist_response(
    pool: &sqlx::PgPool,
    ids: &[String],
) -> AppResult<WatchlistResponse> {
    let mut series = Vec::new();
    let mut films = Vec::new();
    for id in ids {
        if let Ok(item) = postgres_fetch_series_by_id(pool, id, None).await {
            series.push(item);
            continue;
        }
        if let Ok(item) = postgres_fetch_film_by_id(pool, id, None).await {
            films.push(item);
            continue;
        }
        return Err(AppError::NotFound);
    }
    Ok(WatchlistResponse {
        total_titles: (series.len() + films.len()) as i64,
        series,
        films,
    })
}

async fn fetch_postgres_following_feed_response(
    pool: &sqlx::PgPool,
    ids: &[String],
) -> AppResult<FollowingFeedResponse> {
    let mut followed_streamers = Vec::with_capacity(ids.len());
    for id in ids {
        followed_streamers.push(postgres_fetch_streamer_by_id(pool, id).await?);
    }
    let followed_ids = followed_streamers
        .iter()
        .map(|streamer| streamer.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let live_streams = postgres_fetch_live_streams(pool, None)
        .await?
        .into_iter()
        .filter(|stream| followed_ids.contains(stream.streamer.id.as_str()))
        .collect::<Vec<_>>();
    Ok(FollowingFeedResponse {
        total_followed_streamers: followed_streamers.len() as i64,
        live_now_count: live_streams.len() as i64,
        followed_streamers,
        live_streams,
    })
}

async fn fetch_postgres_connected_accounts(
    pool: &sqlx::PgPool,
    user_id: &str,
) -> AppResult<Vec<ConnectedAccount>> {
    Ok(sqlx::query(
        "SELECT id, provider, display_name, connected_at FROM connected_accounts WHERE user_id = $1 ORDER BY connected_at ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| ConnectedAccount {
        id: row.get("id"),
        provider: row.get("provider"),
        display_name: row.get("display_name"),
        connected_at: row.get("connected_at"),
    })
    .collect())
}

async fn fetch_postgres_viewer_account_bundle(
    pool: &sqlx::PgPool,
    user_id: &str,
) -> AppResult<PostgresViewerAccountBundle> {
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
        WHERE up.user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let settings = UserSettingsBundle {
        playback: PlaybackSettings {
            default_quality: row.get("default_quality"),
            audio_language: row.get("audio_language"),
            subtitle_language: row.get("subtitle_language"),
            subtitle_style: row.get("subtitle_style"),
            autoplay_next_episode: row.get("autoplay_next_episode"),
            autoplay_trailers: row.get("playback_autoplay_trailers"),
            reduced_motion: row.get("reduced_motion"),
            prefer_dubbed: row.get("prefer_dubbed"),
            playback_speed: row.get("playback_speed"),
        },
        notifications: NotificationSettings {
            series_releases: notification_channel_with_values(
                "New episodes of series I watch",
                row.get("series_push"),
                row.get("series_email"),
                false,
            ),
            live_streams: notification_channel_with_values(
                "Followed streamers go live",
                row.get("live_push"),
                row.get("live_email"),
                false,
            ),
            originals: notification_channel_with_values(
                "VANTA Originals premieres",
                row.get("originals_push"),
                row.get("originals_email"),
                false,
            ),
            watchlist_updates: notification_channel_with_values(
                "Watchlist price drops",
                row.get("watchlist_push"),
                row.get("watchlist_email"),
                false,
            ),
            creator_updates: notification_channel_with_values(
                "Creator tools & product updates",
                row.get("creator_push"),
                row.get("creator_email"),
                false,
            ),
            security_alerts: notification_channel_with_values(
                "Security alerts",
                row.get("security_push"),
                row.get("security_email"),
                true,
            ),
        },
        privacy: PrivacySettings {
            show_friend_activity: row.get("show_friend_activity"),
            improve_recommendations: row.get("improve_recommendations"),
            personalized_ads: row.get("personalized_ads"),
            ab_tests: row.get("ab_tests"),
            data_export_size_mb: row.get("data_export_size_mb"),
            delete_cooldown_days: row.get("delete_cooldown_days"),
        },
        parental: ParentalControls {
            max_rating: row.get("max_rating"),
            require_pin_for_mature: row.get("require_pin_for_mature"),
            hide_live_chat_for_kids: row.get("hide_live_chat_for_kids"),
            block_mature_live_streams: row.get("block_mature_live_streams"),
            pin_set: row.get("pin_set"),
        },
        downloads: DownloadSettings {
            video_quality: row.get("video_quality"),
            wifi_only: row.get("wifi_only"),
            smart_downloads: row.get("smart_downloads"),
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
    };
    Ok(PostgresViewerAccountBundle {
        profile: PostgresUserProfileRow {
            email: row.get("email"),
            email_verified: row.get("email_verified"),
            mature_content_allowed: row.get("mature_content_allowed"),
            default_audio: row.get("default_audio"),
            subtitle_preset: row.get("subtitle_preset"),
            autoplay_trailers: row.get("autoplay_trailers"),
            live_chat_filter: row.get("live_chat_filter"),
            hours_watched: row.get("hours_watched"),
        },
        settings,
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

fn notification_channel_with_values(
    label: &str,
    push: bool,
    email: bool,
    lock: bool,
) -> NotificationChannelSetting {
    NotificationChannelSetting {
        label: label.to_string(),
        push,
        email,
        lock,
    }
}
