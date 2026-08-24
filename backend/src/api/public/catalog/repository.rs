use super::*;
use crate::api::dashboard::{fetch_creator_app_state, fetch_creator_dashboard_shell};
use crate::api::discovery::{
    fetch_categories_for_live_streams, fetch_user, fetch_viewer_app_state,
};
use sqlx::Row;

pub(crate) struct CatalogRepository<'a> {
    state: &'a SharedState,
}

impl<'a> CatalogRepository<'a> {
    pub(crate) fn new(state: &'a SharedState) -> Self {
        Self { state }
    }

    fn pool(&self) -> AppResult<&SqlitePool> {
        self.state.db.try_sqlite_adapter()
    }

    fn pg_pool(&self) -> AppResult<&sqlx::PgPool> {
        self.state.db.try_postgres_adapter()
    }

    fn is_postgres(&self) -> bool {
        self.state.database_kind == crate::config::DatabaseKind::Postgres
    }

    pub(crate) async fn list_series(&self) -> AppResult<Vec<Series>> {
        if self.is_postgres() {
            return postgres_fetch_series(self.pg_pool()?, None, None).await;
        }
        fetch_series(self.pool()?, None, None).await
    }

    pub(crate) async fn home_response(&self, headers: &HeaderMap) -> AppResult<HomeResponse> {
        if self.is_postgres() {
            let maybe_identity = optional_identity(&self.state.db, headers).await?;
            let continue_watching = match maybe_identity {
                Some(identity) => {
                    self.state
                        .db
                        .fetch_user(&identity.user_id)
                        .await?
                        .continue_watching
                }
                None => Vec::new(),
            };
            let pool = self.pg_pool()?;
            let (trending_series, trending_films, featured_live) = tokio::try_join!(
                postgres_fetch_series(pool, Some("WHERE trending = 1"), Some(6)),
                postgres_fetch_films(pool, Some("WHERE trending = 1"), Some(6)),
                postgres_fetch_live_streams(pool, None),
            )?;
            let categories =
                postgres_fetch_categories_for_live_streams(pool, &featured_live).await?;
            return Ok(HomeResponse {
                trending_series,
                trending_films,
                featured_live,
                categories,
                continue_watching,
            });
        }
        let pool = self.pool()?;
        let (trending_series, trending_films, featured_live, maybe_identity) = tokio::try_join!(
            fetch_series(pool, Some("WHERE trending = 1"), Some(6)),
            fetch_films(pool, Some("WHERE trending = 1"), Some(6)),
            fetch_live_streams(pool, None),
            optional_identity(&self.state.db, headers),
        )?;
        let categories = fetch_categories_for_live_streams(pool, &featured_live).await?;
        let continue_watching = match maybe_identity {
            Some(identity) => fetch_continue_watching_entries(pool, &identity.user_id).await?,
            None => Vec::new(),
        };

        Ok(HomeResponse {
            trending_series,
            trending_films,
            featured_live,
            categories,
            continue_watching,
        })
    }

    pub(crate) async fn get_series(
        &self,
        slug: &str,
        identity: Option<RequestIdentity>,
    ) -> AppResult<Series> {
        if self.is_postgres() {
            let pool = self.pg_pool()?;
            let user_progress = match identity {
                Some(identity) => {
                    self.state
                        .db
                        .fetch_user(&identity.user_id)
                        .await?
                        .continue_watching
                }
                None => Vec::new(),
            };
            let series = postgres_fetch_series_by_slug(pool, slug, None).await?;
            let progress = user_progress
                .iter()
                .find(|entry| entry.content_id == series.id);
            return postgres_fetch_series_by_slug(pool, slug, progress).await;
        }
        let pool = self.pool()?;
        let progress = match identity {
            Some(identity) => {
                fetch_continue_watching_entry(pool, &identity.user_id, None, slug).await?
            }
            None => None,
        };
        fetch_series_by_slug(pool, slug, progress.as_ref()).await
    }

    pub(crate) async fn list_films(&self) -> AppResult<Vec<Film>> {
        if self.is_postgres() {
            return postgres_fetch_films(self.pg_pool()?, None, None).await;
        }
        fetch_films(self.pool()?, None, None).await
    }

