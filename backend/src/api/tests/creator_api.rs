use super::*;
use tower::ServiceExt;

async fn response_json<T: serde::de::DeserializeOwned>(response: Response) -> AppResult<T> {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(serde_json::from_slice(&body)?)
}

fn json_request(
    method: Method,
    uri: &str,
    token: Option<&str>,
    body: serde_json::Value,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    builder
        .body(Body::from(body.to_string()))
        .expect("valid test request")
}

fn get_request(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("valid test request")
}

#[tokio::test]
async fn creator_api_keys_are_viewable_and_authenticate_creator_api_only() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let session_token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let app = router(state.clone());

    let create_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/v1/me/api-keys",
            Some(&session_token),
            json!({
                "name": "Route test key",
                "scopes": ["creator:read", "creator:uploads:read"]
            }),
        ))
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    assert_eq!(create_response.status(), StatusCode::OK);
    let created: serde_json::Value = response_json(create_response).await?;
    let key_id = created["apiKey"]["id"].as_str().expect("api key id");
    let access_token = created["accessToken"].as_str().expect("api token");
    assert!(access_token.starts_with("vnta_live_"));
    assert_eq!(created["apiKey"]["accessToken"], access_token);

    let list_response = app
        .clone()
        .oneshot(get_request("/api/v1/me/api-keys", &session_token))
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let listed: serde_json::Value = response_json(list_response).await?;
    assert!(listed.as_array().is_some_and(|keys| {
        keys.iter()
            .any(|key| key["id"] == key_id && key["accessToken"] == access_token)
    }));

    let profile_response = app
        .clone()
        .oneshot(get_request("/api/v1/creator-api/profile", access_token))
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    assert_eq!(profile_response.status(), StatusCode::OK);
    let profile: serde_json::Value = response_json(profile_response).await?;
    assert_eq!(profile["id"], creator.id);

    let me_response = app
        .oneshot(get_request("/api/v1/me", access_token))
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    assert_eq!(me_response.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn creator_api_rejects_missing_scope_and_revoked_key() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let session_token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let app = router(state.clone());

    let create_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/v1/me/api-keys",
            Some(&session_token),
            json!({
                "name": "Read only route test key",
                "scopes": ["creator:read"]
            }),
        ))
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    assert_eq!(create_response.status(), StatusCode::OK);
    let created: serde_json::Value = response_json(create_response).await?;
    let key_id = created["apiKey"]["id"].as_str().expect("api key id");
    let access_token = created["accessToken"].as_str().expect("api token");

    let forbidden_response = app
        .clone()
        .oneshot(json_request(
            Method::PATCH,
            "/api/v1/creator-api/profile",
            Some(access_token),
            json!({ "tagline": "Should not write" }),
        ))
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    assert_eq!(forbidden_response.status(), StatusCode::FORBIDDEN);

    let revoke_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri(format!("/api/v1/me/api-keys/{key_id}"))
                .header(header::AUTHORIZATION, format!("Bearer {session_token}"))
                .body(Body::empty())
                .expect("valid revoke request"),
        )
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    assert_eq!(revoke_response.status(), StatusCode::NO_CONTENT);

    let revoked_response = app
        .oneshot(get_request("/api/v1/creator-api/profile", access_token))
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    assert_eq!(revoked_response.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}
