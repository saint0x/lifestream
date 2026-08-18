use super::*;

pub(crate) async fn get_my_plan(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<BillingPlan>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_billing_plan(&state.pool, &identity.user_id).await?,
    ))
}

pub(crate) async fn list_sessions(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<AuthSession>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    Ok(Json(
        fetch_auth_sessions(&state.pool, &identity.user_id, &identity.session_id).await?,
    ))
}

pub(crate) async fn create_session(
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

pub(crate) async fn revoke_session(
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
