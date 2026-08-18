use super::*;

pub(crate) async fn begin_media_processing_attempt(
    state: &SharedState,
    creator_id: &str,
    job_id: &str,
) -> AppResult<Option<MediaProcessingAttempt>> {
    let job = fetch_upload_job_by_id(&state.pool, creator_id, job_id).await?;
    if job.status != "uploaded" {
        return Ok(None);
    }

    let now = Utc::now().to_rfc3339();
    let claimed = sqlx::query(
        "UPDATE upload_jobs SET status = 'processing', updated_at = ?, processing_attempt_count = processing_attempt_count + 1 WHERE id = ? AND creator_id = ? AND status = 'uploaded'",
    )
        .bind(&now)
        .bind(job_id)
        .bind(creator_id)
        .execute(&state.pool)
        .await?;
    if claimed.rows_affected() == 0 {
        return Ok(None);
    }

    let job = fetch_upload_job_by_id(&state.pool, creator_id, job_id).await?;
    let session = fetch_upload_ingest_session(&state.pool, creator_id, job_id).await?;
    let asset =
        ensure_media_asset_shell(&state.pool, creator_id, &job, &session.relative_path).await?;
    let source_path = media_path_for_relative(state, &session.relative_path);

    sqlx::query(
        "UPDATE media_assets SET status = 'processing', updated_at = ? WHERE upload_job_id = ? AND creator_id = ?",
    )
        .bind(&now)
        .bind(job_id)
        .bind(creator_id)
        .execute(&state.pool)
        .await?;

    Ok(Some(MediaProcessingAttempt {
        job,
        session,
        asset,
        source_path,
        lease_updated_at: now,
    }))
}

pub(crate) async fn run_probe_stage(
    state: &SharedState,
    creator_id: &str,
    job_id: &str,
    attempt: &MediaProcessingAttempt,
) -> Result<ProbedMedia, (AppError, String)> {
    let probe_run_id = start_media_processing_run(
        &state.pool,
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
                &state.pool,
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
                &state.pool,
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
        &state.pool,
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
                &state.pool,
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
                &state.pool,
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

pub(crate) async fn generate_derivatives_and_package(
    state: &SharedState,
    creator_id: &str,
    job_id: &str,
    attempt: &MediaProcessingAttempt,
    probed: &ProbedMedia,
) -> Result<GeneratedDerivativeBundle, (AppError, String)> {
    let processed_root = format!("processed/{creator_id}/{job_id}");
    let poster_relative_path =
        generate_poster_derivative(state, creator_id, job_id, attempt, probed, &processed_root)
            .await?;
    let image_derivatives_relative_paths =
        generate_image_derivatives(state, creator_id, job_id, attempt, probed, &processed_root)
            .await?;
    let timeline_preview_track =
        generate_timeline_preview(state, creator_id, job_id, attempt, probed, &processed_root)
            .await?;
    let subtitle_variants =
        generate_subtitle_variants(state, creator_id, job_id, attempt, probed, &processed_root)
            .await?;
    let (generated_package, hls_relative_path) = generate_hls_package(
        state,
        creator_id,
        job_id,
        attempt,
        probed,
        &processed_root,
        &subtitle_variants,
    )
    .await?;

    Ok(GeneratedDerivativeBundle {
        poster_relative_path,
        image_derivatives_relative_paths,
        timeline_preview_track,
        subtitle_variants,
        generated_package,
        hls_relative_path,
    })
}

async fn generate_poster_derivative(
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

async fn generate_image_derivatives(
    state: &SharedState,
    creator_id: &str,
    job_id: &str,
    attempt: &MediaProcessingAttempt,
    probed: &ProbedMedia,
    processed_root: &str,
) -> Result<Vec<(String, String, i64, i64)>, (AppError, String)> {
    if !probed.has_video {
        return Ok(Vec::new());
    }

    let image_derivative_plans = build_image_derivative_plans(probed)
        .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    let derivatives_run_id = start_media_processing_run(
        &state.pool,
        creator_id,
        job_id,
        &attempt.asset.id,
        "thumbnails",
        json!({
            "targets": image_derivative_plans.iter().map(|plan| {
                json!({
                    "label": plan.label,
                    "maxWidth": plan.max_width,
                    "maxHeight": plan.max_height,
                })
            }).collect::<Vec<_>>(),
        }),
    )
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    let mut derived = Vec::with_capacity(image_derivative_plans.len());
    for plan in &image_derivative_plans {
        let relative_path = format!("{processed_root}/images/{}.jpg", plan.label);
        let full_path = media_path_for_relative(state, &relative_path);
        ensure_parent_dir(&full_path)
            .await
            .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
        let (width, height) = scaled_dimensions_for_rung(
            probed.width.unwrap_or(plan.max_width),
            probed.height.unwrap_or(plan.max_height),
            plan.max_width,
            plan.max_height,
        );
        if let Err(error) = generate_thumbnail(
            &attempt.source_path,
            &full_path,
            probed.duration_sec,
            width,
            height,
        )
        .await
        {
            let _ = finish_media_processing_run(
                &state.pool,
                &derivatives_run_id,
                "failed",
                json!({
                    "target": relative_path,
                    "error": error.to_string(),
                }),
            )
            .await;
            return Err((error, attempt.lease_updated_at.clone()));
        }
        derived.push((plan.label.to_string(), relative_path, width, height));
    }
    finish_media_processing_run(
        &state.pool,
        &derivatives_run_id,
        "completed",
        json!({
            "targets": derived.iter().map(|(label, relative_path, width, height)| {
                json!({
                    "label": label,
                    "target": relative_path,
                    "width": width,
                    "height": height,
                })
            }).collect::<Vec<_>>(),
        }),
    )
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    Ok(derived)
}

async fn generate_timeline_preview(
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

async fn generate_subtitle_variants(
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

async fn generate_hls_package(
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
    let hls_run_id = start_media_processing_run(
        &state.pool,
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
            finish_media_processing_run(
                &state.pool,
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
            let _ = finish_media_processing_run(
                &state.pool,
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
