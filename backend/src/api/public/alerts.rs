use super::*;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgRow, sqlite::SqliteRow};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreatePublicAlertSubscriptionRequest {
    target_kind: String,
    target_id: String,
    target_slug: Option<String>,
    target_title: String,
    visitor_id: Option<String>,
    contact_channel: String,
    contact_value: String,
    social_platform: Option<String>,
    alert_types: Vec<String>,
    source_path: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublicAlertSubscriptionResponse {
    id: String,
    target_kind: String,
    target_id: String,
    target_title: String,
    contact_channel: String,
    social_platform: Option<String>,
    alert_types: Vec<String>,
    status: String,
    updated_at: String,
}

pub(crate) async fn create_public_alert_subscription(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<CreatePublicAlertSubscriptionRequest>,
) -> AppResult<Json<PublicAlertSubscriptionResponse>> {
    let identity = optional_identity(&state.db, &headers).await?;
    let target_kind = normalize_alert_target_kind(&input.target_kind)?;
    let target_id = require_clean_text(&input.target_id, "target id", 160)?;
    let target_title = require_clean_text(&input.target_title, "target title", 240)?;
    let target_slug = optional_clean_text(input.target_slug.as_deref(), 180);
    let visitor_id = optional_clean_text(input.visitor_id.as_deref(), 160);
    let contact_channel = normalize_alert_contact_channel(&input.contact_channel)?;
    let contact_value = normalize_alert_contact_value(&contact_channel, &input.contact_value)?;
    let social_platform = if contact_channel == "social_dm" {
        Some(normalize_social_platform(input.social_platform.as_deref())?)
    } else {
        None
    };
    let alert_types = normalize_alert_types(&input.alert_types)?;
    let source_path = optional_clean_text(input.source_path.as_deref(), 360);
    let rate_key = identity
        .as_ref()
        .map(|item| format!("public-alert:{}", item.user_id))
        .or_else(|| {
            visitor_id
                .as_ref()
                .map(|item| format!("public-alert:{item}"))
        })
        .unwrap_or_else(|| format!("public-alert:{}:{contact_value}", contact_channel));
    enforce_rate_limit(&state, &rate_key, 12, Duration::from_secs(60)).await?;
    ensure_alert_target_exists(&state.db, &target_kind, &target_id).await?;

    let user_id = identity.as_ref().map(|item| item.user_id.as_str());
    let response = upsert_public_alert_subscription(
        &state.db,
        PublicAlertSubscriptionUpsert {
            target_kind: &target_kind,
            target_id,
            target_slug: target_slug.as_deref(),
            target_title,
            visitor_id: visitor_id.as_deref(),
            user_id,
            contact_channel: &contact_channel,
            contact_value,
            social_platform: social_platform.as_deref(),
            alert_types: &alert_types,
            source_path: source_path.as_deref(),
        },
    )
    .await?;
    Ok(Json(response))
}

struct PublicAlertSubscriptionUpsert<'a> {
    target_kind: &'a str,
    target_id: &'a str,
    target_slug: Option<&'a str>,
    target_title: &'a str,
    visitor_id: Option<&'a str>,
    user_id: Option<&'a str>,
    contact_channel: &'a str,
    contact_value: &'a str,
    social_platform: Option<&'a str>,
    alert_types: &'a [String],
    source_path: Option<&'a str>,
}

