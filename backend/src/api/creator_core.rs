use super::*;

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/api/v1/creator/me/dashboard", get(creator_dashboard))
        .route("/api/v1/creator/me/state", get(get_creator_state))
        .route(
            "/api/v1/creator/me/analytics/summary",
            get(get_creator_analytics_summary),
        )
        .route(
            "/api/v1/creator/me/revenue/summary",
            get(get_creator_revenue_summary),
        )
        .route(
            "/api/v1/creator/me/operations",
            get(get_creator_operational_state).patch(update_creator_operational_state),
        )
        .route(
            "/api/v1/admin/creators/:creator_id/enforcement",
            get(get_admin_creator_enforcement_state),
        )
        .route(
            "/api/v1/admin/creators/:creator_id/enforcement/actions",
            post(create_admin_creator_enforcement_action),
        )
        .route(
            "/api/v1/admin/creators/:creator_id/enforcement/actions/:action_id",
            get(get_admin_creator_enforcement_action),
        )
        .route(
            "/api/v1/admin/creators/:creator_id/enforcement/actions/:action_id/reconcile",
            post(reconcile_admin_creator_enforcement_action),
        )
        .route(
            "/api/v1/admin/creators/:creator_id/enforcement/actions/:action_id/release",
            post(release_admin_creator_enforcement_action),
        )
        .route(
            "/api/v1/creator/me/live",
            get(get_creator_live).patch(update_creator_live),
        )
        .route("/api/v1/creator/me/live/control", get(get_creator_live_control))
        .route("/api/v1/creator/me/live/runtime", get(get_creator_live_runtime))
        .route(
            "/api/v1/creator/me/live/socket-sessions/:socket_id",
            get(get_creator_live_socket_session),
        )
        .route(
            "/api/v1/creator/me/live/socket-sessions/:socket_id/reconcile",
            post(reconcile_creator_live_socket_session),
        )
        .route(
            "/api/v1/creator/me/live/settings",
            get(get_creator_live_settings).patch(update_creator_live_settings),
        )
        .route(
            "/api/v1/creator/me/live/health",
            get(get_creator_live_health),
        )
        .route(
            "/api/v1/creator/me/upload-operations",
            get(get_creator_upload_operations),
        )
}

async fn creator_dashboard(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorDashboard>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_creator_scope()?;
    let payload = creator_dashboard_payload(&state.pool, &identity).await?;
    Ok(Json(payload))
}

pub(super) async fn get_creator_state(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorAppState>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_creator_app_state(
            &state.pool,
            &identity,
            &CreatorContentQuery {
                kind: None,
                status: None,
                q: None,
                sort: None,
            },
        )
        .await?,
    ))
}

async fn get_creator_upload_operations(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorUploadOperationsResponse>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_creator_upload_operations_response(&state.pool, creator_id).await?,
    ))
}

async fn get_creator_operational_state(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorOperationalState>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let profile = fetch_creator_profile(&state.pool, creator_id).await?;
    Ok(Json(
        fetch_creator_operational_state(&state.pool, &profile).await?,
    ))
}

