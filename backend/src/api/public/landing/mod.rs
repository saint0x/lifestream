use super::*;
use serde::{Deserialize, Serialize};
use sqlx::{Row, postgres::PgRow, sqlite::SqliteRow};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LandingSignupRequest {
    kind: String,
    audience: String,
    name: String,
    email: String,
    company: Option<String>,
    website: Option<String>,
    budget: Option<String>,
    message: Option<String>,
    source_path: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LandingSignupResponse {
    id: String,
    kind: String,
    audience: String,
    status: String,
    created_at: String,
}

pub(crate) async fn create_landing_signup(
    State(state): State<SharedState>,
    Json(input): Json<LandingSignupRequest>,
) -> AppResult<Json<LandingSignupResponse>> {
    let signup = normalize_signup(&input)?;
    enforce_rate_limit(
        &state,
        &format!("landing-signup:{}:{}", signup.kind, signup.email),
        4,
        Duration::from_secs(60),
    )
    .await?;

    Ok(Json(insert_landing_signup(&state.db, signup).await?))
}

struct NormalizedSignup<'a> {
    kind: String,
    audience: String,
    name: &'a str,
    email: String,
    company: Option<String>,
    website: Option<String>,
    budget: Option<String>,
    message: Option<String>,
    source_path: Option<String>,
}

async fn insert_landing_signup(
    db: &crate::db::Database,
    signup: NormalizedSignup<'_>,
) -> AppResult<LandingSignupResponse> {
    let id = format!("lsu-{}", Uuid::new_v4().simple());
    let now = Utc::now().to_rfc3339();

    if let Ok(pool) = db.try_postgres_adapter() {
        let row = sqlx::query(
            r#"
            INSERT INTO landing_signups (
                id, kind, audience, name, email, company, website, budget, message,
                source_path, status, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'new', $11, $11)
            RETURNING id, kind, audience, status, created_at
            "#,
        )
        .bind(&id)
        .bind(&signup.kind)
        .bind(&signup.audience)
        .bind(signup.name)
        .bind(&signup.email)
        .bind(signup.company.as_deref())
        .bind(signup.website.as_deref())
        .bind(signup.budget.as_deref())
        .bind(signup.message.as_deref())
        .bind(signup.source_path.as_deref())
        .bind(&now)
        .fetch_one(pool)
        .await?;
        return landing_signup_from_pg_row(row);
    }

    let row = sqlx::query(
        r#"
        INSERT INTO landing_signups (
            id, kind, audience, name, email, company, website, budget, message,
            source_path, status, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'new', ?, ?)
        RETURNING id, kind, audience, status, created_at
        "#,
    )
    .bind(&id)
    .bind(&signup.kind)
    .bind(&signup.audience)
    .bind(signup.name)
    .bind(&signup.email)
    .bind(signup.company.as_deref())
    .bind(signup.website.as_deref())
    .bind(signup.budget.as_deref())
    .bind(signup.message.as_deref())
    .bind(signup.source_path.as_deref())
    .bind(&now)
    .bind(&now)
    .fetch_one(db.try_sqlite_adapter()?)
    .await?;
    landing_signup_from_sqlite_row(row)
}

fn landing_signup_from_pg_row(row: PgRow) -> AppResult<LandingSignupResponse> {
    Ok(LandingSignupResponse {
        id: row.get("id"),
        kind: row.get("kind"),
        audience: row.get("audience"),
        status: row.get("status"),
        created_at: row.get("created_at"),
    })
}

fn landing_signup_from_sqlite_row(row: SqliteRow) -> AppResult<LandingSignupResponse> {
    Ok(LandingSignupResponse {
        id: row.get("id"),
        kind: row.get("kind"),
        audience: row.get("audience"),
        status: row.get("status"),
        created_at: row.get("created_at"),
    })
}

fn normalize_signup<'a>(input: &'a LandingSignupRequest) -> AppResult<NormalizedSignup<'a>> {
    let kind = normalize_kind(&input.kind)?;
    let audience = normalize_audience(&input.audience)?;
    let name = require_text(&input.name, "name", 120)?;
    let email = normalize_email(&input.email)?;
    let company = optional_text(input.company.as_deref(), 160);
    let website = optional_text(input.website.as_deref(), 260);
    let budget = optional_text(input.budget.as_deref(), 160);
    let message = optional_text(input.message.as_deref(), 1200);
    let source_path = optional_text(input.source_path.as_deref(), 260);

    if kind == "buyer" && company.is_none() {
        return Err(AppError::BadRequest("company is required".to_string()));
    }
    if kind == "buyer" && website.is_none() {
        return Err(AppError::BadRequest("website is required".to_string()));
    }
    if kind == "creator" && website.is_none() {
        return Err(AppError::BadRequest("main channel is required".to_string()));
    }

    Ok(NormalizedSignup {
        kind,
        audience,
        name,
        email,
        company,
        website,
        budget,
        message,
        source_path,
    })
}

fn normalize_kind(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "creator" => Ok("creator".to_string()),
        "buyer" | "advertiser" | "agency" => Ok("buyer".to_string()),
        "general" | "early_access" => Ok("general".to_string()),
        _ => Err(AppError::BadRequest("unsupported signup type".to_string())),
    }
}

fn normalize_audience(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "home" | "creators" | "buyers" => Ok(value.trim().to_ascii_lowercase()),
        _ => Err(AppError::BadRequest(
            "unsupported signup audience".to_string(),
        )),
    }
}

fn normalize_email(value: &str) -> AppResult<String> {
    let email = require_text(value, "email", 240)?.to_ascii_lowercase();
    if !email.contains('@') || !email.contains('.') {
        return Err(AppError::BadRequest("email is invalid".to_string()));
    }
    Ok(email)
}

fn require_text<'a>(value: &'a str, label: &str, max_len: usize) -> AppResult<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::BadRequest(format!("{label} is required")));
    }
    if value.len() > max_len {
        return Err(AppError::BadRequest(format!("{label} is too long")));
    }
    Ok(value)
}

fn optional_text(value: Option<&str>, max_len: usize) -> Option<String> {
    value
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| item.chars().take(max_len).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(kind: &str) -> LandingSignupRequest {
        LandingSignupRequest {
            kind: kind.to_string(),
            audience: "creators".to_string(),
            name: "Maya Creator".to_string(),
            email: "MAYA@EXAMPLE.COM".to_string(),
            company: None,
            website: Some("https://example.com".to_string()),
            budget: None,
            message: Some("A launch series".to_string()),
            source_path: Some("/creators".to_string()),
        }
    }

    #[test]
    fn normalizes_creator_signup() {
        let input = request("creator");
        let signup = normalize_signup(&input).expect("creator signup should normalize");

        assert_eq!(signup.kind, "creator");
        assert_eq!(signup.audience, "creators");
        assert_eq!(signup.email, "maya@example.com");
        assert_eq!(signup.website.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn buyer_requires_company_and_website() {
        let mut input = request("buyer");
        input.audience = "buyers".to_string();
        input.website = None;

        assert!(normalize_signup(&input).is_err());

        input.website = Some("https://brand.example".to_string());
        input.company = Some("Northstar Supply".to_string());

        let signup = normalize_signup(&input).expect("buyer signup should normalize");
        assert_eq!(signup.kind, "buyer");
        assert_eq!(signup.company.as_deref(), Some("Northstar Supply"));
    }

    #[test]
    fn rejects_invalid_email() {
        let mut input = request("creator");
        input.email = "not-an-email".to_string();

        assert!(normalize_signup(&input).is_err());
    }
}