async fn upsert_public_alert_subscription(
    db: &crate::db::Database,
    input: PublicAlertSubscriptionUpsert<'_>,
) -> AppResult<PublicAlertSubscriptionResponse> {
    let now = Utc::now().to_rfc3339();
    let id = format!("als-{}", Uuid::new_v4().simple());
    let alert_types_json = to_json(&input.alert_types.to_vec())?;
    if let Ok(pool) = db.try_postgres_adapter() {
        let row = sqlx::query(
            r#"
            INSERT INTO public_alert_subscriptions (
                id, target_kind, target_id, target_slug, target_title, visitor_id, user_id,
                contact_channel, contact_value, social_platform, alert_types_json, source_path,
                status, created_at, updated_at, last_confirmed_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 'active', $13, $13, $13)
            ON CONFLICT(target_kind, target_id, contact_channel, contact_value)
            DO UPDATE SET
                target_slug = excluded.target_slug,
                target_title = excluded.target_title,
                visitor_id = COALESCE(excluded.visitor_id, public_alert_subscriptions.visitor_id),
                user_id = COALESCE(excluded.user_id, public_alert_subscriptions.user_id),
                social_platform = excluded.social_platform,
                alert_types_json = excluded.alert_types_json,
                source_path = excluded.source_path,
                status = 'active',
                updated_at = excluded.updated_at,
                last_confirmed_at = excluded.last_confirmed_at
            RETURNING id, target_kind, target_id, target_title, contact_channel, social_platform,
                      alert_types_json, status, updated_at
            "#,
        )
        .bind(&id)
        .bind(input.target_kind)
        .bind(input.target_id)
        .bind(input.target_slug)
        .bind(input.target_title)
        .bind(input.visitor_id)
        .bind(input.user_id)
        .bind(input.contact_channel)
        .bind(input.contact_value)
        .bind(input.social_platform)
        .bind(&alert_types_json)
        .bind(input.source_path)
        .bind(&now)
        .fetch_one(pool)
        .await?;
        return alert_subscription_response_from_pg_row(row);
    }

    let pool = db.try_sqlite_adapter()?;
    let row = sqlx::query(
        r#"
        INSERT INTO public_alert_subscriptions (
            id, target_kind, target_id, target_slug, target_title, visitor_id, user_id,
            contact_channel, contact_value, social_platform, alert_types_json, source_path,
            status, created_at, updated_at, last_confirmed_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, ?)
        ON CONFLICT(target_kind, target_id, contact_channel, contact_value)
        DO UPDATE SET
            target_slug = excluded.target_slug,
            target_title = excluded.target_title,
            visitor_id = COALESCE(excluded.visitor_id, public_alert_subscriptions.visitor_id),
            user_id = COALESCE(excluded.user_id, public_alert_subscriptions.user_id),
            social_platform = excluded.social_platform,
            alert_types_json = excluded.alert_types_json,
            source_path = excluded.source_path,
            status = 'active',
            updated_at = excluded.updated_at,
            last_confirmed_at = excluded.last_confirmed_at
        RETURNING id, target_kind, target_id, target_title, contact_channel, social_platform,
                  alert_types_json, status, updated_at
        "#,
    )
    .bind(&id)
    .bind(input.target_kind)
    .bind(input.target_id)
    .bind(input.target_slug)
    .bind(input.target_title)
    .bind(input.visitor_id)
    .bind(input.user_id)
    .bind(input.contact_channel)
    .bind(input.contact_value)
    .bind(input.social_platform)
    .bind(&alert_types_json)
    .bind(input.source_path)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .fetch_one(pool)
    .await?;
    alert_subscription_response_from_sqlite_row(row)
}

fn alert_subscription_response_from_pg_row(
    row: PgRow,
) -> AppResult<PublicAlertSubscriptionResponse> {
    alert_subscription_response_from_parts(
        row.get("id"),
        row.get("target_kind"),
        row.get("target_id"),
        row.get("target_title"),
        row.get("contact_channel"),
        row.get("social_platform"),
        row.get("alert_types_json"),
        row.get("status"),
        row.get("updated_at"),
    )
}

fn alert_subscription_response_from_sqlite_row(
    row: SqliteRow,
) -> AppResult<PublicAlertSubscriptionResponse> {
    alert_subscription_response_from_parts(
        row.get("id"),
        row.get("target_kind"),
        row.get("target_id"),
        row.get("target_title"),
        row.get("contact_channel"),
        row.get("social_platform"),
        row.get("alert_types_json"),
        row.get("status"),
        row.get("updated_at"),
    )
}

fn alert_subscription_response_from_parts(
    id: String,
    target_kind: String,
    target_id: String,
    target_title: String,
    contact_channel: String,
    social_platform: Option<String>,
    alert_types_json: String,
    status: String,
    updated_at: String,
) -> AppResult<PublicAlertSubscriptionResponse> {
    Ok(PublicAlertSubscriptionResponse {
        id,
        target_kind,
        target_id,
        target_title,
        contact_channel,
        social_platform,
        alert_types: from_json(alert_types_json)?,
        status,
        updated_at,
    })
}

