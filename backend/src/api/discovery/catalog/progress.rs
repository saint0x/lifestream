use super::*;

pub(crate) async fn validate_watchlist_content(
    pool: &SqlitePool,
    content_id: &str,
) -> AppResult<()> {
    if fetch_series_by_id(pool, content_id, None).await.is_ok()
        || fetch_film_by_id(pool, content_id, None).await.is_ok()
    {
        return Ok(());
    }

    if fetch_live_stream_by_id(pool, content_id).await.is_ok() {
        return Err(AppError::BadRequest(
            "watchlist only supports series and films".to_string(),
        ));
    }

    Err(AppError::NotFound)
}

pub(crate) struct ProgressTarget {
    pub(crate) kind: String,
    pub(crate) episode_id: Option<String>,
    pub(crate) duration_sec: i64,
}

pub(crate) async fn resolve_progress_target(
    pool: &SqlitePool,
    input: &ProgressInput,
) -> AppResult<ProgressTarget> {
    match input.kind.as_str() {
        "film" => {
            if input.episode_id.is_some() {
                return Err(AppError::BadRequest(
                    "film progress cannot include an episodeId".to_string(),
                ));
            }

            let film = fetch_film_by_id(pool, &input.content_id, None).await?;
            Ok(ProgressTarget {
                kind: "film".to_string(),
                episode_id: None,
                duration_sec: film.duration_sec,
            })
        }
        "series" => {
            let episode_id = input.episode_id.clone().ok_or_else(|| {
                AppError::BadRequest("series progress requires an episodeId".to_string())
            })?;
            fetch_series_by_id(pool, &input.content_id, None).await?;
            let episode = fetch_episode_by_id(pool, &episode_id).await?;
            if episode.series_id != input.content_id {
                return Err(AppError::BadRequest(
                    "episodeId does not belong to the requested series".to_string(),
                ));
            }

            Ok(ProgressTarget {
                kind: "series".to_string(),
                episode_id: Some(episode_id),
                duration_sec: episode.duration_sec,
            })
        }
        _ => Err(AppError::BadRequest(
            "kind must be either 'film' or 'series'".to_string(),
        )),
    }
}
