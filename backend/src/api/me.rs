use super::*;

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/api/v1/me", get(get_me))
        .route("/api/v1/me/state", get(get_my_state))
        .route("/api/v1/me/library", get(get_my_library))
        .route("/api/v1/me/entitlements", get(get_my_entitlements))
        .route(
            "/api/v1/me/entitlements/memberships/:creator_id",
            get(get_my_membership_entitlement),
        )
        .route(
            "/api/v1/me/entitlements/memberships/:creator_id/reconcile",
            post(reconcile_my_membership_entitlement),
        )
        .route(
            "/api/v1/me/entitlements/purchases/:purchase_id",
            get(get_my_purchase_entitlement),
        )
        .route(
            "/api/v1/me/entitlements/purchases/:purchase_id/reconcile",
            post(reconcile_my_purchase_entitlement),
        )
        .route("/api/v1/me/watchlist", get(get_my_watchlist))
        .route("/api/v1/me/notifications", get(list_my_notifications))
        .route(
            "/api/v1/me/notifications/:notification_id/read",
            post(mark_my_notification_read),
        )
        .route(
            "/api/v1/me/profile",
            get(get_my_profile).patch(update_my_profile),
        )
        .route(
            "/api/v1/me/settings",
            get(get_my_settings).patch(update_my_settings),
        )
        .route("/api/v1/me/plan", get(get_my_plan))
        .route(
            "/api/v1/me/sessions",
            get(list_sessions).post(create_session),
        )
        .route("/api/v1/me/sessions/:id", delete(revoke_session))
        .route(
            "/api/v1/me/watchlist/:content_id",
            post(add_watchlist).delete(remove_watchlist),
        )
        .route(
            "/api/v1/me/following/:streamer_id",
            post(add_following).delete(remove_following),
        )
        .route("/api/v1/me/following", get(get_my_following_feed))
        .route("/api/v1/me/progress", put(record_progress))
        .route("/api/v1/me/progress/:content_id", delete(remove_progress))
        .route(
            "/api/v1/me/history/:content_id",
            delete(remove_history_entry),
        )
}

pub(super) async fn get_me(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(fetch_user(&state.pool, &identity.user_id).await?))
}

async fn get_my_state(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<ViewerAppState>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_viewer_app_state(&state.pool, &identity.user_id, &identity.session_id).await?,
    ))
}

async fn get_my_library(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<UserLibrary>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_user_library(&state.pool, &identity.user_id).await?,
    ))
}

async fn get_my_watchlist(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<WatchlistResponse>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_watchlist_response(&state.pool, &identity.user_id).await?,
    ))
}

async fn get_my_following_feed(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<FollowingFeedResponse>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let followed_streamer_ids = fetch_followed_streamer_ids(&state.pool, &identity.user_id).await?;
    let mut followed_streamers = Vec::with_capacity(followed_streamer_ids.len());
    for streamer_id in &followed_streamer_ids {
        followed_streamers.push(fetch_streamer_by_id(&state.pool, streamer_id).await?);
    }

    let followed_streamer_id_set: std::collections::HashSet<_> =
        followed_streamer_ids.into_iter().collect();
    let live_streams: Vec<LiveStream> = fetch_live_streams(&state.pool, None)
        .await?
        .into_iter()
        .filter(|stream| followed_streamer_id_set.contains(&stream.streamer.id))
        .collect();

    Ok(Json(FollowingFeedResponse {
        total_followed_streamers: followed_streamers.len() as i64,
        live_now_count: live_streams.len() as i64,
        followed_streamers,
        live_streams,
    }))
}

async fn get_my_entitlements(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<UserEntitlements>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_user_entitlements(&state.pool, &identity.user_id).await?,
    ))
}

pub(super) async fn get_my_membership_entitlement(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(creator_id): Path<String>,
) -> AppResult<Json<CreatorMembership>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_creator_membership(&state.pool, &identity.user_id, &creator_id).await?,
    ))
}

pub(super) async fn reconcile_my_membership_entitlement(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(creator_id): Path<String>,
) -> AppResult<Json<CreatorMembershipReconciliationReport>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let membership = fetch_creator_membership(&state.pool, &identity.user_id, &creator_id).await?;
    if membership.creator_id != creator_id {
        return Err(AppError::NotFound);
    }
    Ok(Json(
        reconcile_single_membership_entitlement(state, &identity.user_id, &creator_id).await?,
    ))
}

