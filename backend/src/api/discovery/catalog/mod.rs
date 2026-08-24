use super::*;

mod films;
mod series;

pub(crate) use films::{
    fetch_film_by_id, fetch_film_by_slug, fetch_films, fetch_films_by_genre, fetch_films_by_ids,
    fetch_films_page,
};
pub(crate) use series::{
    fetch_series, fetch_series_by_episode_id, fetch_series_by_genre, fetch_series_by_id,
    fetch_series_by_slug, fetch_series_page, fetch_series_previews_by_ids,
};
