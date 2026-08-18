use super::*;

pub(crate) async fn list_series(State(state): State<SharedState>) -> AppResult<Json<Vec<Series>>> {
    Ok(Json(fetch_series(&state.pool, None, None).await?))
}

pub(crate) async fn get_series(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> AppResult<Json<Series>> {
    let maybe_identity = optional_identity(&state.pool, &headers).await?;
    let progress = match maybe_identity {
        Some(identity) => {
            fetch_continue_watching_entry(&state.pool, &identity.user_id, None, &slug).await?
        }
        None => None,
    };
    let series = fetch_series_by_slug(&state.pool, &slug, progress.as_ref()).await?;
    Ok(Json(series))
}

pub(crate) async fn list_films(State(state): State<SharedState>) -> AppResult<Json<Vec<Film>>> {
    Ok(Json(fetch_films(&state.pool, None, None).await?))
}

pub(crate) async fn get_film(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> AppResult<Json<Film>> {
    let maybe_identity = optional_identity(&state.pool, &headers).await?;
    let progress = match maybe_identity {
        Some(identity) => {
            fetch_continue_watching_entry(&state.pool, &identity.user_id, None, &slug).await?
        }
        None => None,
    };
    Ok(Json(
        fetch_film_by_slug(&state.pool, &slug, progress.as_ref()).await?,
    ))
}

pub(crate) async fn get_content(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let maybe_identity = optional_identity(&state.pool, &headers).await?;
    let progress = match maybe_identity {
        Some(identity) => {
            fetch_continue_watching_entry(&state.pool, &identity.user_id, Some(&id), &id).await?
        }
        None => None,
    };
    if let Ok(series) = fetch_series_by_id(&state.pool, &id, progress.as_ref()).await {
        return Ok(Json(serde_json::to_value(series)?));
    }
    if let Ok(film) = fetch_film_by_id(&state.pool, &id, progress.as_ref()).await {
        return Ok(Json(serde_json::to_value(film)?));
    }
    if let Ok(series) = fetch_creator_catalog_series_by_id(&state.pool, &id, false).await {
        return Ok(Json(serde_json::to_value(series)?));
    }
    if let Ok(film) = fetch_creator_catalog_film_by_id(&state.pool, &id, false).await {
        return Ok(Json(serde_json::to_value(film)?));
    }
    let live = fetch_live_stream_by_id(&state.pool, &id).await?;
    Ok(Json(serde_json::to_value(live)?))
}

pub(crate) async fn list_creator_catalog_series(
    State(state): State<SharedState>,
) -> AppResult<Json<Vec<CreatorCatalogSeries>>> {
    Ok(Json(fetch_creator_catalog_series(&state.pool, true).await?))
}

pub(crate) async fn get_creator_catalog_series(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> AppResult<Json<CreatorCatalogSeries>> {
    Ok(Json(
        fetch_creator_catalog_series_by_slug(&state.pool, &slug, false).await?,
    ))
}

pub(crate) async fn list_creator_catalog_films(
    State(state): State<SharedState>,
) -> AppResult<Json<Vec<CreatorCatalogFilm>>> {
    Ok(Json(fetch_creator_catalog_films(&state.pool, true).await?))
}

pub(crate) async fn get_creator_catalog_film(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> AppResult<Json<CreatorCatalogFilm>> {
    Ok(Json(
        fetch_creator_catalog_film_by_slug(&state.pool, &slug, false).await?,
    ))
}

pub(crate) async fn list_categories(
    State(state): State<SharedState>,
) -> AppResult<Json<Vec<Category>>> {
    Ok(Json(fetch_categories(&state.pool).await?))
}

pub(crate) async fn get_category(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> AppResult<Json<Category>> {
    Ok(Json(fetch_category_by_slug(&state.pool, &slug).await?))
}

pub(crate) async fn get_category_browse(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> AppResult<Json<CategoryBrowseResponse>> {
    let category = fetch_category_by_slug(&state.pool, &slug).await?;
    let live_streams = fetch_live_streams_by_category(&state.pool, &category.name).await?;
    let series = fetch_series_by_genre(&state.pool, &category.name).await?;
    let films = fetch_films_by_genre(&state.pool, &category.name).await?;
    let total_vod_titles = (series.len() + films.len()) as i64;

    Ok(Json(CategoryBrowseResponse {
        category,
        live_streams,
        series,
        films,
        total_vod_titles,
    }))
}

pub(crate) async fn list_streamers(
    State(state): State<SharedState>,
) -> AppResult<Json<Vec<Streamer>>> {
    Ok(Json(fetch_streamers(&state.pool).await?))
}

pub(crate) async fn get_streamer(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> AppResult<Json<Streamer>> {
    Ok(Json(fetch_streamer_by_id(&state.pool, &id).await?))
}
