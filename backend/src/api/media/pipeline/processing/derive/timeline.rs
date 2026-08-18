use super::*;

pub(super) async fn generate_timeline_preview(
    state: &SharedState,
    creator_id: &str,
    job_id: &str,
    attempt: &MediaProcessingAttempt,
    probed: &ProbedMedia,
    processed_root: &str,
) -> Result<Option<NewMediaPreviewTrack>, (AppError, String)> {
    if !probed.has_video {
        return Ok(None);
    }

    let image_relative_path = format!("{processed_root}/images/timeline_preview.jpg");
    let vtt_relative_path = format!("{processed_root}/images/timeline_preview.vtt");
    let image_full_path = media_path_for_relative(state, &image_relative_path);
    let vtt_full_path = media_path_for_relative(state, &vtt_relative_path);
    ensure_parent_dir(&image_full_path)
        .await
        .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    ensure_parent_dir(&vtt_full_path)
        .await
        .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    let preview_run_id = start_media_processing_run(
        &state.pool,
        creator_id,
        job_id,
        &attempt.asset.id,
        "timeline_preview",
        json!({
            "imageTarget": image_relative_path,
            "vttTarget": vtt_relative_path,
        }),
    )
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    match generate_timeline_preview_track(
        &attempt.source_path,
        &image_full_path,
        &vtt_full_path,
        &image_relative_path,
        &vtt_relative_path,
        probed.duration_sec,
        probed.width.unwrap_or(320),
        probed.height.unwrap_or(180),
    )
    .await
    {
        Ok(track) => {
            finish_media_processing_run(
                &state.pool,
                &preview_run_id,
                "completed",
                json!({
                    "label": track.label,
                    "imageTarget": track.image_relative_path,
                    "vttTarget": track.vtt_relative_path,
                    "tileWidth": track.tile_width,
                    "tileHeight": track.tile_height,
                    "columnsCount": track.columns_count,
                    "rowsCount": track.rows_count,
                    "intervalSec": track.interval_sec,
                    "frameCount": track.frame_count,
                }),
            )
            .await
            .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
            Ok(Some(track))
        }
        Err(error) => {
            let _ = finish_media_processing_run(
                &state.pool,
                &preview_run_id,
                "failed",
                json!({
                    "imageTarget": image_relative_path,
                    "vttTarget": vtt_relative_path,
                    "error": error.to_string(),
                }),
            )
            .await;
            Err((error, attempt.lease_updated_at.clone()))
        }
    }
}
