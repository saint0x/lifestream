use super::*;

pub(super) async fn generate_hls_package(
    state: &SharedState,
    creator_id: &str,
    job_id: &str,
    attempt: &MediaProcessingAttempt,
    probed: &ProbedMedia,
    processed_root: &str,
    subtitle_variants: &[(String, String, String, i64, bool)],
) -> Result<(GeneratedHlsPackage, String), (AppError, String)> {
    let hls_subtitle_tracks = subtitle_variants
        .iter()
        .map(
            |(label, relative_path, language, _file_size_bytes, is_default)| {
                GeneratedHlsSubtitleTrack {
                    relative_path: PathBuf::from(relative_path)
                        .file_name()
                        .map(|name| format!("../captions/{}", name.to_string_lossy()))
                        .unwrap_or_else(|| relative_path.clone()),
                    language: language.clone(),
                    name: label.clone(),
                    is_default: *is_default,
                }
            },
        )
        .collect::<Vec<_>>();

    let hls_relative_path = format!("{processed_root}/hls/master.m3u8");
    let hls_full_path = media_path_for_relative(state, &hls_relative_path);
    ensure_parent_dir(&hls_full_path)
        .await
        .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    let hls_run_id = start_media_processing_run_for_database(
        &state.db,
        creator_id,
        job_id,
        &attempt.asset.id,
        "package",
        json!({ "target": hls_relative_path }),
    )
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    let generated_package = match generate_hls(
        &attempt.source_path,
        &hls_full_path,
        probed,
        &hls_subtitle_tracks,
    )
    .await
    {
        Ok(package) => {
            finish_media_processing_run_for_database(
                &state.db,
                &hls_run_id,
                "completed",
                json!({
                    "target": hls_relative_path,
                    "masterRelativePath": package.master_relative_path,
                    "variantCount": package.variants.len(),
                    "audioTrackCount": package.audio_tracks.len(),
                    "variants": package.variants.iter().map(|variant| {
                        json!({
                            "label": variant.plan.label.clone(),
                            "width": variant.plan.width,
                            "height": variant.plan.height,
                            "bandwidthBps": variant.plan.bandwidth_bps,
                            "playlistPath": variant.relative_playlist_path.clone(),
                            "fileSizeBytes": variant.file_size_bytes,
                        })
                    }).collect::<Vec<_>>(),
                    "audioTracks": package.audio_tracks.iter().map(|track| {
                        json!({
                            "label": track.label.clone(),
                            "language": track.language.clone(),
                            "codec": track.codec.clone(),
                            "bitrateBps": track.bitrate_bps,
                            "playlistPath": track.relative_playlist_path.clone(),
                            "fileSizeBytes": track.file_size_bytes,
                            "default": track.is_default,
                            "dubbed": track.is_dubbed,
                        })
                    }).collect::<Vec<_>>(),
                }),
            )
            .await
            .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
            package
        }
        Err(error) => {
            let _ = finish_media_processing_run_for_database(
                &state.db,
                &hls_run_id,
                "failed",
                json!({ "target": hls_relative_path, "error": error.to_string() }),
            )
            .await;
            return Err((error, attempt.lease_updated_at.clone()));
        }
    };
    Ok((generated_package, hls_relative_path))
}