    pub(crate) async fn get_film(
        &self,
        slug: &str,
        identity: Option<RequestIdentity>,
    ) -> AppResult<Film> {
        if self.is_postgres() {
            let pool = self.pg_pool()?;
            let user_progress = match identity {
                Some(identity) => {
                    self.state
                        .db
                        .fetch_user(&identity.user_id)
                        .await?
                        .continue_watching
                }
                None => Vec::new(),
            };
            let film = postgres_fetch_film_by_slug(pool, slug, None).await?;
            let progress = user_progress
                .iter()
                .find(|entry| entry.content_id == film.id);
            return postgres_fetch_film_by_slug(pool, slug, progress).await;
        }
        let pool = self.pool()?;
        let progress = match identity {
            Some(identity) => {
                fetch_continue_watching_entry(pool, &identity.user_id, None, slug).await?
            }
            None => None,
        };
        fetch_film_by_slug(pool, slug, progress.as_ref()).await
    }

    pub(crate) async fn get_content(
        &self,
        id: &str,
        identity: Option<RequestIdentity>,
    ) -> AppResult<serde_json::Value> {
        if self.is_postgres() {
            let progress = match identity {
                Some(identity) => self
                    .state
                    .db
                    .fetch_user(&identity.user_id)
                    .await?
                    .continue_watching
                    .into_iter()
                    .find(|entry| {
                        entry.content_id == id || entry.episode_id.as_deref() == Some(id)
                    }),
                None => None,
            };
            let pool = self.pg_pool()?;
            if let Ok(series) = postgres_fetch_series_by_id(pool, id, progress.as_ref()).await {
                return Ok(serde_json::to_value(series)?);
            }
            if let Ok(film) = postgres_fetch_film_by_id(pool, id, progress.as_ref()).await {
                return Ok(serde_json::to_value(film)?);
            }
            return Ok(serde_json::to_value(
                postgres_fetch_live_stream_by_id(pool, id).await?,
            )?);
        }
        let pool = self.pool()?;
        let progress = match identity {
            Some(identity) => {
                fetch_continue_watching_entry(pool, &identity.user_id, Some(id), id).await?
            }
            None => None,
        };
        if let Ok(series) = fetch_series_by_id(pool, id, progress.as_ref()).await {
            return Ok(serde_json::to_value(series)?);
        }
        if let Ok(film) = fetch_film_by_id(pool, id, progress.as_ref()).await {
            return Ok(serde_json::to_value(film)?);
        }
        if let Ok(series) = fetch_creator_catalog_series_by_id(pool, id, false).await {
            return Ok(serde_json::to_value(series)?);
        }
        if let Ok(film) = fetch_creator_catalog_film_by_id(pool, id, false).await {
            return Ok(serde_json::to_value(film)?);
        }
        Ok(serde_json::to_value(
            fetch_live_stream_by_id(pool, id).await?,
        )?)
    }

    pub(crate) async fn get_series_for_episode(
        &self,
        episode_id: &str,
        identity: Option<RequestIdentity>,
    ) -> AppResult<Series> {
        if self.is_postgres() {
            let pool = self.pg_pool()?;
            let series = postgres_fetch_series_by_episode_id(pool, episode_id, None).await?;
            let progress = match identity {
                Some(identity) => self
                    .state
                    .db
                    .fetch_user(&identity.user_id)
                    .await?
                    .continue_watching
                    .into_iter()
                    .find(|entry| entry.content_id == series.id),
                None => None,
            };
            return postgres_fetch_series_by_episode_id(pool, episode_id, progress.as_ref()).await;
        }
        let pool = self.pool()?;
        let series = fetch_series_by_episode_id(pool, episode_id, None).await?;
        let progress = match identity {
            Some(identity) => {
                fetch_continue_watching_entry(pool, &identity.user_id, Some(&series.id), &series.id)
                    .await?
            }
            None => None,
        };
        fetch_series_by_episode_id(pool, episode_id, progress.as_ref()).await
    }

    pub(crate) async fn list_creator_catalog_series(&self) -> AppResult<Vec<CreatorCatalogSeries>> {
        if self.is_postgres() {
            return Ok(Vec::new());
        }
        fetch_creator_catalog_series(self.pool()?, true).await
    }

    pub(crate) async fn get_creator_catalog_series(
        &self,
        slug: &str,
    ) -> AppResult<CreatorCatalogSeries> {
        if self.is_postgres() {
            return Err(AppError::NotFound);
        }
        fetch_creator_catalog_series_by_slug(self.pool()?, slug, false).await
    }

    pub(crate) async fn list_creator_catalog_films(&self) -> AppResult<Vec<CreatorCatalogFilm>> {
        if self.is_postgres() {
            return Ok(Vec::new());
        }
        fetch_creator_catalog_films(self.pool()?, true).await
    }

    pub(crate) async fn get_creator_catalog_film(
        &self,
        slug: &str,
    ) -> AppResult<CreatorCatalogFilm> {
        if self.is_postgres() {
            return Err(AppError::NotFound);
        }
        fetch_creator_catalog_film_by_slug(self.pool()?, slug, false).await
    }

