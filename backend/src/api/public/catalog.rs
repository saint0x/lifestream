use super::super::discovery::{
    fetch_categories, fetch_category_by_slug, fetch_film_by_id, fetch_film_by_slug, fetch_films,
    fetch_films_by_genre, fetch_live_stream_by_id, fetch_live_streams_by_category, fetch_series,
    fetch_series_by_genre, fetch_series_by_id, fetch_series_by_slug, fetch_streamer_by_id,
    fetch_streamers, fetch_user, fetch_viewer_app_state,
};
use super::*;
use serde::Deserialize;

pub(super) async fn home(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<HomeResponse>> {
    let trending_series = fetch_series(&state.pool, Some("WHERE trending = 1"), Some(6)).await?;
    let trending_films = fetch_films(&state.pool, Some("WHERE trending = 1"), Some(6)).await?;
    let featured_live = fetch_live_streams(&state.pool, None).await?;
    let categories = fetch_categories(&state.pool).await?;
    let maybe_identity = optional_identity(&state.pool, &headers).await?;
    let continue_watching = match maybe_identity {
        Some(identity) => {
            fetch_user(&state.pool, &identity.user_id)
                .await?
                .continue_watching
        }
        None => Vec::new(),
    };

    Ok(Json(HomeResponse {
        trending_series,
        trending_films,
        featured_live,
        categories,
        continue_watching,
    }))
}

pub(crate) async fn bootstrap(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<serde_json::Value>> {
    let home = home(State(state.clone()), headers.clone()).await?.0;
    let identity = optional_identity(&state.pool, &headers).await?;
    let me = match identity.as_ref() {
        Some(identity) => Some(fetch_user(&state.pool, &identity.user_id).await?),
        None => None,
    };
    let viewer = match identity.as_ref() {
        Some(identity) => Some(
            fetch_viewer_app_state(&state.pool, &identity.user_id, &identity.session_id).await?,
        ),
        None => None,
    };
    let creator = match identity.as_ref() {
        Some(identity) if identity.creator_id.is_some() => {
            Some(creator_dashboard_payload(&state.pool, identity).await?)
        }
        _ => None,
    };
    let creator_state = match identity.as_ref() {
        Some(identity) if identity.creator_id.is_some() => Some(
            fetch_creator_app_state(
                &state.pool,
                identity,
                &CreatorContentQuery {
                    kind: None,
                    status: None,
                    q: None,
                    sort: None,
                },
            )
            .await?,
        ),
        _ => None,
    };

    Ok(Json(serde_json::json!({
        "home": home,
        "me": me,
        "viewer": viewer,
        "creator": creator,
        "creatorState": creator_state
    })))
}

pub(super) async fn list_series(State(state): State<SharedState>) -> AppResult<Json<Vec<Series>>> {
    Ok(Json(fetch_series(&state.pool, None, None).await?))
}

pub(super) async fn get_series(
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

pub(super) async fn list_films(State(state): State<SharedState>) -> AppResult<Json<Vec<Film>>> {
    Ok(Json(fetch_films(&state.pool, None, None).await?))
}

pub(super) async fn get_film(
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

pub(super) async fn get_content(
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

pub(super) async fn list_creator_catalog_series(
    State(state): State<SharedState>,
) -> AppResult<Json<Vec<CreatorCatalogSeries>>> {
    Ok(Json(fetch_creator_catalog_series(&state.pool, true).await?))
}

pub(super) async fn get_creator_catalog_series(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> AppResult<Json<CreatorCatalogSeries>> {
    Ok(Json(
        fetch_creator_catalog_series_by_slug(&state.pool, &slug, false).await?,
    ))
}

pub(super) async fn list_creator_catalog_films(
    State(state): State<SharedState>,
) -> AppResult<Json<Vec<CreatorCatalogFilm>>> {
    Ok(Json(fetch_creator_catalog_films(&state.pool, true).await?))
}

pub(super) async fn get_creator_catalog_film(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> AppResult<Json<CreatorCatalogFilm>> {
    Ok(Json(
        fetch_creator_catalog_film_by_slug(&state.pool, &slug, false).await?,
    ))
}

pub(super) async fn list_categories(
    State(state): State<SharedState>,
) -> AppResult<Json<Vec<Category>>> {
    Ok(Json(fetch_categories(&state.pool).await?))
}

pub(super) async fn get_category(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> AppResult<Json<Category>> {
    Ok(Json(fetch_category_by_slug(&state.pool, &slug).await?))
}

pub(super) async fn get_category_browse(
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

pub(super) async fn list_streamers(
    State(state): State<SharedState>,
) -> AppResult<Json<Vec<Streamer>>> {
    Ok(Json(fetch_streamers(&state.pool).await?))
}

pub(super) async fn get_streamer(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> AppResult<Json<Streamer>> {
    Ok(Json(fetch_streamer_by_id(&state.pool, &id).await?))
}

#[derive(Deserialize)]
pub(super) struct SearchQuery {
    q: String,
}

pub(super) async fn search(
    State(state): State<SharedState>,
    Query(query): Query<SearchQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let Some(fts_query) = build_fts_query(&query.q) else {
        return Ok(Json(serde_json::json!({
            "series": [],
            "films": [],
            "liveStreams": []
        })));
    };

    let rows = sqlx::query(
        r#"
        SELECT entity_id, kind
        FROM search_documents
        WHERE search_documents MATCH ?
        ORDER BY bm25(search_documents, 1.0, 0.3)
        LIMIT 24
        "#,
    )
    .bind(&fts_query)
    .fetch_all(&state.pool)
    .await?;

    let mut series = Vec::new();
    let mut films = Vec::new();
    let mut live_streams = Vec::new();
    for row in rows {
        let entity_id: String = row.get("entity_id");
        let kind: String = row.get("kind");
        match kind.as_str() {
            "series" => {
                if let Ok(item) = fetch_series_by_id(&state.pool, &entity_id, None).await {
                    series.push(item);
                }
            }
            "film" => {
                if let Ok(item) = fetch_film_by_id(&state.pool, &entity_id, None).await {
                    films.push(item);
                }
            }
            "live" => {
                if let Ok(item) = fetch_live_stream_by_id(&state.pool, &entity_id).await {
                    live_streams.push(item);
                }
            }
            _ => {}
        }
    }

    Ok(Json(serde_json::json!({
        "series": series,
        "films": films,
        "liveStreams": live_streams
    })))
}
