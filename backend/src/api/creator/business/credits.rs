use super::*;

#[derive(Clone, Debug)]
struct ResolvedCredit {
    person_id: String,
    role: String,
    character: Option<String>,
}

pub(super) async fn replace_project_credits(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((content_kind, content_id)): Path<(String, String)>,
    Json(input): Json<UpdateProjectCreditsRequest>,
) -> AppResult<Json<Vec<Credit>>> {
    let identity = require_identity(&state.db, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!(
            "creator-content-credits:{}:{}",
            identity.user_id, content_id
        ),
        30,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    validate_project_credits_input(&content_kind, &content_id, &input)?;

    if let Ok(pool) = state.db.try_postgres_adapter() {
        ensure_postgres_content_exists(pool, &content_kind, &content_id).await?;
        enforce_postgres_credit_authority(pool, creator_id, &content_id).await?;
        let credits =
            replace_postgres_credits(pool, &content_kind, &content_id, input.credits).await?;
        return Ok(Json(credits));
    }

    let pool = state.db.try_sqlite_adapter()?;
    ensure_sqlite_content_exists(pool, &content_kind, &content_id).await?;
    enforce_sqlite_credit_authority(pool, creator_id, &content_id).await?;
    Ok(Json(
        replace_sqlite_credits(pool, &content_kind, &content_id, input.credits).await?,
    ))
}

fn validate_project_credits_input(
    content_kind: &str,
    content_id: &str,
    input: &UpdateProjectCreditsRequest,
) -> AppResult<()> {
    if content_kind != "series" && content_kind != "film" {
        return Err(AppError::BadRequest(
            "contentKind must be series or film".to_string(),
        ));
    }
    if content_id.trim().is_empty() {
        return Err(AppError::BadRequest("contentId is required".to_string()));
    }
    if input.credits.len() > 200 {
        return Err(AppError::BadRequest(
            "a project can include at most 200 credits".to_string(),
        ));
    }
    for credit in &input.credits {
        let has_person_id = credit
            .person_id
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
        let has_person_slug = credit
            .person_slug
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
        if has_person_id == has_person_slug {
            return Err(AppError::BadRequest(
                "each credit must provide exactly one of personId or personSlug".to_string(),
            ));
        }
        validate_credit_text("role", &credit.role, 1, 80)?;
        if let Some(character) = credit.character.as_deref() {
            validate_credit_text("character", character, 0, 120)?;
        }
    }
    Ok(())
}

fn validate_credit_text(field: &str, value: &str, min: usize, max: usize) -> AppResult<()> {
    let len = value.trim().len();
    if len < min || len > max {
        return Err(AppError::BadRequest(format!(
            "{field} must be between {min} and {max} characters"
        )));
    }
    Ok(())
}

async fn ensure_sqlite_content_exists(
    pool: &sqlx::SqlitePool,
    content_kind: &str,
    content_id: &str,
) -> AppResult<()> {
    let table = content_table(content_kind)?;
    let exists = sqlx::query(&format!("SELECT 1 FROM {table} WHERE id = ? LIMIT 1"))
        .bind(content_id)
        .fetch_optional(pool)
        .await?
        .is_some();
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound)
    }
}

async fn ensure_postgres_content_exists(
    pool: &sqlx::PgPool,
    content_kind: &str,
    content_id: &str,
) -> AppResult<()> {
    let table = content_table(content_kind)?;
    let exists = sqlx::query(&format!("SELECT 1 FROM {table} WHERE id = $1 LIMIT 1"))
        .bind(content_id)
        .fetch_optional(pool)
        .await?
        .is_some();
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound)
    }
}

async fn enforce_sqlite_credit_authority(
    pool: &sqlx::SqlitePool,
    creator_id: &str,
    content_id: &str,
) -> AppResult<()> {
    let owner =
        sqlx::query("SELECT creator_id FROM upload_jobs WHERE published_content_id = ? LIMIT 1")
            .bind(content_id)
            .fetch_optional(pool)
            .await?;
    if let Some(row) = owner {
        let owner_id: String = row.get("creator_id");
        if owner_id != creator_id {
            return Err(AppError::Forbidden);
        }
    }
    Ok(())
}

