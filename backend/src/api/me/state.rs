use super::*;

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
    let sessions = database
        .list_auth_sessions(user_id, session_id, Some(8))
        .await?;
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
    let watchlist = WatchlistResponse {
        total_titles: user.watchlist.len() as i64,
        series: Vec::new(),
        films: Vec::new(),
    };
    let following = FollowingFeedResponse {
        total_followed_streamers: user.following.len() as i64,
        live_now_count: 0,
        followed_streamers: Vec::new(),
        live_streams: Vec::new(),
    };
    let profile = UserProfileDetails {
        user: user.clone(),
        email: String::new(),
        email_verified: false,
        mature_content_allowed: false,
        default_audio: "English".to_string(),
        subtitle_preset: "Off".to_string(),
        autoplay_trailers: false,
        live_chat_filter: "Standard".to_string(),
        hours_watched: 0,
        connected_accounts: Vec::new(),
    };
    let settings = default_user_settings_bundle();
    let plan = BillingPlan {
        plan_name: "Free".to_string(),
        monthly_price: 0.0,
        next_renewal_date: String::new(),
        payment_brand: String::new(),
        payment_last4: String::new(),
        billing_city: String::new(),
        billing_region: String::new(),
        billing_country: String::new(),
        invoices_count: 0,
        screens: 1,
        features: Vec::new(),
        average_revenue_per_user: 0.0,
    };

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

fn default_user_settings_bundle() -> UserSettingsBundle {
    UserSettingsBundle {
        playback: PlaybackSettings {
            default_quality: "Auto".to_string(),
            audio_language: "English".to_string(),
            subtitle_language: "Off".to_string(),
            subtitle_style: "Default".to_string(),
            autoplay_next_episode: true,
            autoplay_trailers: false,
            reduced_motion: false,
            prefer_dubbed: false,
            playback_speed: "1x".to_string(),
        },
        notifications: NotificationSettings {
            series_releases: notification_channel("Series releases", false),
            live_streams: notification_channel("Live streams", false),
            originals: notification_channel("Originals", false),
            watchlist_updates: notification_channel("Watchlist updates", false),
            creator_updates: notification_channel("Creator updates", false),
            security_alerts: notification_channel("Security alerts", true),
        },
        privacy: PrivacySettings {
            show_friend_activity: false,
            improve_recommendations: false,
            personalized_ads: false,
            ab_tests: false,
            data_export_size_mb: 0.0,
            delete_cooldown_days: 0,
        },
        parental: ParentalControls {
            max_rating: "TV-MA".to_string(),
            require_pin_for_mature: false,
            hide_live_chat_for_kids: false,
            block_mature_live_streams: false,
            pin_set: false,
        },
        downloads: DownloadSettings {
            video_quality: "Auto".to_string(),
            wifi_only: true,
            smart_downloads: false,
            storage_used_gb: 0.0,
            storage_limit_gb: 0.0,
            device_limit: 0,
            active_devices: 0,
        },
        language: LanguageSettings {
            interface_language: "English".to_string(),
            subtitle_language: "Off".to_string(),
            catalog_region: "US".to_string(),
            date_format: "MM/DD/YYYY".to_string(),
            clock_format: "12h".to_string(),
        },
    }
}

fn notification_channel(label: &str, lock: bool) -> NotificationChannelSetting {
    NotificationChannelSetting {
        label: label.to_string(),
        push: false,
        email: false,
        lock,
    }
}
