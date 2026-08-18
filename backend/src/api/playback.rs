use super::*;

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/api/v1/admin/playback/sessions",
            get(list_admin_playback_sessions),
        )
        .route(
            "/api/v1/admin/playback/sessions/:session_id",
            get(get_admin_playback_session),
        )
        .route(
            "/api/v1/admin/playback/sessions/:session_id/reconcile",
            post(reconcile_admin_playback_session),
        )
        .route(
            "/api/v1/admin/playback/sessions/:session_id/revoke",
            post(revoke_admin_playback_session),
        )
        .route(
            "/api/v1/playback/uploads/:upload_id/session",
            post(create_upload_playback_session),
        )
        .route(
            "/api/v1/playback/content/:content_id/session",
            post(create_content_playback_session),
        )
        .route(
            "/api/v1/playback/live/:stream_id/session",
            post(create_live_playback_session),
        )
        .route(
            "/api/v1/uploads/:upload_id/purchase",
            post(purchase_upload_access),
        )
        .route(
            "/api/v1/content/:content_id/purchase",
            post(purchase_content_access),
        )
        .route(
            "/api/v1/playback/sessions/:session_id",
            get(get_playback_session),
        )
        .route(
            "/api/v1/playback/sessions/:session_id/refresh",
            post(refresh_playback_session),
        )
        .route(
            "/api/v1/playback/sessions/:session_id/manifest",
            get(get_playback_manifest),
        )
}

pub(super) async fn list_admin_playback_sessions(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<AdminPlaybackSessionQuery>,
) -> AppResult<Json<Vec<AdminPlaybackSessionRecord>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    Ok(Json(
        fetch_admin_playback_sessions(
            &state.pool,
            query.creator_id.as_deref(),
            query.content_id.as_deref(),
            query.state.as_deref(),
            query.limit.unwrap_or(100),
        )
        .await?,
    ))
}

pub(super) async fn get_admin_playback_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<AdminPlaybackSessionRecord>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    Ok(Json(
        fetch_admin_playback_session_record(&state.pool, &session_id).await?,
    ))
}

pub(super) async fn reconcile_admin_playback_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<PlaybackReconciliationReport>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    fetch_playback_session_record_by_id(&state.pool, &session_id).await?;
    Ok(Json(
        reconcile_single_playback_session(state, &session_id).await?,
    ))
}

pub(super) async fn revoke_admin_playback_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> AppResult<Json<AdminPlaybackSessionRecord>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    expire_playback_session_by_id(&state.pool, &session_id).await?;
    Ok(Json(
        fetch_admin_playback_session_record(&state.pool, &session_id).await?,
    ))
}

pub(super) async fn create_upload_playback_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(upload_id): Path<String>,
) -> AppResult<Json<PlaybackGrant>> {
    create_playback_session_for_content_id(state, headers, upload_id).await
}

pub(super) async fn create_content_playback_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(content_id): Path<String>,
) -> AppResult<Json<PlaybackGrant>> {
    create_playback_session_for_content_id(state, headers, content_id).await
}

