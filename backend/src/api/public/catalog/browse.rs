use super::*;

const DEFAULT_CATALOG_PAGE_LIMIT: i64 = 24;
const MAX_CATALOG_PAGE_LIMIT: i64 = 50;

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogPageQuery {
    pub(crate) genre: Option<String>,
    pub(crate) originals_only: Option<bool>,
    pub(crate) sort: Option<String>,
    pub(crate) limit: Option<i64>,
    pub(crate) offset: Option<i64>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogPageResponse<T> {
    items: Vec<T>,
    total: i64,
    limit: i64,
    offset: i64,
    has_more: bool,
}

impl CatalogPageQuery {
    fn normalized(&self) -> NormalizedCatalogPageQuery {
        let genre = self
            .genre
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != "All")
            .map(str::to_string);
        let sort = match self.sort.as_deref().unwrap_or("trending") {
            "newest" => "newest",
            "score" => "score",
            "title" => "title",
            _ => "trending",
        }
        .to_string();
        let limit = self
            .limit
            .unwrap_or(DEFAULT_CATALOG_PAGE_LIMIT)
            .clamp(1, MAX_CATALOG_PAGE_LIMIT);
        let offset = self.offset.unwrap_or(0).max(0);
        NormalizedCatalogPageQuery {
            genre,
            originals_only: self.originals_only.unwrap_or(false),
            sort,
            limit,
            offset,
        }
    }
}

struct NormalizedCatalogPageQuery {
    genre: Option<String>,
    originals_only: bool,
    sort: String,
    limit: i64,
    offset: i64,
}

fn catalog_page_response<T>(
    items: Vec<T>,
    total: i64,
    query: &NormalizedCatalogPageQuery,
) -> CatalogPageResponse<T> {
    CatalogPageResponse {
        has_more: query.offset + (items.len() as i64) < total,
        items,
        total,
        limit: query.limit,
        offset: query.offset,
    }
}

fn sort_series_for_page(items: &mut [Series], sort: &str) {
    match sort {
        "newest" => items.sort_by(|left, right| {
            right
                .year
                .cmp(&left.year)
                .then_with(|| right.score.cmp(&left.score))
        }),
        "score" => items.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| right.year.cmp(&left.year))
        }),
        "title" => {
            items.sort_by(|left, right| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
        }
        _ => items.sort_by(|left, right| {
            right
                .trending
                .cmp(&left.trending)
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| right.year.cmp(&left.year))
        }),
    }
}

fn sort_films_for_page(items: &mut [Film], sort: &str) {
    match sort {
        "newest" => items.sort_by(|left, right| {
            right
                .year
                .cmp(&left.year)
                .then_with(|| right.score.cmp(&left.score))
        }),
        "score" => items.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| right.year.cmp(&left.year))
        }),
        "title" => {
            items.sort_by(|left, right| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
        }
        _ => items.sort_by(|left, right| {
            right
                .trending
                .cmp(&left.trending)
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| right.year.cmp(&left.year))
        }),
    }
}

pub(crate) async fn list_series(State(state): State<SharedState>) -> AppResult<Json<Vec<Series>>> {
    Ok(Json(CatalogRepository::new(&state).list_series().await?))
}

pub(crate) async fn list_series_page(
    State(state): State<SharedState>,
    Query(query): Query<CatalogPageQuery>,
) -> AppResult<Json<CatalogPageResponse<Series>>> {
    let query = query.normalized();
    if state.database_kind == crate::config::DatabaseKind::Postgres {
        let mut items = CatalogRepository::new(&state).list_series().await?;
        if let Some(genre) = query.genre.as_deref() {
            items.retain(|item| item.genres.iter().any(|value| value == genre));
        }
        if query.originals_only {
            items.retain(|item| item.is_original);
        }
        sort_series_for_page(&mut items, &query.sort);
        let total = items.len() as i64;
        let items = items
            .into_iter()
            .skip(query.offset as usize)
            .take(query.limit as usize)
            .collect();
        return Ok(Json(catalog_page_response(items, total, &query)));
    }
    let (items, total) = fetch_series_page(
        state.db.try_sqlite_adapter()?,
        query.genre.as_deref(),
        query.originals_only,
        &query.sort,
        query.limit,
        query.offset,
    )
    .await?;
    Ok(Json(catalog_page_response(items, total, &query)))
}

