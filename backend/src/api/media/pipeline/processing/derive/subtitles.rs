use super::*;

pub(super) async fn generate_subtitle_variants(
    state: &SharedState,
    creator_id: &str,
    job_id: &str,
    attempt: &MediaProcessingAttempt,
    probed: &ProbedMedia,
    processed_root: &str,
) -> Result<Vec<(String, String, String, i64, bool)>, (AppError, String)> {
    if probed.subtitle_streams.is_empty() {
        return Ok(Vec::new());
    }

    let subtitles_run_id = start_media_processing_run(
        &state.pool,
        creator_id,
        job_id,
        &attempt.asset.id,
        "captions",
        json!({
            "streams": probed.subtitle_streams.iter().map(|stream| {
                json!({
                    "streamIndex": stream.stream_index,
                    "codec": stream.codec,
                    "language": stream.language,
                })
            }).collect::<Vec<_>>(),
        }),
    )
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    let mut generated = Vec::new();
    let mut skipped = Vec::new();
    for (ordinal, stream) in probed.subtitle_streams.iter().enumerate() {
        if !subtitle_codec_supported_for_normalization(stream.codec.as_deref()) {
            skipped.push(json!({
                "streamIndex": stream.stream_index,
                "codec": stream.codec,
                "language": stream.language,
                "reason": "unsupported_subtitle_codec",
            }));
            continue;
        }
        let language = stream.language.as_deref().unwrap_or("und");
        let label = if ordinal == 0 {
            format!("captions-{language}")
        } else {
            format!("captions-{language}-{}", ordinal + 1)
        };
        let relative_path = format!("{processed_root}/captions/{label}.vtt");
        let full_path = media_path_for_relative(state, &relative_path);
        ensure_parent_dir(&full_path)
            .await
            .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
        if let Err(error) =
            extract_subtitle_stream_to_webvtt(&attempt.source_path, stream, &full_path).await
        {
            let _ = finish_media_processing_run(
                &state.pool,
                &subtitles_run_id,
                "failed",
                json!({
                    "streamIndex": stream.stream_index,
                    "codec": stream.codec,
                    "language": stream.language,
                    "target": relative_path,
                    "error": error.to_string(),
                }),
            )
            .await;
            return Err((error, attempt.lease_updated_at.clone()));
        }
        let metadata = std::fs::metadata(&full_path)
            .map_err(AppError::from)
            .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
        generated.push((
            label,
            relative_path,
            language.to_string(),
            metadata.len() as i64,
            ordinal == 0,
        ));
    }
    finish_media_processing_run(
        &state.pool,
        &subtitles_run_id,
        "completed",
        json!({
            "generated": generated.iter().map(|(label, relative_path, language, file_size_bytes, is_default)| {
                json!({
                    "label": label,
                    "target": relative_path,
                    "language": language,
                    "fileSizeBytes": file_size_bytes,
                    "default": is_default,
                })
            }).collect::<Vec<_>>(),
            "skipped": skipped,
        }),
    )
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    Ok(generated)
}
