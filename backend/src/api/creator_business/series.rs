use super::*;

pub(super) async fn list_creator_series(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<CreatorSeriesProject>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(fetch_creator_series(&state.pool, creator_id).await?))
}

pub(super) async fn create_creator_series(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<CreateCreatorSeriesRequest>,
) -> AppResult<Json<CreatorSeriesProject>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-series-create:{}", identity.user_id),
        20,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    if input.slug.trim().is_empty() || input.title.trim().is_empty() {
        return Err(AppError::BadRequest(
            "slug and title are required".to_string(),
        ));
    }

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO creator_series_projects (
            id, creator_id, slug, title, synopsis, rating, genres_json, hero_color,
            poster_url, backdrop_url, status, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(creator_id)
    .bind(input.slug.trim())
    .bind(input.title.trim())
    .bind(input.synopsis.trim())
    .bind(input.rating.trim())
    .bind(to_json(&input.genres)?)
    .bind(input.hero_color.trim())
    .bind(input.poster_url.trim())
    .bind(input.backdrop_url.trim())
    .bind(input.status.trim())
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;

    Ok(Json(
        fetch_creator_series_by_id(&state.pool, creator_id, &id).await?,
    ))
}

pub(super) async fn update_creator_series(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<UpdateCreatorSeriesRequest>,
) -> AppResult<Json<CreatorSeriesProject>> {
    let identity = require_identity(&state.pool, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-series-update:{}", identity.user_id),
        40,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    let current = fetch_creator_series_by_id(&state.pool, creator_id, &id).await?;

    sqlx::query(
        r#"
        UPDATE creator_series_projects
        SET title = ?, synopsis = ?, rating = ?, genres_json = ?, hero_color = ?,
            poster_url = ?, backdrop_url = ?, status = ?, updated_at = ?
        WHERE id = ? AND creator_id = ?
        "#,
    )
    .bind(input.title.unwrap_or(current.title))
    .bind(input.synopsis.unwrap_or(current.synopsis))
    .bind(input.rating.unwrap_or(current.rating))
    .bind(to_json(&input.genres.unwrap_or(current.genres))?)
    .bind(input.hero_color.unwrap_or(current.hero_color))
    .bind(input.poster_url.unwrap_or(current.poster_url))
    .bind(input.backdrop_url.unwrap_or(current.backdrop_url))
    .bind(input.status.unwrap_or(current.status))
    .bind(Utc::now().to_rfc3339())
    .bind(&id)
    .bind(creator_id)
    .execute(&state.pool)
    .await?;

    Ok(Json(
        fetch_creator_series_by_id(&state.pool, creator_id, &id).await?,
    ))
}