pub(super) async fn create_live_playback_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
) -> AppResult<Json<PlaybackGrant>> {
    let maybe_identity = optional_identity(&state.pool, &headers).await?;
    let target = fetch_live_stream_playback_target(&state.pool, &stream_id).await?;
    let now = Utc::now();
    let session_id = format!("pbs-{}", Uuid::new_v4().simple());
    let playback_token = format!("pbt_{}", Uuid::new_v4().simple());
    let expires_at = (now + chrono::Duration::hours(6)).to_rfc3339();
    let now_rfc3339 = now.to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO playback_sessions (
            id, auth_session_id, user_id, creator_id, asset_id, content_id, content_kind, token_hash,
            access_scope, created_at, expires_at, last_used_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&session_id)
    .bind(
        maybe_identity
            .as_ref()
            .map(|identity| identity.session_id.clone()),
    )
    .bind(
        maybe_identity
            .as_ref()
            .map(|identity| identity.user_id.clone()),
    )
    .bind(Some(target.creator_id.clone()))
    .bind(&target.asset_id)
    .bind(&stream_id)
    .bind("live")
    .bind(hash_token(&playback_token))
    .bind("live")
    .bind(&now_rfc3339)
    .bind(&expires_at)
    .bind(&now_rfc3339)
    .execute(&state.pool)
    .await?;

    let session = fetch_playback_session_by_id(&state.pool, &session_id).await?;
    let manifest_url = format!(
        "/api/v1/playback/sessions/{}/manifest?playbackToken={}",
        session.id, playback_token
    );
    let poster_url = target
        .poster_relative_path
        .as_ref()
        .map(|path| format!("/api/v1/media/{path}?playbackToken={playback_token}"));
    let preferred_subtitle_language = fetch_user_subtitle_preference(
        &state.pool,
        maybe_identity
            .as_ref()
            .map(|identity| identity.user_id.as_str()),
    )
    .await?;
    let (preferred_audio_language, prefer_dubbed) = fetch_user_audio_preferences(
        &state.pool,
        maybe_identity
            .as_ref()
            .map(|identity| identity.user_id.as_str()),
    )
    .await?;
    let audio_tracks = build_media_audio_tracks(
        &target.asset.status,
        &target.asset.id,
        &target.asset.variants,
        target.asset.audio_codec.as_deref(),
        Some(&playback_token),
        preferred_audio_language.as_deref(),
        prefer_dubbed,
    );
    let caption_tracks = build_media_caption_tracks(
        &target.asset.status,
        &target.asset.variants,
        Some(&playback_token),
        preferred_subtitle_language.as_deref(),
    );
    let preview_track_rows = fetch_media_preview_track_rows(&state.pool, &target.asset.id).await?;
    let preview_tracks = build_media_preview_tracks(
        &target.asset.status,
        &preview_track_rows,
        Some(&playback_token),
    );

    Ok(Json(PlaybackGrant {
        session,
        playback_token,
        manifest_url,
        poster_url,
        content_title: target.title,
        content_kind: "live".to_string(),
        visibility: "public".to_string(),
        access_policy: "free".to_string(),
        access_tier_id: None,
        price_cents: None,
        currency: None,
        rental_window_hours: None,
        default_audio_track_id: default_audio_track_id(&audio_tracks),
        default_caption_track_id: default_caption_track_id(&caption_tracks),
        default_preview_track_id: default_preview_track_id(&preview_tracks),
        audio_tracks,
        caption_tracks,
        preview_tracks,
    }))
}

async fn create_playback_session_for_content_id(
    state: SharedState,
    headers: HeaderMap,
    content_id: String,
) -> AppResult<Json<PlaybackGrant>> {
    let maybe_identity = optional_identity(&state.pool, &headers).await?;
    let target = fetch_upload_playback_target(&state.pool, &content_id).await?;
    let access =
        resolve_upload_playback_access(&state.pool, maybe_identity.as_ref(), &target).await?;
    let now = Utc::now();
    let session_id = format!("pbs-{}", Uuid::new_v4().simple());
    let playback_token = format!("pbt_{}", Uuid::new_v4().simple());
    let expires_at = (now + chrono::Duration::hours(6)).to_rfc3339();
    let now_rfc3339 = now.to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO playback_sessions (
            id, auth_session_id, user_id, creator_id, asset_id, content_id, content_kind, token_hash,
            access_scope, created_at, expires_at, last_used_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&session_id)
    .bind(
        maybe_identity
            .as_ref()
            .map(|identity| identity.session_id.clone()),
    )
    .bind(
        maybe_identity
            .as_ref()
            .map(|identity| identity.user_id.clone()),
    )
    .bind(Some(target.creator_id.clone()))
    .bind(&target.asset.id)
    .bind(&content_id)
    .bind(&target.asset.kind)
    .bind(hash_token(&playback_token))
    .bind(access.access_scope)
    .bind(&now_rfc3339)
    .bind(&expires_at)
    .bind(&now_rfc3339)
    .execute(&state.pool)
    .await?;

    let session = fetch_playback_session_by_id(&state.pool, &session_id).await?;
    let manifest_url = format!(
        "/api/v1/playback/sessions/{}/manifest?playbackToken={}",
        session.id, playback_token
    );
    let poster_url = target
        .asset
        .poster_path
        .as_ref()
        .map(|path| format!("/api/v1/media/{path}?playbackToken={playback_token}"));
    let preferred_subtitle_language = fetch_user_subtitle_preference(
        &state.pool,
        maybe_identity
            .as_ref()
            .map(|identity| identity.user_id.as_str()),
    )
    .await?;
    let (preferred_audio_language, prefer_dubbed) = fetch_user_audio_preferences(
        &state.pool,
        maybe_identity
            .as_ref()
            .map(|identity| identity.user_id.as_str()),
    )
    .await?;
    let audio_tracks = build_media_audio_tracks(
        &target.asset.status,
        &target.asset.id,
        &target.asset.variants,
        target.asset.audio_codec.as_deref(),
        Some(&playback_token),
        preferred_audio_language.as_deref(),
        prefer_dubbed,
    );
    let caption_tracks = build_media_caption_tracks(
        &target.asset.status,
        &target.asset.variants,
        Some(&playback_token),
        preferred_subtitle_language.as_deref(),
    );
    let preview_track_rows = fetch_media_preview_track_rows(&state.pool, &target.asset.id).await?;
    let preview_tracks = build_media_preview_tracks(
        &target.asset.status,
        &preview_track_rows,
        Some(&playback_token),
    );

    Ok(Json(PlaybackGrant {
        session,
        playback_token,
        manifest_url,
        poster_url,
        content_title: target.asset.title.clone(),
        content_kind: target.asset.kind.clone(),
        visibility: target.asset.visibility.clone(),
        access_policy: target.upload.access_policy.clone(),
        access_tier_id: target.upload.access_tier_id.clone(),
        price_cents: target.upload.price_cents,
        currency: target.upload.currency.clone(),
        rental_window_hours: target.upload.rental_window_hours,
        default_audio_track_id: default_audio_track_id(&audio_tracks),
        default_caption_track_id: default_caption_track_id(&caption_tracks),
        default_preview_track_id: default_preview_track_id(&preview_tracks),
        audio_tracks,
        caption_tracks,
        preview_tracks,
    }))
}

