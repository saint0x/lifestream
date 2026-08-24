use super::*;

pub(crate) async fn get_my_plan(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<BillingPlan>> {
    let identity = require_identity(&state.db, &headers).await?;
    if state.database_kind == crate::config::DatabaseKind::Postgres {
        return Ok(Json(
            super::state::build_postgres_viewer_app_state(
                &state.db,
                &identity.user_id,
                &identity.session_id,
            )
            .await?
            .plan,
        ));
    }
    Ok(Json(
        fetch_billing_plan(state.db.sqlite_adapter(), &identity.user_id).await?,
    ))
}

pub(crate) async fn list_sessions(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<AuthSession>>> {
    let identity = require_identity(&state.db, &headers).await?;
    Ok(Json(
        state
            .db
            .list_auth_sessions(&identity.user_id, &identity.session_id, None)
            .await?,
    ))
}

pub(crate) async fn create_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<CreateSessionRequest>,
) -> AppResult<Json<SessionTokenResponse>> {
    let identity = require_identity(&state.db, &headers).await?;
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

    let token_hash = crate::auth::hash_token(&access_token);
    let scopes_json = to_json(&scopes)?;
    state
        .db
        .create_auth_session(crate::db::NewAuthSession {
            id: &session_id,
            user_id: &identity.user_id,
            label,
            token_hash: &token_hash,
            scopes_json: &scopes_json,
            created_at: &created_at,
            expires_at: expires_at.as_deref(),
        })
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

pub(crate) async fn revoke_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let identity = require_identity(&state.db, &headers).await?;
    let revoked_at = Utc::now().to_rfc3339();
    if state
        .db
        .revoke_auth_session(&id, &identity.user_id, &revoked_at)
        .await?
        == 0
    {
        return Err(AppError::NotFound);
    }

    state
        .db
        .expire_playback_sessions_for_auth_session(&id, &revoked_at)
        .await?;

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
