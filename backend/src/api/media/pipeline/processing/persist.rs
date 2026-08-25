use super::*;
use crate::api::media::access::media_content_type;

pub(crate) async fn persist_media_variants(
    state: &SharedState,
    attempt: &MediaProcessingAttempt,
    probed: &ProbedMedia,
    generated: &GeneratedDerivativeBundle,
) -> Result<(), (AppError, String)> {
    publish_generated_media_files(state, generated)
        .await
        .map_err(|error| (error, attempt.lease_updated_at.clone()))?;

    replace_media_variants_for_database(&state.db, &attempt.asset.id, &{
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
    replace_media_preview_tracks_for_database(
        &state.db,
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

async fn publish_generated_media_files(
    state: &SharedState,
    generated: &GeneratedDerivativeBundle,
) -> AppResult<()> {
    if let Some(relative_path) = &generated.poster_relative_path {
        publish_relative_file(state, relative_path).await?;
    }
    for (_, relative_path, _, _) in &generated.image_derivatives_relative_paths {
        publish_relative_file(state, relative_path).await?;
    }
    if let Some(track) = &generated.timeline_preview_track {
        publish_relative_file(state, &track.image_relative_path).await?;
        publish_relative_file(state, &track.vtt_relative_path).await?;
    }
    for (_, relative_path, _, _, _) in &generated.subtitle_variants {
        publish_relative_file(state, relative_path).await?;
    }

    let hls_root = PathBuf::from(&generated.hls_relative_path)
        .parent()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| "processed".to_string());
    for relative_path in collect_relative_files(state, &hls_root)? {
        publish_relative_file(state, &relative_path).await?;
    }

    Ok(())
}

async fn publish_relative_file(state: &SharedState, relative_path: &str) -> AppResult<()> {
    let full_path = media_path_for_relative(state, relative_path);
    state
        .storage
        .publish_file(relative_path, &full_path, media_content_type(relative_path))
        .await
}

fn collect_relative_files(state: &SharedState, relative_root: &str) -> AppResult<Vec<String>> {
    let root = media_path_for_relative(state, relative_root);
    let mut files = Vec::new();
    collect_relative_files_from_dir(&root, relative_root.trim_end_matches('/'), &mut files)?;
    files.sort();
    files.dedup();
    Ok(files)
}

fn collect_relative_files_from_dir(
    dir: &std::path::Path,
    relative_dir: &str,
    files: &mut Vec<String>,
) -> AppResult<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| AppError::Internal("generated media path must be utf-8".to_string()))?;
        let relative_path = format!("{relative_dir}/{name}");
        if file_type.is_dir() {
            collect_relative_files_from_dir(&entry.path(), &relative_path, files)?;
        } else if file_type.is_file() {
            files.push(relative_path);
        }
    }
    Ok(())
}