    pub(crate) async fn list_categories(&self) -> AppResult<Vec<Category>> {
        if self.is_postgres() {
            return postgres_fetch_categories(self.pg_pool()?).await;
        }
        fetch_categories(self.pool()?).await
    }

    pub(crate) async fn get_category(&self, slug: &str) -> AppResult<Category> {
        if self.is_postgres() {
            return postgres_fetch_category_by_slug(self.pg_pool()?, slug).await;
        }
        fetch_category_by_slug(self.pool()?, slug).await
    }

    pub(crate) async fn get_category_browse(
        &self,
        slug: &str,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> AppResult<CategoryBrowseResponse> {
        if self.is_postgres() {
            let pool = self.pg_pool()?;
            let limit = limit.unwrap_or(24).clamp(1, 50) as usize;
            let offset = offset.unwrap_or(0).max(0) as usize;
            let category = postgres_fetch_category_by_slug(pool, slug).await?;
            let live_streams =
                postgres_fetch_live_streams_by_category(pool, &category.name).await?;
            let all_series = postgres_fetch_series_by_genre(pool, &category.name).await?;
            let all_films = postgres_fetch_films_by_genre(pool, &category.name).await?;
            let series_count = all_series.len();
            let total_vod_titles = (series_count + all_films.len()) as i64;
            let series = all_series
                .into_iter()
                .skip(offset)
                .take(limit)
                .collect::<Vec<_>>();
            let remaining_limit = limit.saturating_sub(series.len());
            let film_offset = offset.saturating_sub(series_count);
            let films = all_films
                .into_iter()
                .skip(film_offset)
                .take(remaining_limit)
                .collect::<Vec<_>>();

            return Ok(CategoryBrowseResponse {
                category,
                live_streams,
                series,
                films,
                total_vod_titles,
            });
        }
        let pool = self.pool()?;
        let limit = limit.unwrap_or(24).clamp(1, 50) as usize;
        let offset = offset.unwrap_or(0).max(0) as usize;
        let category = fetch_category_by_slug(pool, slug).await?;
        let live_streams = fetch_live_streams_by_category(pool, &category.name).await?;
        let all_series = fetch_series_by_genre(pool, &category.name).await?;
        let all_films = fetch_films_by_genre(pool, &category.name).await?;
        let series_count = all_series.len();
        let total_vod_titles = (series_count + all_films.len()) as i64;
        let series = all_series
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect::<Vec<_>>();
        let remaining_limit = limit.saturating_sub(series.len());
        let film_offset = offset.saturating_sub(series_count);
        let films = all_films
            .into_iter()
            .skip(film_offset)
            .take(remaining_limit)
            .collect::<Vec<_>>();

        Ok(CategoryBrowseResponse {
            category,
            live_streams,
            series,
            films,
            total_vod_titles,
        })
    }

    pub(crate) async fn list_streamers(&self) -> AppResult<Vec<Streamer>> {
        if self.is_postgres() {
            return postgres_fetch_streamers(self.pg_pool()?).await;
        }
        fetch_streamers(self.pool()?).await
    }

    pub(crate) async fn get_streamer(&self, id: &str) -> AppResult<Streamer> {
        if self.is_postgres() {
            return postgres_fetch_streamer_by_id(self.pg_pool()?, id).await;
        }
        fetch_streamer_by_id(self.pool()?, id).await
    }

    pub(crate) async fn list_live_streams(&self) -> AppResult<Vec<LiveStream>> {
        if self.is_postgres() {
            return postgres_fetch_live_streams(self.pg_pool()?, None).await;
        }
        fetch_live_streams(self.pool()?, None).await
    }

    pub(crate) async fn get_live_stream(&self, slug: &str) -> AppResult<LiveStream> {
        if self.is_postgres() {
            return postgres_fetch_live_streams(self.pg_pool()?, Some(slug))
                .await?
                .into_iter()
                .next()
                .ok_or(AppError::NotFound);
        }
        fetch_live_stream_by_slug(self.pool()?, slug).await
    }

    pub(crate) async fn search_page(
        &self,
        query: &str,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> AppResult<serde_json::Value> {
        let limit = limit.unwrap_or(24).clamp(1, 50);
        let offset = offset.unwrap_or(0).max(0);
        let hits = self
            .state
            .db
            .search_catalog_documents(query, limit + 1, offset)
            .await?;
        if hits.is_empty() {
            return Ok(serde_json::json!({
                "items": [],
                "series": [],
                "films": [],
                "liveStreams": [],
                "total": 0,
                "limit": limit,
                "offset": offset,
                "hasMore": false
            }));
        }
        let total = hits.first().map(|hit| hit.total_count).unwrap_or(0);
        let has_more = hits.len() as i64 > limit;
        let page_hits = hits.into_iter().take(limit as usize).collect::<Vec<_>>();
        let items = page_hits
            .iter()
            .map(|hit| {
                let metadata = serde_json::from_str::<serde_json::Value>(&hit.metadata_json)
                    .unwrap_or_else(|_| serde_json::json!({}));
                serde_json::json!({
                    "id": hit.entity_id,
                    "kind": hit.kind,
                    "slug": hit.slug,
                    "title": hit.title,
                    "subtitle": hit.subtitle,
                    "image": hit.image,
                    "href": hit.href,
                    "metadata": metadata,
                    "score": hit.score
                })
            })
            .collect::<Vec<_>>();
        if self.is_postgres() {
            let pool = self.pg_pool()?;
            let mut series = Vec::new();
            let mut films = Vec::new();
            let mut live_streams = Vec::new();
            for hit in &page_hits {
                match hit.kind.as_str() {
                    "series" => {
                        if let Ok(item) =
                            postgres_fetch_series_by_id(pool, &hit.entity_id, None).await
                        {
                            series.push(item);
                        }
                    }
                    "film" => {
                        if let Ok(item) =
                            postgres_fetch_film_by_id(pool, &hit.entity_id, None).await
                        {
                            films.push(item);
                        }
                    }
                    "live" => {
                        if let Ok(item) =
                            postgres_fetch_live_stream_by_id(pool, &hit.entity_id).await
                        {
                            live_streams.push(item);
                        }
                    }
                    _ => {}
                }
            }
            return Ok(serde_json::json!({
                "items": items,
                "series": series,
                "films": films,
                "liveStreams": live_streams,
                "total": total,
                "limit": limit,
                "offset": offset,
                "hasMore": has_more
            }));
        }
        let pool = self.pool()?;

        let mut series = Vec::new();
        let mut films = Vec::new();
        let mut live_streams = Vec::new();
        for hit in &page_hits {
            match hit.kind.as_str() {
                "series" => {
                    if let Ok(item) = fetch_series_by_id(pool, &hit.entity_id, None).await {
                        series.push(item);
                    }
                }
                "film" => {
                    if let Ok(item) = fetch_film_by_id(pool, &hit.entity_id, None).await {
                        films.push(item);
                    }
                }
                "live" => {
                    if let Ok(item) = fetch_live_stream_by_id(pool, &hit.entity_id).await {
                        live_streams.push(item);
                    }
                }
                _ => {}
            }
        }

        Ok(serde_json::json!({
            "items": items,
            "series": series,
            "films": films,
            "liveStreams": live_streams,
            "total": total,
            "limit": limit,
            "offset": offset,
            "hasMore": has_more
        }))
    }

    pub(crate) async fn user(&self, user_id: &str) -> AppResult<User> {
        if self.is_postgres() {
            return self.state.db.fetch_user(user_id).await;
        }
        fetch_user(self.pool()?, user_id).await
    }

    pub(crate) async fn viewer_app_state(
        &self,
        user_id: &str,
        session_id: &str,
    ) -> AppResult<ViewerAppState> {
        if self.is_postgres() {
            return Err(AppError::NotFound);
        }
        fetch_viewer_app_state(&self.state.db, self.pool()?, user_id, session_id).await
    }

    pub(crate) async fn creator_dashboard_shell(
        &self,
        creator_id: &str,
    ) -> AppResult<CreatorDashboard> {
        if self.is_postgres() {
            return Err(AppError::NotFound);
        }
        fetch_creator_dashboard_shell(self.pool()?, creator_id).await
    }

    pub(crate) async fn creator_app_state(
        &self,
        identity: &RequestIdentity,
    ) -> AppResult<CreatorAppState> {
        fetch_creator_app_state(
            self.state,
            identity,
            &CreatorContentQuery {
                kind: None,
                status: None,
                q: None,
                sort: None,
            },
        )
        .await
    }
}

const POSTGRES_SERIES_SELECT: &str = r#"
    SELECT id, slug, title, tagline, synopsis, year::BIGINT AS year, rating,
           genres_json, images_json, credits_json, score::BIGINT AS score,
           is_original::BIGINT AS is_original, trending::BIGINT AS trending,
           hero_color, status, total_episodes::BIGINT AS total_episodes
    FROM series
"#;

const POSTGRES_FILM_SELECT: &str = r#"
    SELECT id, slug, title, tagline, synopsis, year::BIGINT AS year, rating,
           genres_json, images_json, credits_json, score::BIGINT AS score,
           is_original::BIGINT AS is_original, trending::BIGINT AS trending,
           hero_color, duration_sec::BIGINT AS duration_sec,
           CASE WHEN EXISTS (
                SELECT 1
                FROM media_assets ma
                WHERE ma.upload_id = films.id
                  AND ma.status IN ('ready', 'published')
           ) THEN 1 ELSE 0 END::BIGINT AS playback_ready
    FROM films
"#;

async fn postgres_fetch_series(
    pool: &sqlx::PgPool,
    extra_where: Option<&str>,
    limit: Option<i64>,
) -> AppResult<Vec<Series>> {
    let mut query = String::from(POSTGRES_SERIES_SELECT);
    if let Some(extra) = extra_where {
        query.push(' ');
        query.push_str(extra);
    }
    query.push_str(" ORDER BY score DESC, year DESC");
    if let Some(limit) = limit {
        query.push_str(&format!(" LIMIT {limit}"));
    }

    let rows = sqlx::query(&query).fetch_all(pool).await?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(postgres_series_from_row(pool, row, None, false).await?);
    }
    Ok(items)
}

async fn postgres_fetch_series_by_slug(
    pool: &sqlx::PgPool,
    slug: &str,
    progress: Option<&ContinueWatchingEntry>,
) -> AppResult<Series> {
    let row = sqlx::query(&(POSTGRES_SERIES_SELECT.to_string() + " WHERE slug = $1"))
        .bind(slug)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    postgres_series_from_row(pool, row, progress, true).await
}

pub(crate) async fn postgres_fetch_series_by_id(
    pool: &sqlx::PgPool,
    id: &str,
    progress: Option<&ContinueWatchingEntry>,
) -> AppResult<Series> {
    let row = sqlx::query(&(POSTGRES_SERIES_SELECT.to_string() + " WHERE id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    postgres_series_from_row(pool, row, progress, true).await
}

async fn postgres_fetch_series_by_episode_id(
    pool: &sqlx::PgPool,
    episode_id: &str,
    progress: Option<&ContinueWatchingEntry>,
) -> AppResult<Series> {
    let row = sqlx::query(
        &(POSTGRES_SERIES_SELECT.to_string()
            + " WHERE id = (SELECT series_id FROM episodes WHERE id = $1)"),
    )
    .bind(episode_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    postgres_series_from_row(pool, row, progress, true).await
}

async fn postgres_fetch_series_by_genre(
    pool: &sqlx::PgPool,
    genre: &str,
) -> AppResult<Vec<Series>> {
    let rows = sqlx::query(
        &(POSTGRES_SERIES_SELECT.to_string()
            + " WHERE genres_json::jsonb ? $1 ORDER BY score DESC, year DESC"),
    )
    .bind(genre)
    .fetch_all(pool)
    .await?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(postgres_series_from_row(pool, row, None, false).await?);
    }
    Ok(items)
}

async fn postgres_series_from_row(
    pool: &sqlx::PgPool,
    row: sqlx::postgres::PgRow,
    progress: Option<&ContinueWatchingEntry>,
    include_episodes: bool,
) -> AppResult<Series> {
    let series_id: String = row.get("id");
    Ok(Series {
        id: series_id.clone(),
        slug: row.get("slug"),
        kind: "series".to_string(),
        title: row.get("title"),
        tagline: row.get("tagline"),
        synopsis: row.get("synopsis"),
        year: row.get("year"),
        rating: row.get("rating"),
        genres: from_json(row.get::<String, _>("genres_json"))?,
        images: from_json(row.get::<String, _>("images_json"))?,
        credits: from_json(row.get::<String, _>("credits_json"))?,
        score: row.get("score"),
        is_original: row.get::<i64, _>("is_original") == 1,
        trending: row.get::<i64, _>("trending") == 1,
        hero_color: row.get("hero_color"),
        seasons: if include_episodes {
            postgres_fetch_seasons(pool, &series_id, progress).await?
        } else {
            postgres_fetch_season_previews(pool, &series_id).await?
        },
        total_episodes: row.get("total_episodes"),
        status: row.get("status"),
    })
}

async fn postgres_fetch_seasons(
    pool: &sqlx::PgPool,
    series_id: &str,
    progress: Option<&ContinueWatchingEntry>,
) -> AppResult<Vec<Season>> {
    let season_rows = sqlx::query(
        "SELECT season_number::BIGINT AS season_number, title FROM seasons WHERE series_id = $1 ORDER BY season_number ASC",
    )
    .bind(series_id)
    .fetch_all(pool)
    .await?;

    let mut seasons = Vec::with_capacity(season_rows.len());
    for season_row in season_rows {
        let season_number: i64 = season_row.get("season_number");
        let episode_rows = sqlx::query(
            r#"
            SELECT id, series_id, season_number::BIGINT AS season_number,
                   episode_number::BIGINT AS episode_number, title, synopsis,
                   duration_sec::BIGINT AS duration_sec, aired_at, thumbnail,
                   CASE WHEN EXISTS (
                        SELECT 1
                        FROM media_assets ma
                        WHERE ma.upload_id = episodes.id
                          AND ma.status IN ('ready', 'published')
                   ) THEN 1 ELSE 0 END::BIGINT AS playback_ready
            FROM episodes
            WHERE series_id = $1 AND season_number = $2
            ORDER BY episode_number ASC
            "#,
        )
        .bind(series_id)
        .bind(season_number)
        .fetch_all(pool)
        .await?;

        let episodes = episode_rows
            .into_iter()
            .map(|episode_row| {
                let episode_id: String = episode_row.get("id");
                let playback_ready = episode_row.get::<i64, _>("playback_ready") == 1;
                Episode {
                    id: episode_id.clone(),
                    series_id: episode_row.get("series_id"),
                    season_number: episode_row.get("season_number"),
                    episode_number: episode_row.get("episode_number"),
                    title: episode_row.get("title"),
                    synopsis: episode_row.get("synopsis"),
                    duration_sec: episode_row.get("duration_sec"),
                    aired_at: episode_row.get("aired_at"),
                    thumbnail: episode_row.get("thumbnail"),
                    progress_sec: progress.and_then(|entry| {
                        (entry.episode_id.as_deref() == Some(episode_id.as_str()))
                            .then_some(entry.progress_sec)
                    }),
                    playback_session_url: playback_ready
                        .then(|| playback_content_session_api_url(&episode_id)),
                    playback_ready,
                }
            })
            .collect();

        seasons.push(Season {
            season_number,
            title: season_row.get("title"),
            episodes,
        });
    }
    Ok(seasons)
}

async fn postgres_fetch_season_previews(
    pool: &sqlx::PgPool,
    series_id: &str,
) -> AppResult<Vec<Season>> {
    Ok(sqlx::query(
        "SELECT season_number::BIGINT AS season_number, title FROM seasons WHERE series_id = $1 ORDER BY season_number ASC",
    )
    .bind(series_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|season_row| Season {
        season_number: season_row.get("season_number"),
        title: season_row.get("title"),
        episodes: Vec::new(),
    })
    .collect())
}

async fn postgres_fetch_films(
    pool: &sqlx::PgPool,
    extra_where: Option<&str>,
    limit: Option<i64>,
) -> AppResult<Vec<Film>> {
    let mut query = String::from(POSTGRES_FILM_SELECT);
    if let Some(extra) = extra_where {
        query.push(' ');
        query.push_str(extra);
    }
    query.push_str(" ORDER BY score DESC, year DESC");
    if let Some(limit) = limit {
        query.push_str(&format!(" LIMIT {limit}"));
    }

    Ok(sqlx::query(&query)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| postgres_film_from_row(row, None))
        .collect::<AppResult<Vec<_>>>()?)
}

async fn postgres_fetch_film_by_slug(
    pool: &sqlx::PgPool,
    slug: &str,
    progress: Option<&ContinueWatchingEntry>,
) -> AppResult<Film> {
    let row = sqlx::query(&(POSTGRES_FILM_SELECT.to_string() + " WHERE slug = $1"))
        .bind(slug)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    postgres_film_from_row(row, progress)
}

pub(crate) async fn postgres_fetch_film_by_id(
    pool: &sqlx::PgPool,
    id: &str,
    progress: Option<&ContinueWatchingEntry>,
) -> AppResult<Film> {
    let row = sqlx::query(&(POSTGRES_FILM_SELECT.to_string() + " WHERE id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    postgres_film_from_row(row, progress)
}

async fn postgres_fetch_films_by_genre(pool: &sqlx::PgPool, genre: &str) -> AppResult<Vec<Film>> {
    Ok(sqlx::query(
        &(POSTGRES_FILM_SELECT.to_string()
            + " WHERE genres_json::jsonb ? $1 ORDER BY score DESC, year DESC"),
    )
    .bind(genre)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| postgres_film_from_row(row, None))
    .collect::<AppResult<Vec<_>>>()?)
}

fn postgres_film_from_row(
    row: sqlx::postgres::PgRow,
    progress: Option<&ContinueWatchingEntry>,
) -> AppResult<Film> {
    let film_id: String = row.get("id");
    let playback_ready = row.get::<i64, _>("playback_ready") == 1;
    Ok(Film {
        id: film_id.clone(),
        slug: row.get("slug"),
        kind: "film".to_string(),
        title: row.get("title"),
        tagline: row.get("tagline"),
        synopsis: row.get("synopsis"),
        year: row.get("year"),
        rating: row.get("rating"),
        genres: from_json(row.get::<String, _>("genres_json"))?,
        images: from_json(row.get::<String, _>("images_json"))?,
        credits: from_json(row.get::<String, _>("credits_json"))?,
        score: row.get("score"),
        is_original: row.get::<i64, _>("is_original") == 1,
        trending: row.get::<i64, _>("trending") == 1,
        hero_color: row.get("hero_color"),
        duration_sec: row.get("duration_sec"),
        progress_sec: progress.map(|entry| entry.progress_sec),
        playback_session_url: playback_ready.then(|| playback_content_session_api_url(&film_id)),
        playback_ready,
    })
}

pub(crate) async fn postgres_fetch_live_streams(
    pool: &sqlx::PgPool,
    filter_slug: Option<&str>,
) -> AppResult<Vec<LiveStream>> {
    let mut query = String::from(
        r#"
        SELECT ls.id, ls.slug, ls.title, ls.category, ls.tags_json,
               ls.viewers::BIGINT AS viewers, ls.started_at, ls.thumbnail, ls.language,
               ls.is_mature::BIGINT AS is_mature,
               CASE WHEN ls.playback_asset_id IS NOT NULL AND ls.playback_relative_path IS NOT NULL
                    THEN 1 ELSE 0 END::BIGINT AS playback_ready,
               s.id AS streamer_id, s.handle, s.display_name, s.avatar, s.bio,
               s.followers::BIGINT AS followers, s.is_partner::BIGINT AS is_partner,
               s.is_live::BIGINT AS is_live
        FROM live_streams ls
        JOIN streamers s ON s.id = ls.streamer_id
        "#,
    );
    if filter_slug.is_some() {
        query.push_str(" WHERE ls.slug = $1");
    }
    query.push_str(" ORDER BY ls.viewers DESC, ls.started_at DESC");

    let mut statement = sqlx::query(&query);
    if let Some(slug) = filter_slug {
        statement = statement.bind(slug);
    }
    Ok(statement
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(postgres_live_stream_from_row)
        .collect())
}

async fn postgres_fetch_live_stream_by_id(pool: &sqlx::PgPool, id: &str) -> AppResult<LiveStream> {
    let row = sqlx::query(
        r#"
        SELECT ls.id, ls.slug, ls.title, ls.category, ls.tags_json,
               ls.viewers::BIGINT AS viewers, ls.started_at, ls.thumbnail, ls.language,
               ls.is_mature::BIGINT AS is_mature,
               CASE WHEN ls.playback_asset_id IS NOT NULL AND ls.playback_relative_path IS NOT NULL
                    THEN 1 ELSE 0 END::BIGINT AS playback_ready,
               s.id AS streamer_id, s.handle, s.display_name, s.avatar, s.bio,
               s.followers::BIGINT AS followers, s.is_partner::BIGINT AS is_partner,
               s.is_live::BIGINT AS is_live
        FROM live_streams ls
        JOIN streamers s ON s.id = ls.streamer_id
        WHERE ls.id = $1 OR ls.slug = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(postgres_live_stream_from_row(row))
}

async fn postgres_fetch_live_streams_by_category(
    pool: &sqlx::PgPool,
    category: &str,
) -> AppResult<Vec<LiveStream>> {
    let mut streams = postgres_fetch_live_streams(pool, None).await?;
    streams.retain(|stream| stream.category == category);
    Ok(streams)
}

fn postgres_live_stream_from_row(row: sqlx::postgres::PgRow) -> LiveStream {
    let playback_ready = row.get::<i64, _>("playback_ready") == 1;
    let stream_id: String = row.get("id");
    LiveStream {
        id: stream_id.clone(),
        slug: row.get("slug"),
        title: row.get("title"),
        category: row.get("category"),
        tags: from_json(row.get::<String, _>("tags_json")).unwrap_or_default(),
        streamer: Streamer {
            id: row.get("streamer_id"),
            handle: row.get("handle"),
            display_name: row.get("display_name"),
            avatar: row.get("avatar"),
            bio: row.get("bio"),
            followers: row.get("followers"),
            is_partner: row.get::<i64, _>("is_partner") == 1,
            is_live: row.get::<i64, _>("is_live") == 1,
        },
        viewers: row.get("viewers"),
        started_at: row.get("started_at"),
        thumbnail: row.get("thumbnail"),
        language: row.get("language"),
        is_mature: row.get::<i64, _>("is_mature") == 1,
        kind: "live".to_string(),
        playback_session_url: playback_ready.then(|| playback_live_session_api_url(&stream_id)),
        playback_ready,
    }
}

async fn postgres_fetch_categories(pool: &sqlx::PgPool) -> AppResult<Vec<Category>> {
    let categories = sqlx::query(
        r#"
        SELECT slug, name, cover_image, live_viewers::BIGINT AS live_viewers,
               live_channels::BIGINT AS live_channels, tags_json
        FROM categories
        ORDER BY live_viewers DESC, name ASC
        "#,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(postgres_category_from_row)
    .collect::<Vec<_>>();
    let live_streams = postgres_fetch_live_streams(pool, None).await?;
    apply_postgres_category_live_totals(categories, &live_streams)
}

async fn postgres_fetch_categories_for_live_streams(
    pool: &sqlx::PgPool,
    live_streams: &[LiveStream],
) -> AppResult<Vec<Category>> {
    let categories = sqlx::query(
        r#"
        SELECT slug, name, cover_image, live_viewers::BIGINT AS live_viewers,
               live_channels::BIGINT AS live_channels, tags_json
        FROM categories
        ORDER BY live_viewers DESC, name ASC
        "#,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(postgres_category_from_row)
    .collect::<Vec<_>>();
    apply_postgres_category_live_totals(categories, live_streams)
}

async fn postgres_fetch_category_by_slug(pool: &sqlx::PgPool, slug: &str) -> AppResult<Category> {
    let row = sqlx::query(
        r#"
        SELECT slug, name, cover_image, live_viewers::BIGINT AS live_viewers,
               live_channels::BIGINT AS live_channels, tags_json
        FROM categories
        WHERE slug = $1
        "#,
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let mut category = postgres_category_from_row(row);
    let streams = postgres_fetch_live_streams_by_category(pool, &category.name).await?;
    category.live_viewers = streams.iter().map(|stream| stream.viewers).sum();
    category.live_channels = streams.len() as i64;
    Ok(category)
}

fn postgres_category_from_row(row: sqlx::postgres::PgRow) -> Category {
    Category {
        slug: row.get("slug"),
        name: row.get("name"),
        cover_image: row.get("cover_image"),
        live_viewers: row.get("live_viewers"),
        live_channels: row.get("live_channels"),
        tags: from_json(row.get::<String, _>("tags_json")).unwrap_or_default(),
    }
}

fn apply_postgres_category_live_totals(
    mut categories: Vec<Category>,
    live_streams: &[LiveStream],
) -> AppResult<Vec<Category>> {
    let mut totals_by_category: std::collections::HashMap<String, (i64, i64)> =
        std::collections::HashMap::new();
    for stream in live_streams {
        let entry = totals_by_category
            .entry(stream.category.clone())
            .or_insert((0, 0));
        entry.0 += stream.viewers;
        entry.1 += 1;
    }

    for category in &mut categories {
        let (live_viewers, live_channels) = totals_by_category
            .get(&category.name)
            .copied()
            .unwrap_or((0, 0));
        category.live_viewers = live_viewers;
        category.live_channels = live_channels;
    }

    categories.sort_by(|left, right| {
        right
            .live_viewers
            .cmp(&left.live_viewers)
            .then_with(|| right.live_channels.cmp(&left.live_channels))
            .then_with(|| left.name.cmp(&right.name))
    });

    Ok(categories)
}

async fn postgres_fetch_streamers(pool: &sqlx::PgPool) -> AppResult<Vec<Streamer>> {
    Ok(sqlx::query(
        r#"
        SELECT id, handle, display_name, avatar, bio, followers::BIGINT AS followers,
               is_partner::BIGINT AS is_partner, is_live::BIGINT AS is_live
        FROM streamers
        ORDER BY followers DESC
        "#,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(postgres_streamer_from_row)
    .collect())
}

pub(crate) async fn postgres_fetch_streamer_by_id(
    pool: &sqlx::PgPool,
    id: &str,
) -> AppResult<Streamer> {
    let row = sqlx::query(
        r#"
        SELECT id, handle, display_name, avatar, bio, followers::BIGINT AS followers,
               is_partner::BIGINT AS is_partner, is_live::BIGINT AS is_live
        FROM streamers
        WHERE id = $1 OR handle = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(postgres_streamer_from_row(row))
}

fn postgres_streamer_from_row(row: sqlx::postgres::PgRow) -> Streamer {
    Streamer {
        id: row.get("id"),
        handle: row.get("handle"),
        display_name: row.get("display_name"),
        avatar: row.get("avatar"),
        bio: row.get("bio"),
        followers: row.get("followers"),
        is_partner: row.get::<i64, _>("is_partner") == 1,
        is_live: row.get::<i64, _>("is_live") == 1,
    }
}