pub(super) async fn purchase_upload_access(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(upload_id): Path<String>,
) -> AppResult<Json<ContentPurchase>> {
    purchase_content_access_for_id(state, headers, upload_id).await
}

pub(super) async fn purchase_content_access(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(content_id): Path<String>,
) -> AppResult<Json<ContentPurchase>> {
    purchase_content_access_for_id(state, headers, content_id).await
}

async fn purchase_content_access_for_id(
    state: SharedState,
    headers: HeaderMap,
    content_id: String,
) -> AppResult<Json<ContentPurchase>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("purchase-upload:{}", identity.user_id),
        20,
        Duration::from_secs(60),
    )
    .await?;
    let target = fetch_upload_playback_target(&state.pool, &content_id).await?;
    let terms = resolve_upload_access_terms(
        Some(target.upload.access_policy.clone()),
        target.upload.access_tier_id.clone(),
        target.upload.price_cents,
        target.upload.currency.clone(),
        target.upload.rental_window_hours,
    )?;
    if terms.access_policy != "purchase" && terms.access_policy != "subscription_or_purchase" {
        return Err(AppError::BadRequest(
            "content is not configured for direct purchase".to_string(),
        ));
    }
    ensure_creator_can_accept_paid_transactions(&state.pool, &target.creator_id).await?;
    if let Some(existing_purchase) =
        fetch_current_content_purchase(&state.pool, &identity.user_id, &target.upload.id).await?
    {
        return Ok(Json(existing_purchase));
    }
    let now = Utc::now();
    let purchased_at = now.to_rfc3339();
    let expires_at = terms
        .rental_window_hours
        .map(|hours| (now + chrono::Duration::hours(hours)).to_rfc3339());
    let purchase_id = format!("pur-{}", Uuid::new_v4().simple());

    sqlx::query(
        r#"
        INSERT INTO content_purchases (
            id, user_id, creator_id, upload_id, access_policy, amount_cents, currency,
            status, purchased_at, expires_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, 'active', ?, ?)
        "#,
    )
    .bind(&purchase_id)
    .bind(&identity.user_id)
    .bind(&target.creator_id)
    .bind(&target.upload.id)
    .bind(&terms.access_policy)
    .bind(terms.price_cents.unwrap_or_default())
    .bind(terms.currency.clone().unwrap_or_else(|| "USD".to_string()))
    .bind(&purchased_at)
    .bind(expires_at)
    .execute(&state.pool)
    .await?;
    let buyer = fetch_user(&state.pool, &identity.user_id).await?;
    enqueue_notification_event(
        &state.pool,
        "content_purchase",
        &format!("{} purchased {}.", buyer.display_name, target.upload.title),
        Some(&identity.user_id),
        Some(&buyer.display_name),
        Some(&target.creator_id),
        None,
        Some(terms.price_cents.unwrap_or_default() as f64 / 100.0),
        json!({
            "purchaseId": purchase_id,
            "uploadId": target.upload.id,
            "accessPolicy": terms.access_policy,
        }),
        &[],
        &[target.creator_id.clone()],
    )
    .await?;

    Ok(Json(
        fetch_content_purchase_by_id(&state.pool, &purchase_id).await?,
    ))
}

