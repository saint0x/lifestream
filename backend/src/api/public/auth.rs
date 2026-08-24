use serde::{Deserialize, Serialize};

use super::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EmailSignUpRequest {
    email: String,
    password: String,
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EmailSignInRequest {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SocialSignInRequest {
    provider: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GoogleCallbackQuery {
    code: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    sub: String,
    email: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthResponse {
    access_token: String,
    user: User,
}

const SESSION_DAYS: i64 = 365;

pub(crate) async fn create_guest_session(
    State(state): State<SharedState>,
) -> AppResult<Json<AuthResponse>> {
    let id_suffix = Uuid::new_v4().simple().to_string();
    let user_id = format!("guest-{}", &id_suffix[..12]);
    let handle = format!("guest{}", &id_suffix[..10]);
    let display_name = "Guest Creator".to_string();
    provision_user_and_creator(&state, &user_id, &handle, &display_name).await?;
    issue_auth_response(&state, &user_id, "Guest session").await
}

pub(crate) async fn sign_up_email(
    State(state): State<SharedState>,
    Json(input): Json<EmailSignUpRequest>,
) -> AppResult<Json<AuthResponse>> {
    let email = normalize_email(&input.email)?;
    let password_hash = crate::better_auth_runtime::hash_password(&input.password).await?;
    let display_name = input
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| email.split('@').next().unwrap_or("Creator"));
    let handle = unique_handle(&state.db, email.split('@').next().unwrap_or("creator")).await?;
    let user_id = format!("usr-{}", Uuid::new_v4().simple());
    provision_user_and_creator(&state, &user_id, &handle, display_name).await?;
    let now = Utc::now().to_rfc3339();
    state
        .db
        .create_email_credential(&user_id, &email, &password_hash, &now)
        .await?;
    issue_auth_response(&state, &user_id, "Email sign up").await
}

pub(crate) async fn sign_in_email(
    State(state): State<SharedState>,
    Json(input): Json<EmailSignInRequest>,
) -> AppResult<Json<AuthResponse>> {
    let email = normalize_email(&input.email)?;
    let credential = state
        .db
        .fetch_email_credential(&email)
        .await?
        .ok_or(AppError::Unauthorized)?;
    crate::better_auth_runtime::verify_password(&input.password, &credential.password_hash).await?;
    issue_auth_response(&state, &credential.user_id, "Email sign in").await
}

pub(crate) async fn sign_in_social(Json(input): Json<SocialSignInRequest>) -> AppResult<Response> {
    match input.provider.trim().to_ascii_lowercase().as_str() {
        "google" => start_google_auth().await,
        _ => Err(AppError::BadRequest(
            "That sign-in provider is not configured.".to_string(),
        )),
    }
}

pub(crate) async fn start_google_auth() -> AppResult<Response> {
    let client_id = std::env::var("VANTA_GOOGLE_CLIENT_ID")
        .map_err(|_| AppError::BadRequest("Google sign-in is not configured.".to_string()))?;
    let redirect_uri = std::env::var("VANTA_GOOGLE_REDIRECT_URI")
        .unwrap_or_else(|_| "http://127.0.0.1:8080/api/auth/callback/google".to_string());
    let state = Uuid::new_v4().simple().to_string();
    let mut url = "https://accounts.google.com/o/oauth2/v2/auth".to_string();
    url.push_str("?response_type=code");
    url.push_str("&scope=openid%20email%20profile");
    url.push_str("&access_type=offline");
    url.push_str("&prompt=select_account");
    url.push_str("&client_id=");
    url.push_str(&url_encode(&client_id));
    url.push_str("&redirect_uri=");
    url.push_str(&url_encode(&redirect_uri));
    url.push_str("&state=");
    url.push_str(&state);
    Ok((
        StatusCode::FOUND,
        [(
            header::LOCATION,
            HeaderValue::from_str(&url)
                .map_err(|_| AppError::BadRequest("invalid Google redirect URL".to_string()))?,
        )],
    )
        .into_response())
}

pub(crate) async fn google_oauth_callback(
    State(state): State<SharedState>,
    Query(query): Query<GoogleCallbackQuery>,
) -> AppResult<Response> {
    if let Some(error) = query.error {
        return redirect_to_frontend(&format!("/auth/callback?error={}", url_encode(&error)));
    }
    let code = query
        .code
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest("Missing Google OAuth code.".to_string()))?;
    let client_id = std::env::var("VANTA_GOOGLE_CLIENT_ID")
        .map_err(|_| AppError::BadRequest("Google sign-in is not configured.".to_string()))?;
    let client_secret = std::env::var("VANTA_GOOGLE_CLIENT_SECRET")
        .map_err(|_| AppError::BadRequest("Google sign-in is not configured.".to_string()))?;
    let redirect_uri = std::env::var("VANTA_GOOGLE_REDIRECT_URI")
        .unwrap_or_else(|_| "http://127.0.0.1:8080/api/auth/callback/google".to_string());

    let client = reqwest::Client::new();
    let token = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", code.as_str()),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|_| AppError::BadRequest("Google sign-in failed.".to_string()))?
        .error_for_status()
        .map_err(|_| AppError::BadRequest("Google sign-in failed.".to_string()))?
        .json::<GoogleTokenResponse>()
        .await
        .map_err(|_| AppError::BadRequest("Google sign-in failed.".to_string()))?;
    let google_user = client
        .get("https://openidconnect.googleapis.com/v1/userinfo")
        .bearer_auth(&token.access_token)
        .send()
        .await
        .map_err(|_| AppError::BadRequest("Google profile lookup failed.".to_string()))?
        .error_for_status()
        .map_err(|_| AppError::BadRequest("Google profile lookup failed.".to_string()))?
        .json::<GoogleUserInfo>()
        .await
        .map_err(|_| AppError::BadRequest("Google profile lookup failed.".to_string()))?;

    let user_id = match state
        .db
        .fetch_oauth_account("google", &google_user.sub)
        .await?
    {
        Some(account) => account.user_id,
        None => {
            let email = google_user
                .email
                .as_deref()
                .unwrap_or("google.user@vanta.local");
            let display_name = google_user
                .name
                .as_deref()
                .or_else(|| email.split('@').next())
                .unwrap_or("VANTA Viewer");
            let existing_email_user = state
                .db
                .fetch_email_credential(&normalize_email(email)?)
                .await?
                .map(|credential| credential.user_id);
            let user_id = match existing_email_user {
                Some(user_id) => user_id,
                None => {
                    let handle_seed = email.split('@').next().unwrap_or("viewer");
                    let handle = unique_handle(&state.db, handle_seed).await?;
                    let user_id = format!("usr-{}", Uuid::new_v4().simple());
                    provision_user_and_creator(&state, &user_id, &handle, display_name).await?;
                    user_id
                }
            };
            let now = Utc::now().to_rfc3339();
            state
                .db
                .upsert_oauth_account(
                    &format!("oauth-{}", Uuid::new_v4().simple()),
                    &user_id,
                    "google",
                    &google_user.sub,
                    google_user.email.as_deref(),
                    google_user.name.as_deref(),
                    &now,
                )
                .await?;
            user_id
        }
    };
    let auth = issue_auth_response_payload(&state, &user_id, "Google sign in").await?;
    redirect_to_frontend(&format!(
        "/auth/callback?accessToken={}",
        url_encode(&auth.access_token)
    ))
}

async fn provision_user_and_creator(
    state: &SharedState,
    user_id: &str,
    handle: &str,
    display_name: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    state
        .db
        .provision_user(crate::db::ProvisionedUser {
            id: user_id,
            handle,
            display_name,
            avatar_url: "",
            tier: "free",
            joined_at: &now,
        })
        .await?;
    let creator_id = format!("cr-{handle}");
    let stream_key = format!(
        "sk_{}_{}",
        Uuid::new_v4().simple(),
        &Uuid::new_v4().simple().to_string()[..8]
    );
    let default_tags_json = to_json(&vec!["live".to_string(), "creator".to_string()])?;
    state
        .db
        .provision_creator(crate::db::ProvisionedCreator {
            id: &creator_id,
            user_id,
            handle,
            display_name,
            avatar_url: "",
            banner_url: "",
            tagline: "Live on VANTA",
            bio: "Creator workspace",
            partner_status: "standard",
            joined_at: &now,
            stream_key: &stream_key,
            rtmp_url: "rtmp://127.0.0.1:1935/live",
            default_category: "Tech",
            default_tags_json: &default_tags_json,
        })
        .await?;
    let default_scenes_json = to_json(&vec![
        serde_json::json!({"id": "cam-main", "label": "Main cam", "active": true}),
        serde_json::json!({"id": "screen", "label": "Screen + cam", "active": false}),
        serde_json::json!({"id": "slide", "label": "Slideshow", "active": false}),
        serde_json::json!({"id": "brb", "label": "BRB loop", "active": false}),
    ])?;
    state
        .db
        .provision_creator_defaults(
            &creator_id,
            display_name,
            &format!("{handle}@vanta.local"),
            &now,
            &default_scenes_json,
        )
        .await?;
    Ok(())
}

async fn issue_auth_response(
    state: &SharedState,
    user_id: &str,
    label: &str,
) -> AppResult<Json<AuthResponse>> {
    Ok(Json(
        issue_auth_response_payload(state, user_id, label).await?,
    ))
}

async fn issue_auth_response_payload(
    state: &SharedState,
    user_id: &str,
    label: &str,
) -> AppResult<AuthResponse> {
    let access_token = crate::better_auth_runtime::new_session_token();
    let session_id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();
    let expires_at = (Utc::now() + ChronoDuration::days(SESSION_DAYS)).to_rfc3339();
    let scopes = vec!["user".to_string(), "creator".to_string()];
    let scopes_json = to_json(&scopes)?;
    let token_hash = hash_token(&access_token);
    state
        .db
        .create_auth_session(crate::db::NewAuthSession {
            id: &session_id,
            user_id,
            label,
            token_hash: &token_hash,
            scopes_json: &scopes_json,
            created_at: &created_at,
            expires_at: Some(&expires_at),
        })
        .await?;
    Ok(AuthResponse {
        access_token,
        user: state.db.fetch_user(user_id).await?,
    })
}

fn normalize_email(value: &str) -> AppResult<String> {
    let email = value.trim().to_lowercase();
    if !email.contains('@') || email.len() > 254 {
        return Err(AppError::BadRequest(
            "Enter a valid email address.".to_string(),
        ));
    }
    Ok(email)
}

async fn unique_handle(database: &crate::db::Database, seed: &str) -> AppResult<String> {
    let base = sanitize_handle(seed);
    database.unique_user_handle(&base).await
}

fn sanitize_handle(value: &str) -> String {
    let handle: String = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .take(24)
        .collect();
    if handle.len() >= 3 {
        handle
    } else {
        "creator".to_string()
    }
}

fn url_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

fn redirect_to_frontend(path: &str) -> AppResult<Response> {
    let origin = std::env::var("VANTA_FRONTEND_ORIGIN")
        .unwrap_or_else(|_| "https://streamvanta.tv".to_string());
    let location = format!("{}{}", origin.trim_end_matches('/'), path);
    Ok((
        StatusCode::FOUND,
        [(
            header::LOCATION,
            HeaderValue::from_str(&location)
                .map_err(|_| AppError::BadRequest("invalid frontend redirect URL".to_string()))?,
        )],
    )
        .into_response())
}