async fn update_creator_operational_state(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<UpdateCreatorOperationalStateRequest>,
) -> AppResult<Json<CreatorOperationalState>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let profile = fetch_creator_profile(&state.pool, creator_id).await?;
    let current = fetch_creator_operational_state(&state.pool, &profile).await?;
    let now = Utc::now().to_rfc3339();

    let legal_name = input
        .legal_name
        .unwrap_or(current.legal_name)
        .trim()
        .to_string();
    let support_email = input
        .support_email
        .unwrap_or(current.support_email)
        .trim()
        .to_string();
    let business_type = input
        .business_type
        .unwrap_or(current.business_type)
        .trim()
        .to_string();
    let payout_country = input
        .payout_country
        .unwrap_or(current.payout_country)
        .trim()
        .to_ascii_uppercase();
    let payout_provider = input
        .payout_provider
        .unwrap_or(current.payout_provider)
        .trim()
        .to_string();

    if legal_name.is_empty()
        || support_email.is_empty()
        || business_type.is_empty()
        || payout_country.is_empty()
        || payout_provider.is_empty()
    {
        return Err(AppError::BadRequest(
            "legalName, supportEmail, businessType, payoutCountry, and payoutProvider must be non-empty"
                .to_string(),
        ));
    }
    if !support_email.contains('@') {
        return Err(AppError::BadRequest(
            "supportEmail must be a valid email address".to_string(),
        ));
    }
    if payout_country.len() < 2 || payout_country.len() > 3 {
        return Err(AppError::BadRequest(
            "payoutCountry must be a 2-3 character country code".to_string(),
        ));
    }

    let onboarding_status = transition_creator_operational_status(
        &current.onboarding_status,
        input.submit_onboarding.unwrap_or(false),
        "approved",
        "blocked",
    )?;
    let identity_status = transition_creator_operational_status(
        &current.identity_status,
        input.submit_identity_verification.unwrap_or(false),
        "verified",
        "rejected",
    )?;
    let tax_status = transition_creator_operational_status(
        &current.tax_status,
        input.submit_tax_profile.unwrap_or(false),
        "verified",
        "rejected",
    )?;
    let payout_status = transition_creator_operational_status(
        &current.payout_status,
        input.submit_payout_method.unwrap_or(false),
        "active",
        "disabled",
    )?;

    sqlx::query(
        r#"
        UPDATE creator_operational_state
        SET legal_name = ?, support_email = ?, business_type = ?, payout_country = ?, payout_provider = ?,
            onboarding_status = ?, identity_status = ?, tax_status = ?, payout_status = ?, updated_at = ?
        WHERE creator_id = ?
        "#,
    )
    .bind(&legal_name)
    .bind(&support_email)
    .bind(&business_type)
    .bind(&payout_country)
    .bind(&payout_provider)
    .bind(&onboarding_status)
    .bind(&identity_status)
    .bind(&tax_status)
    .bind(&payout_status)
    .bind(&now)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;

    let refreshed_profile = fetch_creator_profile(&state.pool, creator_id).await?;
    Ok(Json(
        fetch_creator_operational_state(&state.pool, &refreshed_profile).await?,
    ))
}

async fn get_admin_creator_enforcement_state(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(creator_id): Path<String>,
) -> AppResult<Json<CreatorEnforcementState>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    let profile = fetch_creator_profile(&state.pool, &creator_id).await?;
    Ok(Json(
        fetch_creator_enforcement_state(&state.pool, &profile).await?,
    ))
}

async fn create_admin_creator_enforcement_action(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(creator_id): Path<String>,
    Json(input): Json<CreateCreatorEnforcementActionRequest>,
) -> AppResult<Json<CreatorEnforcementAction>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    let profile = fetch_creator_profile(&state.pool, &creator_id).await?;
    validate_creator_enforcement_scope(&input.scope)?;
    if input.reason.trim().is_empty() {
        return Err(AppError::BadRequest("reason is required".to_string()));
    }

    let expires_at = parse_optional_future_timestamp(input.expires_at.as_deref())?;
    let now = Utc::now().to_rfc3339();
    let action_id = format!("cea-{}", Uuid::new_v4().simple());

    sqlx::query(
        r#"
        INSERT INTO creator_enforcement_actions (
            id, creator_id, scope, state, reason, resolution_note, created_by_user_id,
            released_by_user_id, created_at, released_at, expires_at
        ) VALUES (?, ?, ?, 'active', ?, NULL, ?, NULL, ?, NULL, ?)
        "#,
    )
    .bind(&action_id)
    .bind(&creator_id)
    .bind(input.scope.trim())
    .bind(input.reason.trim())
    .bind(&identity.user_id)
    .bind(&now)
    .bind(expires_at.as_deref())
    .execute(&state.pool)
    .await?;

    write_moderation_audit_entry(
        &state.pool,
        &creator_id,
        None,
        &identity.user_id,
        Some(&profile.user_id),
        "creator_enforcement_applied",
        json!({
            "actionId": action_id,
            "scope": input.scope.trim(),
            "reason": input.reason.trim(),
            "expiresAt": expires_at,
        }),
    )
    .await?;
    enqueue_notification_event(
        &state.pool,
        "creator_enforcement_applied",
        &format!(
            "A creator enforcement action was applied to {}.",
            profile.display_name
        ),
        Some(&identity.user_id),
        Some("operator"),
        Some(&creator_id),
        None,
        None,
        json!({
            "actionId": action_id,
            "scope": input.scope.trim(),
            "reason": input.reason.trim(),
            "expiresAt": expires_at,
        }),
        &[],
        &[creator_id.clone()],
    )
    .await?;

    Ok(Json(
        fetch_creator_enforcement_action_by_id(&state.pool, &action_id).await?,
    ))
}