async fn enforce_postgres_credit_authority(
    pool: &sqlx::PgPool,
    creator_id: &str,
    content_id: &str,
) -> AppResult<()> {
    let owner =
        sqlx::query("SELECT creator_id FROM upload_jobs WHERE published_content_id = $1 LIMIT 1")
            .bind(content_id)
            .fetch_optional(pool)
            .await?;
    if let Some(row) = owner {
        let owner_id: String = row.get("creator_id");
        if owner_id != creator_id {
            return Err(AppError::Forbidden);
        }
    }
    Ok(())
}

async fn replace_sqlite_credits(
    pool: &sqlx::SqlitePool,
    content_kind: &str,
    content_id: &str,
    input: Vec<ProjectCreditInput>,
) -> AppResult<Vec<Credit>> {
    let mut resolved = Vec::with_capacity(input.len());
    for credit in input {
        resolved.push(resolve_sqlite_credit_person(pool, credit).await?);
    }

    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM content_credits WHERE content_kind = ? AND content_id = ?")
        .bind(content_kind)
        .bind(content_id)
        .execute(&mut *tx)
        .await?;
    let now = Utc::now().to_rfc3339();
    for (index, credit) in resolved.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO content_credits (
                id, person_id, content_id, content_kind, role, character, credit_order, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(format!(
            "cc-{}-{}-{}",
            content_kind,
            content_id,
            Uuid::new_v4().simple()
        ))
        .bind(&credit.person_id)
        .bind(content_id)
        .bind(content_kind)
        .bind(&credit.role)
        .bind(credit.character.as_deref())
        .bind((index as i64) + 1)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    refresh_sqlite_content_credits(pool, content_kind, content_id).await
}

async fn replace_postgres_credits(
    pool: &sqlx::PgPool,
    content_kind: &str,
    content_id: &str,
    input: Vec<ProjectCreditInput>,
) -> AppResult<Vec<Credit>> {
    let mut resolved = Vec::with_capacity(input.len());
    for credit in input {
        resolved.push(resolve_postgres_credit_person(pool, credit).await?);
    }

    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM content_credits WHERE content_kind = $1 AND content_id = $2")
        .bind(content_kind)
        .bind(content_id)
        .execute(&mut *tx)
        .await?;
    let now = Utc::now().to_rfc3339();
    for (index, credit) in resolved.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO content_credits (
                id, person_id, content_id, content_kind, role, character, credit_order, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(format!(
            "cc-{}-{}-{}",
            content_kind,
            content_id,
            Uuid::new_v4().simple()
        ))
        .bind(&credit.person_id)
        .bind(content_id)
        .bind(content_kind)
        .bind(&credit.role)
        .bind(credit.character.as_deref())
        .bind((index as i64) + 1)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    refresh_postgres_content_credits(pool, content_kind, content_id).await
}

async fn resolve_sqlite_credit_person(
    pool: &sqlx::SqlitePool,
    credit: ProjectCreditInput,
) -> AppResult<ResolvedCredit> {
    let row = if let Some(person_id) = credit.person_id.as_deref() {
        sqlx::query("SELECT id FROM person_profiles WHERE id = ? LIMIT 1")
            .bind(person_id.trim())
            .fetch_optional(pool)
            .await?
    } else {
        sqlx::query("SELECT id FROM person_profiles WHERE slug = ? LIMIT 1")
            .bind(credit.person_slug.as_deref().unwrap_or_default().trim())
            .fetch_optional(pool)
            .await?
    }
    .ok_or_else(|| AppError::BadRequest("credit person was not found".to_string()))?;

    Ok(ResolvedCredit {
        person_id: row.get("id"),
        role: credit.role.trim().to_string(),
        character: credit.character.map(|value| value.trim().to_string()),
    })
}

async fn resolve_postgres_credit_person(
    pool: &sqlx::PgPool,
    credit: ProjectCreditInput,
) -> AppResult<ResolvedCredit> {
    let row = if let Some(person_id) = credit.person_id.as_deref() {
        sqlx::query("SELECT id FROM person_profiles WHERE id = $1 LIMIT 1")
            .bind(person_id.trim())
            .fetch_optional(pool)
            .await?
    } else {
        sqlx::query("SELECT id FROM person_profiles WHERE slug = $1 LIMIT 1")
            .bind(credit.person_slug.as_deref().unwrap_or_default().trim())
            .fetch_optional(pool)
            .await?
    }
    .ok_or_else(|| AppError::BadRequest("credit person was not found".to_string()))?;

    Ok(ResolvedCredit {
        person_id: row.get("id"),
        role: credit.role.trim().to_string(),
        character: credit.character.map(|value| value.trim().to_string()),
    })
}

async fn refresh_sqlite_content_credits(
    pool: &sqlx::SqlitePool,
    content_kind: &str,
    content_id: &str,
) -> AppResult<Vec<Credit>> {
    let rows = sqlx::query(
        r#"
        SELECT cc.id, p.id AS person_id, p.slug, p.display_name, cc.role, cc.character,
               COALESCE(NULLIF(cp.avatar, ''), NULLIF(u.avatar, ''), NULLIF(p.avatar, ''), '') AS avatar
        FROM content_credits cc
        JOIN person_profiles p ON p.id = cc.person_id
        LEFT JOIN users u ON u.id = p.user_id
        LEFT JOIN creator_profiles cp ON cp.user_id = p.user_id
        WHERE cc.content_kind = ? AND cc.content_id = ?
        ORDER BY cc.credit_order ASC
        "#,
    )
    .bind(content_kind)
    .bind(content_id)
    .fetch_all(pool)
    .await?;
    let credits = rows
        .into_iter()
        .map(sqlite_credit_from_row)
        .collect::<Vec<_>>();
    update_sqlite_content_credits_json(pool, content_kind, content_id, &credits).await?;
    Ok(credits)
}

async fn refresh_postgres_content_credits(
    pool: &sqlx::PgPool,
    content_kind: &str,
    content_id: &str,
) -> AppResult<Vec<Credit>> {
    let rows = sqlx::query(
        r#"
        SELECT cc.id, p.id AS person_id, p.slug, p.display_name, cc.role, cc.character,
               COALESCE(NULLIF(cp.avatar, ''), NULLIF(u.avatar, ''), NULLIF(p.avatar, ''), '') AS avatar
        FROM content_credits cc
        JOIN person_profiles p ON p.id = cc.person_id
        LEFT JOIN users u ON u.id = p.user_id
        LEFT JOIN creator_profiles cp ON cp.user_id = p.user_id
        WHERE cc.content_kind = $1 AND cc.content_id = $2
        ORDER BY cc.credit_order ASC
        "#,
    )
    .bind(content_kind)
    .bind(content_id)
    .fetch_all(pool)
    .await?;
    let credits = rows
        .into_iter()
        .map(postgres_credit_from_row)
        .collect::<Vec<_>>();
    update_postgres_content_credits_json(pool, content_kind, content_id, &credits).await?;
    Ok(credits)
}

fn sqlite_credit_from_row(row: sqlx::sqlite::SqliteRow) -> Credit {
    Credit {
        id: row.get("id"),
        person_id: Some(row.get("person_id")),
        person_slug: Some(row.get("slug")),
        name: row.get("display_name"),
        role: row.get("role"),
        character: row.get("character"),
        avatar: Some(row.get("avatar")),
    }
}

fn postgres_credit_from_row(row: sqlx::postgres::PgRow) -> Credit {
    Credit {
        id: row.get("id"),
        person_id: Some(row.get("person_id")),
        person_slug: Some(row.get("slug")),
        name: row.get("display_name"),
        role: row.get("role"),
        character: row.get("character"),
        avatar: Some(row.get("avatar")),
    }
}

async fn update_sqlite_content_credits_json(
    pool: &sqlx::SqlitePool,
    content_kind: &str,
    content_id: &str,
    credits: &[Credit],
) -> AppResult<()> {
    let table = content_table(content_kind)?;
    sqlx::query(&format!("UPDATE {table} SET credits_json = ? WHERE id = ?"))
        .bind(to_json(&credits)?)
        .bind(content_id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn update_postgres_content_credits_json(
    pool: &sqlx::PgPool,
    content_kind: &str,
    content_id: &str,
    credits: &[Credit],
) -> AppResult<()> {
    let table = content_table(content_kind)?;
    sqlx::query(&format!(
        "UPDATE {table} SET credits_json = $1 WHERE id = $2"
    ))
    .bind(to_json(&credits)?)
    .bind(content_id)
    .execute(pool)
    .await?;
    Ok(())
}

fn content_table(content_kind: &str) -> AppResult<&'static str> {
    match content_kind {
        "series" => Ok("series"),
        "film" => Ok("films"),
        _ => Err(AppError::BadRequest(
            "contentKind must be series or film".to_string(),
        )),
    }
}
