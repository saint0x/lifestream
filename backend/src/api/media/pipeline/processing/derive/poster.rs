use super::*;

pub(super) async fn generate_poster_derivative(
    state: &SharedState,
    creator_id: &str,
    job_id: &str,
    attempt: &MediaProcessingAttempt,
    probed: &ProbedMedia,
    processed_root: &str,
) -> Result<Option<String>, (AppError, String)> {
    if !probed.has_video {
        return Ok(None);
    }

    let poster_relative_path = format!("{processed_root}/poster.jpg");
    let poster_full_path = media_path_for_relative(state, &poster_relative_path);
    ensure_parent_dir(&poster_full_path)
        .await
        .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    let poster_run_id = start_media_processing_run(
        &state.pool,
        creator_id,
        job_id,
        &attempt.asset.id,
        "poster",
        json!({ "target": poster_relative_path }),
    )
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    match generate_poster(&attempt.source_path, &poster_full_path, probed.duration_sec).await {
        Ok(()) => {
            finish_media_processing_run(
                &state.pool,
                &poster_run_id,
                "completed",
                json!({ "target": poster_relative_path }),
            )
            .await
            .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
            Ok(Some(poster_relative_path))
        }
        Err(error) => {
            let _ = finish_media_processing_run(
                &state.pool,
                &poster_run_id,
                "failed",
                json!({ "target": poster_relative_path, "error": error.to_string() }),
            )
            .await;
            Err((error, attempt.lease_updated_at.clone()))
        }
    }
}
