use super::*;
use crate::models::{
    ImageSet, PersonCredit, PersonProfile, PersonProfileLink, UpdatePersonProfileLinkRequest,
    UpdatePersonProfileRequest,
};
use sqlx::Row;

type LitePool = sqlx::Pool<sqlx::Sqlite>;
type LiteRow = sqlx::sqlite::SqliteRow;

pub(crate) async fn get_person_profile(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> AppResult<Json<PersonProfile>> {
    Ok(Json(fetch_person_profile(&state.db, &slug).await?))
}

pub(crate) async fn get_my_person_profile(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<PersonProfile>> {
    let identity = require_identity(&state.db, &headers).await?;
    Ok(Json(
        ensure_person_profile_for_user(&state.db, &identity.user_id).await?,
    ))
}

pub(crate) async fn update_my_person_profile(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<UpdatePersonProfileRequest>,
) -> AppResult<Json<PersonProfile>> {
    let identity = require_identity(&state.db, &headers).await?;
    validate_person_profile_update(&input)?;
    let profile = ensure_person_profile_for_user(&state.db, &identity.user_id).await?;
    update_person_profile(&state.db, &profile.id, &input).await?;
    Ok(Json(
        fetch_person_profile_by_id(&state.db, &profile.id).await?,
    ))
}

async fn fetch_person_profile(db: &crate::db::Database, slug: &str) -> AppResult<PersonProfile> {
    if let Ok(pool) = db.try_postgres_adapter() {
        let row = sqlx::query(person_profile_select("slug = $1").as_str())
            .bind(slug)
            .fetch_optional(pool)
            .await?
            .ok_or(AppError::NotFound)?;
        return pg_person_profile_from_row(pool, row).await;
    }
    let pool = db.try_sqlite_adapter()?;
    let row = sqlx::query(person_profile_select("slug = ?").as_str())
        .bind(slug)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    sqlite_person_profile_from_row(pool, row).await
}

async fn fetch_person_profile_by_id(
    db: &crate::db::Database,
    id: &str,
) -> AppResult<PersonProfile> {
    if let Ok(pool) = db.try_postgres_adapter() {
        let row = sqlx::query(person_profile_select("id = $1").as_str())
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or(AppError::NotFound)?;
        return pg_person_profile_from_row(pool, row).await;
    }
    let pool = db.try_sqlite_adapter()?;
    let row = sqlx::query(person_profile_select("id = ?").as_str())
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    sqlite_person_profile_from_row(pool, row).await
}

async fn ensure_person_profile_for_user(
    db: &crate::db::Database,
    user_id: &str,
) -> AppResult<PersonProfile> {
    let user = db.fetch_user(user_id).await?;
    let now = chrono::Utc::now().to_rfc3339();
    let id = format!("per-{}", uuid::Uuid::new_v4().simple());

    if let Ok(pool) = db.try_postgres_adapter() {
        if let Some(row) = sqlx::query(person_profile_select("user_id = $1").as_str())
            .bind(user_id)
            .fetch_optional(pool)
            .await?
        {
            return pg_person_profile_from_row(pool, row).await;
        }

        let slug_seed = if user.display_name.trim().is_empty() {
            user.handle.as_str()
        } else {
            user.display_name.as_str()
        };
        let slug = pg_unique_person_slug(pool, slug_seed).await?;
        sqlx::query(
            r#"
            INSERT INTO person_profiles (
                id, user_id, slug, display_name, avatar, hero_image, headline, location,
                about, known_for_json, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, '', '', '', '', '[]', $6, $6)
            "#,
        )
        .bind(&id)
        .bind(user_id)
        .bind(&slug)
        .bind(&user.display_name)
        .bind(&user.avatar)
        .bind(&now)
        .execute(pool)
        .await?;
        return fetch_person_profile_by_id(db, &id).await;
    }

    let pool = db.try_sqlite_adapter()?;
    if let Some(row) = sqlx::query(person_profile_select("user_id = ?").as_str())
        .bind(user_id)
        .fetch_optional(pool)
        .await?
    {
        return sqlite_person_profile_from_row(pool, row).await;
    }

    let slug_seed = if user.display_name.trim().is_empty() {
        user.handle.as_str()
    } else {
        user.display_name.as_str()
    };
    let slug = sqlite_unique_person_slug(pool, slug_seed).await?;
    sqlx::query(
        r#"
        INSERT INTO person_profiles (
            id, user_id, slug, display_name, avatar, hero_image, headline, location,
            about, known_for_json, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, '', '', '', '', '[]', ?, ?)
        "#,
    )
    .bind(&id)
    .bind(user_id)
    .bind(&slug)
    .bind(&user.display_name)
    .bind(&user.avatar)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    fetch_person_profile_by_id(db, &id).await
}

async fn update_person_profile(
    db: &crate::db::Database,
    person_id: &str,
    input: &UpdatePersonProfileRequest,
) -> AppResult<()> {
    let normalized_slug = input.slug.as_deref().map(normalize_slug);
    let known_for_json = input
        .known_for
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    let public_links = input
        .public_links
        .as_ref()
        .map(|links| normalize_profile_links(links))
        .transpose()?;
    let website_url = normalize_patch_url(&input.website_url)?;
    let instagram_url = normalize_patch_url(&input.instagram_url)?;
    let x_url = normalize_patch_url(&input.x_url)?;
    let imdb_url = normalize_patch_url(&input.imdb_url)?;
    let linkedin_url = normalize_patch_url(&input.linkedin_url)?;
    let facebook_url = normalize_patch_url(&input.facebook_url)?;
    let now = chrono::Utc::now().to_rfc3339();

    if let Ok(pool) = db.try_postgres_adapter() {
        if let Some(slug) = normalized_slug.as_deref() {
            let collision =
                sqlx::query("SELECT 1 FROM person_profiles WHERE slug = $1 AND id <> $2 LIMIT 1")
                    .bind(slug)
                    .bind(person_id)
                    .fetch_optional(pool)
                    .await?
                    .is_some();
            if collision {
                return Err(AppError::Conflict(
                    "profile slug is already taken".to_string(),
                ));
            }
        }
        sqlx::query(
            r#"
        UPDATE person_profiles
        SET slug = COALESCE($1, slug),
            display_name = COALESCE($2, display_name),
            avatar = COALESCE($3, avatar),
            hero_image = COALESCE($4, hero_image),
            headline = COALESCE($5, headline),
            location = COALESCE($6, location),
            about = COALESCE($7, about),
            known_for_json = COALESCE($8, known_for_json),
            website_url = CASE WHEN $9 THEN $10 ELSE website_url END,
            instagram_url = CASE WHEN $11 THEN $12 ELSE instagram_url END,
            x_url = CASE WHEN $13 THEN $14 ELSE x_url END,
            imdb_url = CASE WHEN $15 THEN $16 ELSE imdb_url END,
            linkedin_url = CASE WHEN $17 THEN $18 ELSE linkedin_url END,
            facebook_url = CASE WHEN $19 THEN $20 ELSE facebook_url END,
            updated_at = $21
        WHERE id = $22
        "#,
        )
        .bind(normalized_slug.as_deref())
        .bind(input.display_name.as_deref().map(str::trim))
        .bind(input.avatar.as_deref().map(str::trim))
        .bind(input.hero_image.as_deref().map(str::trim))
        .bind(input.headline.as_deref().map(str::trim))
        .bind(input.location.as_deref().map(str::trim))
        .bind(input.about.as_deref().map(str::trim))
        .bind(known_for_json.as_deref())
        .bind(input.website_url.is_set())
        .bind(website_url.as_deref())
        .bind(input.instagram_url.is_set())
        .bind(instagram_url.as_deref())
        .bind(input.x_url.is_set())
        .bind(x_url.as_deref())
        .bind(input.imdb_url.is_set())
        .bind(imdb_url.as_deref())
        .bind(input.linkedin_url.is_set())
        .bind(linkedin_url.as_deref())
        .bind(input.facebook_url.is_set())
        .bind(facebook_url.as_deref())
        .bind(&now)
        .bind(person_id)
        .execute(pool)
        .await?;
        if let Some(public_links) = public_links.as_ref() {
            replace_pg_person_profile_links(pool, person_id, public_links, &now).await?;
        }
        return Ok(());
    }

    let pool = db.try_sqlite_adapter()?;
    if let Some(slug) = normalized_slug.as_deref() {
        let collision =
            sqlx::query("SELECT 1 FROM person_profiles WHERE slug = ? AND id <> ? LIMIT 1")
                .bind(slug)
                .bind(person_id)
                .fetch_optional(pool)
                .await?
                .is_some();
        if collision {
            return Err(AppError::Conflict(
                "profile slug is already taken".to_string(),
            ));
        }
    }
    sqlx::query(
        r#"
        UPDATE person_profiles
        SET slug = COALESCE(?, slug),
            display_name = COALESCE(?, display_name),
            avatar = COALESCE(?, avatar),
            hero_image = COALESCE(?, hero_image),
            headline = COALESCE(?, headline),
            location = COALESCE(?, location),
            about = COALESCE(?, about),
            known_for_json = COALESCE(?, known_for_json),
            website_url = CASE WHEN ? THEN ? ELSE website_url END,
            instagram_url = CASE WHEN ? THEN ? ELSE instagram_url END,
            x_url = CASE WHEN ? THEN ? ELSE x_url END,
            imdb_url = CASE WHEN ? THEN ? ELSE imdb_url END,
            linkedin_url = CASE WHEN ? THEN ? ELSE linkedin_url END,
            facebook_url = CASE WHEN ? THEN ? ELSE facebook_url END,
            updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(normalized_slug.as_deref())
    .bind(input.display_name.as_deref().map(str::trim))
    .bind(input.avatar.as_deref().map(str::trim))
    .bind(input.hero_image.as_deref().map(str::trim))
    .bind(input.headline.as_deref().map(str::trim))
    .bind(input.location.as_deref().map(str::trim))
    .bind(input.about.as_deref().map(str::trim))
    .bind(known_for_json.as_deref())
    .bind(input.website_url.is_set())
    .bind(website_url.as_deref())
    .bind(input.instagram_url.is_set())
    .bind(instagram_url.as_deref())
    .bind(input.x_url.is_set())
    .bind(x_url.as_deref())
    .bind(input.imdb_url.is_set())
    .bind(imdb_url.as_deref())
    .bind(input.linkedin_url.is_set())
    .bind(linkedin_url.as_deref())
    .bind(input.facebook_url.is_set())
    .bind(facebook_url.as_deref())
    .bind(&now)
    .bind(person_id)
    .execute(pool)
    .await?;
    if let Some(public_links) = public_links.as_ref() {
        replace_sqlite_person_profile_links(pool, person_id, public_links, &now).await?;
    }
    Ok(())
}

fn person_profile_select(where_clause: &str) -> String {
    format!(
        r#"
        SELECT id, user_id, slug, display_name, avatar, hero_image, headline, location,
               about, known_for_json, website_url, instagram_url, x_url, imdb_url,
               linkedin_url, facebook_url, created_at, updated_at
        FROM person_profiles
        WHERE {where_clause}
        "#
    )
}

async fn pg_person_profile_from_row(
    pool: &sqlx::PgPool,
    row: sqlx::postgres::PgRow,
) -> AppResult<PersonProfile> {
    let id: String = row.get("id");
    let slug: String = row.get("slug");
    Ok(PersonProfile {
        id: id.clone(),
        user_id: row.get("user_id"),
        profile_url_path: profile_url_path(&slug),
        slug,
        display_name: row.get("display_name"),
        avatar: row.get("avatar"),
        hero_image: row.get("hero_image"),
        headline: row.get("headline"),
        location: row.get("location"),
        about: row.get("about"),
        known_for: from_json(row.get::<String, _>("known_for_json"))?,
        website_url: row.get("website_url"),
        instagram_url: row.get("instagram_url"),
        x_url: row.get("x_url"),
        imdb_url: row.get("imdb_url"),
        linkedin_url: row.get("linkedin_url"),
        facebook_url: row.get("facebook_url"),
        public_links: pg_fetch_person_profile_links(pool, &id).await?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        credits: pg_fetch_person_credits(pool, &id).await?,
    })
}

async fn sqlite_person_profile_from_row(pool: &LitePool, row: LiteRow) -> AppResult<PersonProfile> {
    let id: String = row.get("id");
    let slug: String = row.get("slug");
    Ok(PersonProfile {
        id: id.clone(),
        user_id: row.get("user_id"),
        profile_url_path: profile_url_path(&slug),
        slug,
        display_name: row.get("display_name"),
        avatar: row.get("avatar"),
        hero_image: row.get("hero_image"),
        headline: row.get("headline"),
        location: row.get("location"),
        about: row.get("about"),
        known_for: from_json(row.get::<String, _>("known_for_json"))?,
        website_url: row.get("website_url"),
        instagram_url: row.get("instagram_url"),
        x_url: row.get("x_url"),
        imdb_url: row.get("imdb_url"),
        linkedin_url: row.get("linkedin_url"),
        facebook_url: row.get("facebook_url"),
        public_links: sqlite_fetch_person_profile_links(pool, &id).await?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        credits: sqlite_fetch_person_credits(pool, &id).await?,
    })
}

#[derive(Clone, Debug)]
struct NormalizedProfileLink {
    platform: String,
    label: String,
    url: String,
    position: i64,
}

async fn pg_fetch_person_profile_links(
    pool: &sqlx::PgPool,
    person_id: &str,
) -> AppResult<Vec<PersonProfileLink>> {
    let rows = sqlx::query(
        r#"
        SELECT id, platform, label, url, position
        FROM person_profile_links
        WHERE person_id = $1
        ORDER BY position ASC
        "#,
    )
    .bind(person_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| PersonProfileLink {
            id: row.get("id"),
            platform: row.get("platform"),
            label: row.get("label"),
            url: row.get("url"),
            position: row.get("position"),
        })
        .collect())
}

async fn sqlite_fetch_person_profile_links(
    pool: &LitePool,
    person_id: &str,
) -> AppResult<Vec<PersonProfileLink>> {
    let rows = sqlx::query(
        r#"
        SELECT id, platform, label, url, position
        FROM person_profile_links
        WHERE person_id = ?
        ORDER BY position ASC
        "#,
    )
    .bind(person_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| PersonProfileLink {
            id: row.get("id"),
            platform: row.get("platform"),
            label: row.get("label"),
            url: row.get("url"),
            position: row.get("position"),
        })
        .collect())
}

async fn replace_pg_person_profile_links(
    pool: &sqlx::PgPool,
    person_id: &str,
    links: &[NormalizedProfileLink],
    now: &str,
) -> AppResult<()> {
    sqlx::query("DELETE FROM person_profile_links WHERE person_id = $1")
        .bind(person_id)
        .execute(pool)
        .await?;
    for link in links {
        sqlx::query(
            r#"
            INSERT INTO person_profile_links (
                id, person_id, platform, label, url, position, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $7)
            "#,
        )
        .bind(format!("plink-{}", uuid::Uuid::new_v4().simple()))
        .bind(person_id)
        .bind(&link.platform)
        .bind(&link.label)
        .bind(&link.url)
        .bind(link.position)
        .bind(now)
        .execute(pool)
        .await?;
    }
    Ok(())
}

async fn replace_sqlite_person_profile_links(
    pool: &LitePool,
    person_id: &str,
    links: &[NormalizedProfileLink],
    now: &str,
) -> AppResult<()> {
    sqlx::query("DELETE FROM person_profile_links WHERE person_id = ?")
        .bind(person_id)
        .execute(pool)
        .await?;
    for link in links {
        sqlx::query(
            r#"
            INSERT INTO person_profile_links (
                id, person_id, platform, label, url, position, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(format!("plink-{}", uuid::Uuid::new_v4().simple()))
        .bind(person_id)
        .bind(&link.platform)
        .bind(&link.label)
        .bind(&link.url)
        .bind(link.position)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
    }
    Ok(())
}

async fn pg_fetch_person_credits(
    pool: &sqlx::PgPool,
    person_id: &str,
) -> AppResult<Vec<PersonCredit>> {
    let rows = sqlx::query(
        r#"
        SELECT cc.content_id, cc.content_kind, cc.role, cc.character,
               COALESCE(s.slug, f.slug) AS content_slug,
               COALESCE(s.title, f.title) AS title,
               COALESCE(s.year, f.year)::BIGINT AS year,
               COALESCE(s.images_json, f.images_json) AS images_json
        FROM content_credits cc
        LEFT JOIN series s ON cc.content_kind = 'series' AND s.id = cc.content_id
        LEFT JOIN films f ON cc.content_kind = 'film' AND f.id = cc.content_id
        WHERE cc.person_id = $1
          AND (s.id IS NOT NULL OR f.id IS NOT NULL)
        ORDER BY year DESC, title ASC, cc.credit_order ASC
        "#,
    )
    .bind(person_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let images: ImageSet = from_json(row.get::<String, _>("images_json"))?;
            Ok(PersonCredit {
                content_id: row.get("content_id"),
                content_slug: row.get("content_slug"),
                content_kind: row.get("content_kind"),
                title: row.get("title"),
                year: row.get("year"),
                role: row.get("role"),
                character: row.get("character"),
                poster: images.poster,
            })
        })
        .collect()
}

async fn sqlite_fetch_person_credits(
    pool: &LitePool,
    person_id: &str,
) -> AppResult<Vec<PersonCredit>> {
    let rows = sqlx::query(
        r#"
        SELECT cc.content_id, cc.content_kind, cc.role, cc.character,
               COALESCE(s.slug, f.slug) AS content_slug,
               COALESCE(s.title, f.title) AS title,
               COALESCE(s.year, f.year) AS year,
               COALESCE(s.images_json, f.images_json) AS images_json
        FROM content_credits cc
        LEFT JOIN series s ON cc.content_kind = 'series' AND s.id = cc.content_id
        LEFT JOIN films f ON cc.content_kind = 'film' AND f.id = cc.content_id
        WHERE cc.person_id = ?
          AND (s.id IS NOT NULL OR f.id IS NOT NULL)
        ORDER BY year DESC, title ASC, cc.credit_order ASC
        "#,
    )
    .bind(person_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let images: ImageSet = from_json(row.get::<String, _>("images_json"))?;
            Ok(PersonCredit {
                content_id: row.get("content_id"),
                content_slug: row.get("content_slug"),
                content_kind: row.get("content_kind"),
                title: row.get("title"),
                year: row.get("year"),
                role: row.get("role"),
                character: row.get("character"),
                poster: images.poster,
            })
        })
        .collect()
}

async fn pg_unique_person_slug(pool: &sqlx::PgPool, seed: &str) -> AppResult<String> {
    let base = normalize_slug(seed);
    for index in 0..100 {
        let candidate = if index == 0 {
            base.clone()
        } else {
            format!("{base}-{index}")
        };
        let exists = sqlx::query("SELECT 1 FROM person_profiles WHERE slug = $1 LIMIT 1")
            .bind(&candidate)
            .fetch_optional(pool)
            .await?
            .is_some();
        if !exists {
            return Ok(candidate);
        }
    }
    Ok(format!("{base}-{}", uuid::Uuid::new_v4().simple()))
}

async fn sqlite_unique_person_slug(pool: &LitePool, seed: &str) -> AppResult<String> {
    let base = normalize_slug(seed);
    for index in 0..100 {
        let candidate = if index == 0 {
            base.clone()
        } else {
            format!("{base}-{index}")
        };
        let exists = sqlx::query("SELECT 1 FROM person_profiles WHERE slug = ? LIMIT 1")
            .bind(&candidate)
            .fetch_optional(pool)
            .await?
            .is_some();
        if !exists {
            return Ok(candidate);
        }
    }
    Ok(format!("{base}-{}", uuid::Uuid::new_v4().simple()))
}

fn validate_person_profile_update(input: &UpdatePersonProfileRequest) -> AppResult<()> {
    if let Some(slug) = input.slug.as_deref() {
        let normalized = normalize_slug(slug);
        if normalized.len() < 3 || normalized.len() > 64 {
            return Err(AppError::BadRequest(
                "slug must be 3-64 characters".to_string(),
            ));
        }
    }
    validate_len("displayName", input.display_name.as_deref(), 1, 80)?;
    validate_len("headline", input.headline.as_deref(), 0, 140)?;
    validate_len("location", input.location.as_deref(), 0, 80)?;
    validate_len("about", input.about.as_deref(), 0, 2000)?;
    if let Some(known_for) = input.known_for.as_ref() {
        if known_for.len() > 12 {
            return Err(AppError::BadRequest(
                "knownFor can include at most 12 entries".to_string(),
            ));
        }
        for value in known_for {
            validate_len("knownFor", Some(value), 1, 48)?;
        }
    }
    normalize_patch_url(&input.website_url)?;
    normalize_patch_url(&input.instagram_url)?;
    normalize_patch_url(&input.x_url)?;
    normalize_patch_url(&input.imdb_url)?;
    normalize_patch_url(&input.linkedin_url)?;
    normalize_patch_url(&input.facebook_url)?;
    if let Some(links) = input.public_links.as_ref() {
        normalize_profile_links(links)?;
    }
    Ok(())
}

fn normalize_profile_links(
    links: &[UpdatePersonProfileLinkRequest],
) -> AppResult<Vec<NormalizedProfileLink>> {
    if links.len() > 12 {
        return Err(AppError::BadRequest(
            "publicLinks can include at most 12 links".to_string(),
        ));
    }

    let mut normalized = Vec::new();
    for link in links {
        let label = link.label.trim();
        validate_len("public link label", Some(label), 1, 48)?;
        let Some(url) = normalize_link_url(Some(&link.url))? else {
            continue;
        };
        let platform = link
            .platform
            .as_deref()
            .map(normalize_platform)
            .filter(|value: &String| !value.is_empty())
            .unwrap_or_else(|| "custom".to_string());
        normalized.push(NormalizedProfileLink {
            platform,
            label: label.to_string(),
            url,
            position: normalized.len() as i64,
        });
    }
    Ok(normalized)
}

fn normalize_patch_url(input: &crate::models::NullablePatch<String>) -> AppResult<Option<String>> {
    match input.as_deref() {
        Some(value) => normalize_link_url(value),
        None => Ok(None),
    }
}

fn normalize_link_url(input: Option<&str>) -> AppResult<Option<String>> {
    let Some(value) = input else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > 500 {
        return Err(AppError::BadRequest(
            "profile link URLs must be 500 characters or fewer".to_string(),
        ));
    }
    let parsed = reqwest::Url::parse(value).map_err(|_| {
        AppError::BadRequest("profile links must be valid http:// or https:// URLs".to_string())
    })?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(AppError::BadRequest(
            "profile links must be valid http:// or https:// URLs".to_string(),
        ));
    }
    Ok(Some(parsed.to_string()))
}

fn normalize_platform(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn validate_len(field: &str, value: Option<&str>, min: usize, max: usize) -> AppResult<()> {
    if let Some(value) = value {
        let len = value.trim().len();
        if len < min || len > max {
            return Err(AppError::BadRequest(format!(
                "{field} must be between {min} and {max} characters"
            )));
        }
    }
    Ok(())
}

fn normalize_slug(value: &str) -> String {
    let slug = value
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "person".to_string()
    } else {
        slug
    }
}

fn profile_url_path(slug: &str) -> String {
    format!("/@{slug}")
}