pub(super) async fn get_my_purchase_entitlement(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(purchase_id): Path<String>,
) -> AppResult<Json<ContentPurchase>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let purchase = fetch_content_purchase_by_id(&state.pool, &purchase_id).await?;
    if purchase_belongs_to_user(&state.pool, &identity.user_id, &purchase.id).await? {
        return Ok(Json(purchase));
    }
    Err(AppError::NotFound)
}

pub(super) async fn reconcile_my_purchase_entitlement(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(purchase_id): Path<String>,
) -> AppResult<Json<ContentPurchaseReconciliationReport>> {
    let identity = require_identity(&state.pool, &headers).await?;
    if !purchase_belongs_to_user(&state.pool, &identity.user_id, &purchase_id).await? {
        return Err(AppError::NotFound);
    }
    Ok(Json(
        reconcile_single_purchase_entitlement(state, &identity.user_id, &purchase_id).await?,
    ))
}

async fn get_my_profile(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<UserProfileDetails>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_user_profile_details(&state.pool, &identity.user_id).await?,
    ))
}

async fn update_my_profile(
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

async fn get_my_settings(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<UserSettingsBundle>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_user_settings_bundle(&state.pool, &identity.user_id).await?,
    ))
}

async fn update_my_settings(
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

async fn get_my_plan(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<BillingPlan>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_billing_plan(&state.pool, &identity.user_id).await?,
    ))
}

async fn list_sessions(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<AuthSession>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_auth_sessions(&state.pool, &identity.user_id, &identity.session_id).await?,
    ))
}

async fn create_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<CreateSessionRequest>,
) -> AppResult<Json<SessionTokenResponse>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let label = input.label.trim();
    if label.is_empty() {
        return Err(AppError::BadRequest("label is required".to_string()));
    }
    if label.len() > 64 {
        return Err(AppError::BadRequest(
            "label must be 64 characters or fewer".to_string(),
        ));
    }

    let scopes = input.scopes.unwrap_or_else(|| identity.scopes.clone());
    if scopes.is_empty() {
        return Err(AppError::BadRequest(
            "session must contain at least one scope".to_string(),
        ));
    }
    if scopes.iter().any(|scope| !identity.scopes.contains(scope)) {
        return Err(AppError::Forbidden);
    }

    let expires_at = match input.expires_in_days {
        Some(days) if !(1..=365).contains(&days) => {
            return Err(AppError::BadRequest(
                "expiresInDays must be between 1 and 365".to_string(),
            ));
        }
        Some(days) => Some((Utc::now() + chrono::Duration::days(days)).to_rfc3339()),
        None => None,
    };

    let session_id = Uuid::new_v4().to_string();
    let access_token = format!(
        "lst_{}_{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    let created_at = Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO auth_sessions (
            id, user_id, label, token_hash, scopes_json, created_at, expires_at, revoked_at, last_used_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, NULL, NULL)
        "#,
    )
    .bind(&session_id)
    .bind(&identity.user_id)
    .bind(label)
    .bind(crate::auth::hash_token(&access_token))
    .bind(to_json(&scopes)?)
    .bind(&created_at)
    .bind(&expires_at)
    .execute(&state.pool)
    .await?;

    Ok(Json(SessionTokenResponse {
        session: AuthSession {
            id: session_id,
            label: label.to_string(),
            scopes,
            created_at,
            expires_at,
            revoked_at: None,
            last_used_at: None,
            is_current: false,
        },
        access_token,
    }))
}

pub(super) async fn revoke_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let identity = require_identity(&state.pool, &headers).await?;
    let revoked_at = Utc::now().to_rfc3339();
    let result = sqlx::query(
        "UPDATE auth_sessions SET revoked_at = ? WHERE id = ? AND user_id = ? AND revoked_at IS NULL",
    )
    .bind(&revoked_at)
    .bind(&id)
    .bind(&identity.user_id)
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    expire_playback_sessions_for_auth_session(&state.pool, &id).await?;

    state
        .realtime
        .publish(
            &auth_session_channel_id(&id),
            WsEvent::SessionInvalidated {
                reason: "auth session revoked".to_string(),
            },
        )
        .await;
    Ok(StatusCode::NO_CONTENT)
}

