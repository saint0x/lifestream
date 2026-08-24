use super::*;

pub(crate) async fn run_probe_stage(
    state: &SharedState,
    creator_id: &str,
    job_id: &str,
    attempt: &MediaProcessingAttempt,
) -> Result<ProbedMedia, (AppError, String)> {
    let probe_run_id = start_media_processing_run(
        state.db.sqlite_adapter(),
        creator_id,
        job_id,
        &attempt.asset.id,
        "probe",
        json!({}),
    )
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))?;

    match probe_media(&attempt.source_path).await {
        Ok(probed) => {
            validate_probed_media(&attempt.job, &probed)
                .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
            finish_media_processing_run(
                state.db.sqlite_adapter(),
                &probe_run_id,
                "completed",
                json!({
                    "durationSec": probed.duration_sec,
                    "width": probed.width,
                    "height": probed.height,
                    "videoCodec": probed.video_codec,
                    "audioCodec": probed.audio_codec,
                    "audioSampleRateHz": probed.audio_sample_rate_hz,
                    "audioChannels": probed.audio_channels,
                    "bitrateBps": probed.bitrate_bps,
                    "attempt": attempt.job.processing_attempt_count + 1
                }),
            )
            .await
            .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
            Ok(probed)
        }
        Err(error) => {
            let _ = finish_media_processing_run(
                state.db.sqlite_adapter(),
                &probe_run_id,
                "failed",
                json!({ "error": error.to_string() }),
            )
            .await;
            Err((error, attempt.lease_updated_at.clone()))
        }
    }
}

pub(crate) async fn run_integrity_stage(
    state: &SharedState,
    creator_id: &str,
    job_id: &str,
    attempt: &MediaProcessingAttempt,
    probed: &ProbedMedia,
) -> Result<(), (AppError, String)> {
    let integrity_run_id = start_media_processing_run(
        state.db.sqlite_adapter(),
        creator_id,
        job_id,
        &attempt.asset.id,
        "integrity",
        json!({
            "sourcePath": attempt.session.relative_path,
        }),
    )
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    match verify_media_integrity(&attempt.source_path, probed).await {
        Ok(()) => {
            finish_media_processing_run(
                state.db.sqlite_adapter(),
                &integrity_run_id,
                "completed",
                json!({
                    "sourcePath": attempt.session.relative_path,
                    "hasVideo": probed.has_video,
                    "hasAudio": probed.has_audio,
                }),
            )
            .await
            .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
            Ok(())
        }
        Err(error) => {
            let _ = finish_media_processing_run(
                state.db.sqlite_adapter(),
                &integrity_run_id,
                "failed",
                json!({
                    "sourcePath": attempt.session.relative_path,
                    "error": error.to_string(),
                }),
            )
            .await;
            Err((error, attempt.lease_updated_at.clone()))
        }
    }
}