pub(super) async fn get_playback_session(
    State(state): State<SharedState>,
    Path(session_id): Path<String>,
    Query(query): Query<PlaybackAccessQuery>,
) -> AppResult<Json<PlaybackGrant>> {
    let playback_token = query.playback_token.ok_or(AppError::Unauthorized)?;
    let session_record =
        validate_playback_session_record(&state.pool, &session_id, &playback_token).await?;
    Ok(Json(
        build_playback_grant_from_session_record(&state, &session_record, &playback_token).await?,
    ))
}

pub(super) async fn refresh_playback_session(
    State(state): State<SharedState>,
    Path(session_id): Path<String>,
    Query(query): Query<PlaybackAccessQuery>,
) -> AppResult<Json<PlaybackGrant>> {
    let playback_token = query.playback_token.ok_or(AppError::Unauthorized)?;
    let session_record =
        validate_playback_session_record(&state.pool, &session_id, &playback_token).await?;
    let refreshed_token = format!("pbt_{}", Uuid::new_v4().simple());
    let refreshed_at = Utc::now().to_rfc3339();
    let refreshed_expires_at = (Utc::now() + chrono::Duration::hours(6)).to_rfc3339();

    sqlx::query(
        r#"
        UPDATE playback_sessions
        SET token_hash = ?, expires_at = ?, last_used_at = ?
        WHERE id = ? AND token_hash = ? AND expires_at > ?
        "#,
    )
    .bind(hash_token(&refreshed_token))
    .bind(&refreshed_expires_at)
    .bind(&refreshed_at)
    .bind(&session_id)
    .bind(hash_token(&playback_token))
    .bind(&refreshed_at)
    .execute(&state.pool)
    .await?;

    let refreshed_record = PlaybackSessionRecord {
        expires_at: refreshed_expires_at,
        last_used_at: refreshed_at,
        ..session_record
    };

    Ok(Json(
        build_playback_grant_from_session_record(&state, &refreshed_record, &refreshed_token)
            .await?,
    ))
}