async fn add_watchlist(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(content_id): Path<String>,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.pool, &headers).await?;
    validate_watchlist_content(&state.pool, &content_id).await?;
    sqlx::query("INSERT OR IGNORE INTO user_watchlist (user_id, content_id) VALUES (?, ?)")
        .bind(&identity.user_id)
        .bind(content_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(fetch_user(&state.pool, &identity.user_id).await?))
}

async fn remove_watchlist(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(content_id): Path<String>,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.pool, &headers).await?;
    sqlx::query("DELETE FROM user_watchlist WHERE user_id = ? AND content_id = ?")
        .bind(&identity.user_id)
        .bind(content_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(fetch_user(&state.pool, &identity.user_id).await?))
}

async fn add_following(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(streamer_id): Path<String>,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.pool, &headers).await?;
    fetch_streamer_by_id(&state.pool, &streamer_id).await?;
    sqlx::query("INSERT OR IGNORE INTO user_following (user_id, streamer_id) VALUES (?, ?)")
        .bind(&identity.user_id)
        .bind(streamer_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(fetch_user(&state.pool, &identity.user_id).await?))
}

async fn remove_following(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(streamer_id): Path<String>,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.pool, &headers).await?;
    sqlx::query("DELETE FROM user_following WHERE user_id = ? AND streamer_id = ?")
        .bind(&identity.user_id)
        .bind(streamer_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(fetch_user(&state.pool, &identity.user_id).await?))
}

async fn record_progress(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<ProgressInput>,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.pool, &headers).await?;
    if input.progress_sec < 0 {
        return Err(AppError::BadRequest("progressSec must be >= 0".to_string()));
    }
    let progress_target = resolve_progress_target(&state.pool, &input).await?;
    let canonical_duration_sec = progress_target.duration_sec;
    let normalized_progress_sec = input.progress_sec.min(canonical_duration_sec);
    let watched_at = Utc::now().to_rfc3339();
    let progress_kind = progress_target.kind.clone();
    let progress_episode_id = progress_target.episode_id.clone();

    if normalized_progress_sec >= canonical_duration_sec {
        sqlx::query("DELETE FROM continue_watching WHERE user_id = ? AND content_id = ?")
            .bind(&identity.user_id)
            .bind(&input.content_id)
            .execute(&state.pool)
            .await?;
        upsert_watch_history_entry(
            &state.pool,
            &identity.user_id,
            &input.content_id,
            &progress_target.kind,
            progress_target.episode_id.as_deref(),
            canonical_duration_sec,
            canonical_duration_sec,
            true,
            &watched_at,
        )
        .await?;
        return Ok(Json(fetch_user(&state.pool, &identity.user_id).await?));
    }

    sqlx::query(
        r#"
        INSERT INTO continue_watching (user_id, content_id, kind, episode_id, progress_sec, duration_sec, last_watched_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(user_id, content_id) DO UPDATE SET
            kind = excluded.kind,
            episode_id = excluded.episode_id,
            progress_sec = excluded.progress_sec,
            duration_sec = excluded.duration_sec,
            last_watched_at = excluded.last_watched_at
        "#,
    )
    .bind(&identity.user_id)
    .bind(&input.content_id)
    .bind(&progress_kind)
    .bind(&progress_episode_id)
    .bind(normalized_progress_sec)
    .bind(canonical_duration_sec)
    .bind(&watched_at)
    .execute(&state.pool)
    .await?;
    upsert_watch_history_entry(
        &state.pool,
        &identity.user_id,
        &input.content_id,
        &progress_kind,
        progress_episode_id.as_deref(),
        normalized_progress_sec,
        canonical_duration_sec,
        false,
        &watched_at,
    )
    .await?;

    Ok(Json(fetch_user(&state.pool, &identity.user_id).await?))
}

async fn remove_progress(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(content_id): Path<String>,
) -> AppResult<Json<User>> {
    let identity = require_identity(&state.pool, &headers).await?;
    sqlx::query("DELETE FROM continue_watching WHERE user_id = ? AND content_id = ?")
        .bind(&identity.user_id)
        .bind(content_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(fetch_user(&state.pool, &identity.user_id).await?))
}

async fn remove_history_entry(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(content_id): Path<String>,
) -> AppResult<Json<UserLibrary>> {
    let identity = require_identity(&state.pool, &headers).await?;
    sqlx::query("DELETE FROM user_watch_history WHERE user_id = ? AND content_id = ?")
        .bind(&identity.user_id)
        .bind(content_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(
        fetch_user_library(&state.pool, &identity.user_id).await?,
    ))
}