pub(super) async fn get_admin_creator_enforcement_action(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((creator_id, action_id)): Path<(String, String)>,
) -> AppResult<Json<CreatorEnforcementAction>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    let action = fetch_creator_enforcement_action_by_id_raw(&state.pool, &action_id).await?;
    if action.creator_id != creator_id {
        return Err(AppError::NotFound);
    }
    Ok(Json(action))
}

pub(super) async fn reconcile_admin_creator_enforcement_action(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((creator_id, action_id)): Path<(String, String)>,
) -> AppResult<Json<CreatorEnforcementReconciliationReport>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    let action = fetch_creator_enforcement_action_by_id_raw(&state.pool, &action_id).await?;
    if action.creator_id != creator_id {
        return Err(AppError::NotFound);
    }
    Ok(Json(
        reconcile_single_creator_enforcement_action(state, &action_id).await?,
    ))
}

async fn release_admin_creator_enforcement_action(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((creator_id, action_id)): Path<(String, String)>,
    Json(input): Json<ReleaseCreatorEnforcementActionRequest>,
) -> AppResult<Json<CreatorEnforcementAction>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_admin_scope()?;
    let profile = fetch_creator_profile(&state.pool, &creator_id).await?;
    let action = fetch_creator_enforcement_action_by_id(&state.pool, &action_id).await?;
    if action.creator_id != creator_id {
        return Err(AppError::NotFound);
    }
    if action.state != "active" {
        return Ok(Json(action));
    }
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        UPDATE creator_enforcement_actions
        SET state = 'released', resolution_note = ?, released_by_user_id = ?, released_at = ?
        WHERE id = ? AND creator_id = ?
        "#,
    )
    .bind(input.resolution_note.as_deref())
    .bind(&identity.user_id)
    .bind(&now)
    .bind(&action_id)
    .bind(&creator_id)
    .execute(&state.pool)
    .await?;

    write_moderation_audit_entry(
        &state.pool,
        &creator_id,
        None,
        &identity.user_id,
        Some(&profile.user_id),
        "creator_enforcement_released",
        json!({
            "actionId": action_id,
            "scope": action.scope,
            "resolutionNote": input.resolution_note,
            "releasedAt": now,
        }),
    )
    .await?;
    enqueue_notification_event(
        &state.pool,
        "creator_enforcement_released",
        &format!(
            "A creator enforcement action was released for {}.",
            profile.display_name
        ),
        Some(&identity.user_id),
        Some("operator"),
        Some(&creator_id),
        None,
        None,
        json!({
            "actionId": action_id,
            "scope": action.scope,
            "resolutionNote": input.resolution_note,
            "releasedAt": now,
        }),
        &[],
        &[creator_id.clone()],
    )
    .await?;

    Ok(Json(
        fetch_creator_enforcement_action_by_id(&state.pool, &action_id).await?,
    ))
}