pub(super) async fn get_playback_manifest(
    State(state): State<SharedState>,
    Path(session_id): Path<String>,
    Query(query): Query<PlaybackAccessQuery>,
) -> AppResult<Response> {
    let playback_token = query.playback_token.ok_or(AppError::Unauthorized)?;
    let session = validate_playback_session(&state.pool, &session_id, &playback_token).await?;
    let manifest_relative_path = if session.content_kind == "live" {
        fetch_live_stream_playback_target(&state.pool, &session.content_id)
            .await?
            .playback_relative_path
    } else {
        fetch_upload_playback_target(&state.pool, &session.content_id)
            .await?
            .asset
            .playback_path
            .clone()
            .ok_or_else(|| AppError::BadRequest("playback manifest unavailable".to_string()))?
    };
    let manifest_path = media_path_for_relative(&state, &manifest_relative_path);
    let manifest_body = tokio::fs::read_to_string(&manifest_path).await?;
    let manifest_dir = PathBuf::from(&manifest_relative_path)
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| AppError::BadRequest("invalid playback manifest path".to_string()))?;

    let rewritten = manifest_body
        .lines()
        .map(|line| {
            if line.is_empty() {
                line.to_string()
            } else if line.starts_with("#EXT-X-MEDIA:") {
                rewrite_hls_manifest_media_uri_line(line, &manifest_dir, &playback_token)
            } else if line.starts_with('#') {
                line.to_string()
            } else {
                rewrite_hls_manifest_reference(line, &manifest_dir, &playback_token)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok((
        [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
        Body::from(format!("{rewritten}\n")),
    )
        .into_response())
}

async fn build_playback_grant_from_session_record(
    state: &SharedState,
    session_record: &PlaybackSessionRecord,
    playback_token: &str,
) -> AppResult<PlaybackGrant> {
    let session = playback_session_from_record(session_record);
    let manifest_url = format!(
        "/api/v1/playback/sessions/{}/manifest?playbackToken={}",
        session.id, playback_token
    );

    if session.content_kind == "live" {
        let target = fetch_live_stream_playback_target(&state.pool, &session.content_id).await?;
        let poster_url = target
            .poster_relative_path
            .as_ref()
            .map(|path| format!("/api/v1/media/{path}?playbackToken={playback_token}"));
        let preferred_subtitle_language =
            fetch_user_subtitle_preference(&state.pool, session_record.user_id.as_deref()).await?;
        let (preferred_audio_language, prefer_dubbed) =
            fetch_user_audio_preferences(&state.pool, session_record.user_id.as_deref()).await?;
        let audio_tracks = build_media_audio_tracks(
            &target.asset.status,
            &target.asset.id,
            &target.asset.variants,
            target.asset.audio_codec.as_deref(),
            Some(playback_token),
            preferred_audio_language.as_deref(),
            prefer_dubbed,
        );
        let caption_tracks = build_media_caption_tracks(
            &target.asset.status,
            &target.asset.variants,
            Some(playback_token),
            preferred_subtitle_language.as_deref(),
        );
        let preview_track_rows =
            fetch_media_preview_track_rows(&state.pool, &target.asset.id).await?;
        let preview_tracks = build_media_preview_tracks(
            &target.asset.status,
            &preview_track_rows,
            Some(playback_token),
        );
        return Ok(PlaybackGrant {
            session,
            playback_token: playback_token.to_string(),
            manifest_url,
            poster_url,
            content_title: target.title,
            content_kind: "live".to_string(),
            visibility: "public".to_string(),
            access_policy: "free".to_string(),
            access_tier_id: None,
            price_cents: None,
            currency: None,
            rental_window_hours: None,
            default_audio_track_id: default_audio_track_id(&audio_tracks),
            default_caption_track_id: default_caption_track_id(&caption_tracks),
            default_preview_track_id: default_preview_track_id(&preview_tracks),
            audio_tracks,
            caption_tracks,
            preview_tracks,
        });
    }

    let target = fetch_upload_playback_target(&state.pool, &session.content_id).await?;
    let poster_url = target
        .asset
        .poster_path
        .as_ref()
        .map(|path| format!("/api/v1/media/{path}?playbackToken={playback_token}"));
    let preferred_subtitle_language =
        fetch_user_subtitle_preference(&state.pool, session_record.user_id.as_deref()).await?;
    let (preferred_audio_language, prefer_dubbed) =
        fetch_user_audio_preferences(&state.pool, session_record.user_id.as_deref()).await?;
    let audio_tracks = build_media_audio_tracks(
        &target.asset.status,
        &target.asset.id,
        &target.asset.variants,
        target.asset.audio_codec.as_deref(),
        Some(playback_token),
        preferred_audio_language.as_deref(),
        prefer_dubbed,
    );
    let caption_tracks = build_media_caption_tracks(
        &target.asset.status,
        &target.asset.variants,
        Some(playback_token),
        preferred_subtitle_language.as_deref(),
    );
    let preview_track_rows = fetch_media_preview_track_rows(&state.pool, &target.asset.id).await?;
    let preview_tracks = build_media_preview_tracks(
        &target.asset.status,
        &preview_track_rows,
        Some(playback_token),
    );

    Ok(PlaybackGrant {
        session,
        playback_token: playback_token.to_string(),
        manifest_url,
        poster_url,
        content_title: target.asset.title.clone(),
        content_kind: target.asset.kind.clone(),
        visibility: target.asset.visibility.clone(),
        access_policy: target.upload.access_policy.clone(),
        access_tier_id: target.upload.access_tier_id.clone(),
        price_cents: target.upload.price_cents,
        currency: target.upload.currency.clone(),
        rental_window_hours: target.upload.rental_window_hours,
        default_audio_track_id: default_audio_track_id(&audio_tracks),
        default_caption_track_id: default_caption_track_id(&caption_tracks),
        default_preview_track_id: default_preview_track_id(&preview_tracks),
        audio_tracks,
        caption_tracks,
        preview_tracks,
    })
}
