use super::*;

pub(crate) async fn finalize_media_processing(
    state: &SharedState,
    creator_id: &str,
    job_id: &str,
    attempt: &MediaProcessingAttempt,
    probed: &ProbedMedia,
    generated: &GeneratedDerivativeBundle,
) -> Result<(), (AppError, String)> {
    let completed_at = Utc::now().to_rfc3339();
    let asset_update = sqlx::query(
        r#"
        UPDATE media_assets
        SET status = 'ready',
            source_relative_path = ?,
            poster_relative_path = ?,
            playback_relative_path = ?,
            mime_type = ?,
            checksum_sha256 = ?,
            container_format = ?,
            file_size_bytes = ?,
            duration_sec = ?,
            width = ?,
            height = ?,
            frame_rate = ?,
            video_codec = ?,
            audio_codec = ?,
            has_video = ?,
            has_audio = ?,
            updated_at = ?,
            processed_at = ?
        WHERE upload_job_id = ? AND creator_id = ?
          AND status = 'processing'
          AND updated_at = ?
        "#,
    )
    .bind(&attempt.session.relative_path)
    .bind(generated.poster_relative_path.clone())
    .bind(&generated.hls_relative_path)
    .bind(&attempt.job.mime_type)
    .bind(attempt.job.checksum_sha256.clone())
    .bind(probed.container_format.clone())
    .bind(attempt.job.bytes_expected)
    .bind(probed.duration_sec)
    .bind(probed.width)
    .bind(probed.height)
    .bind(probed.frame_rate)
    .bind(probed.video_codec.clone())
    .bind(probed.audio_codec.clone())
    .bind(probed.has_video as i64)
    .bind(probed.has_audio as i64)
    .bind(&completed_at)
    .bind(&completed_at)
    .bind(job_id)
    .bind(creator_id)
    .bind(&attempt.lease_updated_at)
    .execute(state.db.sqlite_adapter())
    .await
    .map_err(|error| (AppError::from(error), attempt.lease_updated_at.clone()))?;
    if asset_update.rows_affected() == 0 {
        return Ok(());
    }

    let job_update = sqlx::query(
        "UPDATE upload_jobs SET status = 'ready', updated_at = ?, last_processing_error = NULL, last_failed_at = NULL WHERE id = ? AND creator_id = ? AND status = 'processing' AND updated_at = ?",
    )
    .bind(&completed_at)
    .bind(job_id)
    .bind(creator_id)
    .bind(&attempt.lease_updated_at)
    .execute(state.db.sqlite_adapter())
    .await
    .map_err(|error| (AppError::from(error), attempt.lease_updated_at.clone()))?;
    if job_update.rows_affected() == 0 {
        return Ok(());
    }

    Ok(())
}