pub(crate) async fn get_series(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> AppResult<Json<Series>> {
    let maybe_identity = optional_identity(&state.db, &headers).await?;
    Ok(Json(
        CatalogRepository::new(&state)
            .get_series(&slug, maybe_identity)
            .await?,
    ))
}

pub(crate) async fn list_films(State(state): State<SharedState>) -> AppResult<Json<Vec<Film>>> {
    Ok(Json(CatalogRepository::new(&state).list_films().await?))
}

pub(crate) async fn list_films_page(
    State(state): State<SharedState>,
    Query(query): Query<CatalogPageQuery>,
) -> AppResult<Json<CatalogPageResponse<Film>>> {
    let query = query.normalized();
    if state.database_kind == crate::config::DatabaseKind::Postgres {
        let mut items = CatalogRepository::new(&state).list_films().await?;
        if let Some(genre) = query.genre.as_deref() {
            items.retain(|item| item.genres.iter().any(|value| value == genre));
        }
        if query.originals_only {
            items.retain(|item| item.is_original);
        }
        sort_films_for_page(&mut items, &query.sort);
        let total = items.len() as i64;
        let items = items
            .into_iter()
            .skip(query.offset as usize)
            .take(query.limit as usize)
            .collect();
        return Ok(Json(catalog_page_response(items, total, &query)));
    }
    let (items, total) = fetch_films_page(
        state.db.try_sqlite_adapter()?,
        query.genre.as_deref(),
        query.originals_only,
        &query.sort,
        query.limit,
        query.offset,
    )
    .await?;
    Ok(Json(catalog_page_response(items, total, &query)))
}

pub(crate) async fn get_film(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> AppResult<Json<Film>> {
    let maybe_identity = optional_identity(&state.db, &headers).await?;
    Ok(Json(
        CatalogRepository::new(&state)
            .get_film(&slug, maybe_identity)
            .await?,
    ))
}

pub(crate) async fn get_content(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let maybe_identity = optional_identity(&state.db, &headers).await?;
    Ok(Json(
        CatalogRepository::new(&state)
            .get_content(&id, maybe_identity)
            .await?,
    ))
}

pub(crate) async fn get_series_for_episode(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<Series>> {
    let maybe_identity = optional_identity(&state.db, &headers).await?;
    Ok(Json(
        CatalogRepository::new(&state)
            .get_series_for_episode(&id, maybe_identity)
            .await?,
    ))
}

pub(crate) async fn list_creator_catalog_series(
    State(state): State<SharedState>,
) -> AppResult<Json<Vec<CreatorCatalogSeries>>> {
    Ok(Json(
        CatalogRepository::new(&state)
            .list_creator_catalog_series()
            .await?,
    ))
}

pub(crate) async fn get_creator_catalog_series(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> AppResult<Json<CreatorCatalogSeries>> {
    Ok(Json(
        CatalogRepository::new(&state)
            .get_creator_catalog_series(&slug)
            .await?,
    ))
}

pub(crate) async fn list_creator_catalog_films(
    State(state): State<SharedState>,
) -> AppResult<Json<Vec<CreatorCatalogFilm>>> {
    Ok(Json(
        CatalogRepository::new(&state)
            .list_creator_catalog_films()
            .await?,
    ))
}

pub(crate) async fn get_creator_catalog_film(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> AppResult<Json<CreatorCatalogFilm>> {
    Ok(Json(
        CatalogRepository::new(&state)
            .get_creator_catalog_film(&slug)
            .await?,
    ))
}

pub(crate) async fn list_categories(
    State(state): State<SharedState>,
) -> AppResult<Json<Vec<Category>>> {
    Ok(Json(
        CatalogRepository::new(&state).list_categories().await?,
    ))
}

pub(crate) async fn get_category(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> AppResult<Json<Category>> {
    Ok(Json(
        CatalogRepository::new(&state).get_category(&slug).await?,
    ))
}

pub(crate) async fn get_category_browse(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
    Query(query): Query<CatalogPageQuery>,
) -> AppResult<Json<CategoryBrowseResponse>> {
    Ok(Json(
        CatalogRepository::new(&state)
            .get_category_browse(&slug, query.limit, query.offset)
            .await?,
    ))
}

pub(crate) async fn list_streamers(
    State(state): State<SharedState>,
) -> AppResult<Json<Vec<Streamer>>> {
    Ok(Json(CatalogRepository::new(&state).list_streamers().await?))
}

pub(crate) async fn get_streamer(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> AppResult<Json<Streamer>> {
    Ok(Json(
        CatalogRepository::new(&state).get_streamer(&id).await?,
    ))
}
