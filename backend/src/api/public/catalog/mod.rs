use super::super::discovery::{
    fetch_categories, fetch_category_by_slug, fetch_continue_watching_entries, fetch_film_by_id,
    fetch_film_by_slug, fetch_films, fetch_films_by_genre, fetch_films_page,
    fetch_live_stream_by_id, fetch_live_stream_by_slug, fetch_live_streams_by_category,
    fetch_series, fetch_series_by_episode_id, fetch_series_by_genre, fetch_series_by_id,
    fetch_series_by_slug, fetch_series_page, fetch_streamer_by_id, fetch_streamers,
};
use super::*;

mod browse;
mod home;
mod repository;
mod search;

pub(crate) use repository::CatalogRepository;
pub(crate) use repository::{
    postgres_fetch_film_by_id, postgres_fetch_live_streams, postgres_fetch_series_by_id,
    postgres_fetch_streamer_by_id,
};

#[cfg(test)]
pub(crate) use browse::CatalogPageQuery;
pub(crate) use browse::{
    get_category, get_category_browse, get_content, get_creator_catalog_film,
    get_creator_catalog_series, get_film, get_series, get_series_for_episode, get_streamer,
    list_categories, list_creator_catalog_films, list_creator_catalog_series, list_films,
    list_films_page, list_series, list_series_page, list_streamers,
};
pub(crate) use home::bootstrap;
pub(super) use home::home;
#[cfg(test)]
pub(crate) use search::SearchQuery;
pub(crate) use search::search;