async fn ensure_alert_target_exists(
    db: &crate::db::Database,
    target_kind: &str,
    target_id: &str,
) -> AppResult<()> {
    if let Ok(pool) = db.try_postgres_adapter() {
        let exists = match target_kind {
            "profile" => sqlx::query("SELECT 1 FROM person_profiles WHERE id = $1 LIMIT 1")
                .bind(target_id)
                .fetch_optional(pool)
                .await?
                .is_some(),
            "series" => sqlx::query("SELECT 1 FROM series WHERE id = $1 LIMIT 1")
                .bind(target_id)
                .fetch_optional(pool)
                .await?
                .is_some(),
            "episode" => sqlx::query("SELECT 1 FROM episodes WHERE id = $1 LIMIT 1")
                .bind(target_id)
                .fetch_optional(pool)
                .await?
                .is_some(),
            _ => false,
        };
        return if exists {
            Ok(())
        } else {
            Err(AppError::NotFound)
        };
    }

    let pool = db.try_sqlite_adapter()?;
    let exists = match target_kind {
        "profile" => sqlx::query("SELECT 1 FROM person_profiles WHERE id = ? LIMIT 1")
            .bind(target_id)
            .fetch_optional(pool)
            .await?
            .is_some(),
        "series" => sqlx::query("SELECT 1 FROM series WHERE id = ? LIMIT 1")
            .bind(target_id)
            .fetch_optional(pool)
            .await?
            .is_some(),
        "episode" => sqlx::query("SELECT 1 FROM episodes WHERE id = ? LIMIT 1")
            .bind(target_id)
            .fetch_optional(pool)
            .await?
            .is_some(),
        _ => false,
    };
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound)
    }
}

fn normalize_alert_target_kind(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "profile" | "creator" => Ok("profile".to_string()),
        "series" => Ok("series".to_string()),
        "episode" => Ok("episode".to_string()),
        _ => Err(AppError::BadRequest("unsupported alert target".to_string())),
    }
}

fn normalize_alert_contact_channel(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "email" => Ok("email".to_string()),
        "sms" | "text" => Ok("sms".to_string()),
        "social" | "social_dm" | "dm" => Ok("social_dm".to_string()),
        _ => Err(AppError::BadRequest("unsupported alert method".to_string())),
    }
}

fn normalize_alert_contact_value<'a>(channel: &str, value: &'a str) -> AppResult<&'a str> {
    let value = require_clean_text(value, "contact", 240)?;
    let valid = match channel {
        "email" => value.contains('@') && value.contains('.'),
        "sms" => value.chars().filter(|item| item.is_ascii_digit()).count() >= 10,
        "social_dm" => {
            value.starts_with('@') || value.starts_with("http://") || value.starts_with("https://")
        }
        _ => false,
    };
    if valid {
        Ok(value)
    } else {
        Err(AppError::BadRequest(
            "enter a valid alert contact".to_string(),
        ))
    }
}

fn normalize_social_platform(value: Option<&str>) -> AppResult<String> {
    match value
        .map(|item| item.trim().to_ascii_lowercase().replace('-', "_"))
        .unwrap_or_default()
        .as_str()
    {
        "instagram" | "x" | "twitter" | "tiktok" | "facebook" | "linkedin" => Ok(value
            .unwrap_or("instagram")
            .trim()
            .to_ascii_lowercase()
            .replace('-', "_")),
        _ => Err(AppError::BadRequest(
            "choose a supported social platform".to_string(),
        )),
    }
}

fn normalize_alert_types(values: &[String]) -> AppResult<Vec<String>> {
    let mut normalized = Vec::new();
    for value in values {
        let item = match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "new_episode" | "episode_drop" => "new_episode",
            "series_drop" | "new_series" => "series_drop",
            "creator_update" | "creator_updates" => "creator_update",
            "release_reminder" | "reminder" => "release_reminder",
            _ => continue,
        };
        if !normalized.iter().any(|existing| existing == item) {
            normalized.push(item.to_string());
        }
    }
    if normalized.is_empty() {
        normalized.push("new_episode".to_string());
    }
    Ok(normalized)
}

fn require_clean_text<'a>(value: &'a str, label: &str, max_len: usize) -> AppResult<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::BadRequest(format!("{label} is required")));
    }
    if value.len() > max_len {
        return Err(AppError::BadRequest(format!("{label} is too long")));
    }
    Ok(value)
}

fn optional_clean_text(value: Option<&str>, max_len: usize) -> Option<String> {
    value
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| item.chars().take(max_len).collect())
}
