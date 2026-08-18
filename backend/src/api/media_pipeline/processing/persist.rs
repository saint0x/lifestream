use super::*;

pub(crate) async fn persist_media_variants(
    state: &SharedState,
    attempt: &MediaProcessingAttempt,
    probed: &ProbedMedia,
    generated: &GeneratedDerivativeBundle,
) -> Result<(), (AppError, String)> {
    replace_media_variants(&state.pool, &attempt.asset.id, &{
        let mut variants = vec![NewMediaVariant {
            variant_type: "source",
            label: "source".to_string(),
            relative_path: attempt.session.relative_path.clone(),
            mime_type: attempt.job.mime_type.clone(),
            width: probed.width,
            height: probed.height,
            bitrate_bps: probed.bitrate_bps,
            file_size_bytes: attempt.job.bytes_expected,
            is_default: false,
        }];
        for (label, relative_path, width, height) in &generated.image_derivatives_relative_paths {
            let full_path = media_path_for_relative(state, relative_path);
            let metadata = std::fs::metadata(&full_path)
                .map_err(AppError::from)
                .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
            variants.push(NewMediaVariant {
                variant_type: "thumbnail",
                label: label.clone(),
                relative_path: relative_path.clone(),
                mime_type: "image/jpeg".to_string(),
                width: Some(*width),
                height: Some(*height),
                bitrate_bps: None,
                file_size_bytes: metadata.len() as i64,
                is_default: label == "card_thumbnail",
            });
        }
        for (label, relative_path, language, file_size_bytes, is_default) in
            generated.subtitle_variants.iter().cloned()
        {
            variants.push(NewMediaVariant {
                variant_type: "caption",
                label: format!("{label}:{language}"),
                relative_path,
                mime_type: "text/vtt".to_string(),
                width: None,
                height: None,
                bitrate_bps: None,
                file_size_bytes,
                is_default,
            });
        }
        for track in &generated.generated_package.audio_tracks {
            variants.push(NewMediaVariant {
                variant_type: "audio",
                label: format!(
                    "{}:{}:{}:{}:{}",
                    track.label,
                    track.language,
                    "source-provided",
                    if track.is_dubbed { 1 } else { 0 },
                    track.codec
                ),
                relative_path: format!(
                    "{}/{}",
                    PathBuf::from(&generated.hls_relative_path)
                        .parent()
                        .map(|path| path.to_string_lossy().to_string())
                        .unwrap_or_else(|| "processed".to_string()),
                    track.relative_playlist_path
                ),
                mime_type: "application/vnd.apple.mpegurl".to_string(),
                width: None,
                height: None,
                bitrate_bps: Some(track.bitrate_bps),
                file_size_bytes: track.file_size_bytes,
                is_default: track.is_default,
            });
        }
        let highest_height = generated
            .generated_package
            .variants
            .iter()
            .map(|variant| variant.plan.height)
            .max()
            .unwrap_or_default();
        for variant in &generated.generated_package.variants {
            variants.push(NewMediaVariant {
                variant_type: "playback",
                label: variant.plan.label.clone(),
                relative_path: format!(
                    "{}/{}",
                    PathBuf::from(&generated.hls_relative_path)
                        .parent()
                        .map(|path| path.to_string_lossy().to_string())
                        .unwrap_or_else(|| "processed".to_string()),
                    variant.relative_playlist_path
                ),
                mime_type: "application/vnd.apple.mpegurl".to_string(),
                width: Some(variant.plan.width),
                height: Some(variant.plan.height),
                bitrate_bps: Some(variant.plan.bandwidth_bps),
                file_size_bytes: variant.file_size_bytes,
                is_default: variant.plan.height == highest_height,
            });
        }
        variants
    })
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    replace_media_preview_tracks(
        &state.pool,
        &attempt.asset.id,
        &generated
            .timeline_preview_track
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
    )
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))
}