async fn get_creator_analytics_summary(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorAnalyticsSummary>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let analytics = fetch_analytics(&state.pool, creator_id).await?;
    Ok(Json(summarize_creator_analytics(&analytics)))
}

async fn get_creator_revenue_summary(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorRevenueSummary>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let analytics = fetch_analytics(&state.pool, creator_id).await?;
    let revenue = fetch_revenue_entries(&state.pool, creator_id).await?;
    let subscriber_tiers = fetch_creator_subscriber_tiers(&state.pool, creator_id).await?;
    Ok(Json(summarize_creator_revenue(
        &analytics,
        &revenue,
        &subscriber_tiers,
    )))
}

pub(super) async fn get_creator_live(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorLiveSnapshot>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        build_creator_live_snapshot(&state.pool, creator_id).await?,
    ))
}

async fn get_creator_live_control(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorLiveControlResponse>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_authoritative_creator_live_control_response(&state, creator_id).await?,
    ))
}

async fn get_creator_live_runtime(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorLiveRuntimeResponse>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_authoritative_creator_live_runtime_response(&state, creator_id).await?,
    ))
}

pub(super) async fn get_creator_live_socket_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(socket_id): Path<String>,
) -> AppResult<Json<CreatorLiveSocketPresence>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_creator_live_socket_presence_by_id_raw(&state.pool, creator_id, &socket_id).await?,
    ))
}

pub(super) async fn reconcile_creator_live_socket_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(socket_id): Path<String>,
) -> AppResult<Json<CreatorLiveSocketPresenceReconciliationReport>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let socket_session =
        fetch_creator_live_socket_presence_by_id_raw(&state.pool, creator_id, &socket_id).await?;
    if socket_session.creator_id != creator_id {
        return Err(AppError::NotFound);
    }
    Ok(Json(
        reconcile_single_creator_live_socket_session(state, creator_id, &socket_id).await?,
    ))
}

async fn get_creator_live_settings(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorLiveSettings>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_creator_live_settings(&state.pool, creator_id).await?,
    ))
}

async fn update_creator_live_settings(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<UpdateCreatorLiveSettingsRequest>,
) -> AppResult<Json<CreatorLiveSettings>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-live-settings:{}", identity.user_id),
        30,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    let current = fetch_creator_live_settings(&state.pool, creator_id).await?;
    if let Some(value) = input.slow_mode_seconds {
        validate_slow_mode_seconds(value)?;
    }
    if let Some(value) = input.auto_mod_level.as_deref() {
        validate_auto_mod_level(value)?;
    }

    let scenes = input.scenes.unwrap_or(current.scenes);
    let active_scene_id = input
        .active_scene_id
        .unwrap_or_else(|| current.active_scene_id.clone());

    sqlx::query(
        r#"
        UPDATE creator_live_settings
        SET subscriber_only = ?, slow_mode_seconds = ?, auto_mod_level = ?,
            notify_followers_default = ?, active_scene_id = ?, scenes_json = ?
        WHERE creator_id = ?
        "#,
    )
    .bind(input.subscriber_only.unwrap_or(current.subscriber_only) as i64)
    .bind(input.slow_mode_seconds.unwrap_or(current.slow_mode_seconds))
    .bind(
        input
            .auto_mod_level
            .as_deref()
            .unwrap_or(current.auto_mod_level.as_str()),
    )
    .bind(
        input
            .notify_followers_default
            .unwrap_or(current.notify_followers_default) as i64,
    )
    .bind(&active_scene_id)
    .bind(to_json(&scenes)?)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;

    let settings = fetch_creator_live_settings(&state.pool, creator_id).await?;
    publish_creator_live_state(&state, creator_id).await?;
    Ok(Json(settings))
}

async fn get_creator_live_health(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorLiveHealth>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_creator_live_health(&state.pool, creator_id).await?,
    ))
}
