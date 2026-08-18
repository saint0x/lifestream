use super::*;

mod assets;
mod job_control;
mod packaging;
mod probe;

pub(super) use assets::{
    NewMediaVariant, StoredMediaPreviewTrack, ensure_media_asset_shell,
    fetch_media_asset_by_id_any_creator, fetch_media_asset_by_upload_id,
    fetch_media_asset_by_upload_job, fetch_media_asset_variants, fetch_media_assets,
    fetch_media_preview_track_rows, fetch_media_processing_runs, finish_media_processing_run,
    replace_media_preview_tracks, replace_media_variants, start_media_processing_run,
};
pub(super) use job_control::{
    MAX_MEDIA_PROCESSING_ATTEMPTS, fail_media_job_for_lease, fetch_admin_media_job_record,
    fetch_admin_media_jobs, fetch_pending_media_jobs, fetch_upload_ingest_session,
    fetch_upload_ingest_sessions, fetch_upload_job_by_id, fetch_upload_job_by_id_global,
    fetch_upload_job_creator_id, fetch_upload_jobs, publish_due_scheduled_upload_releases,
    reconcile_single_media_job, reconcile_stale_media_processing_jobs,
    reconcile_stale_media_processing_jobs_for_read, requeue_media_job_for_processing,
    schedule_media_processing,
};
pub(super) use packaging::{
    GeneratedHlsPackage, GeneratedHlsSubtitleTrack, GeneratedHlsVariant, HlsVariantPlan,
    build_image_derivative_plans, extract_subtitle_stream_to_webvtt, generate_hls, generate_poster,
    generate_thumbnail, generate_timeline_preview_track, plan_hls_variants,
    scaled_dimensions_for_rung, subtitle_codec_supported_for_normalization,
    validate_generated_hls_package, write_hls_master_manifest,
};
pub(super) use probe::{
    ProbedAudioStream, ProbedMedia, classify_media_processing_error, probe_media,
    validate_probed_media, verify_media_integrity,
};

struct MediaProcessingAttempt {
    job: UploadJob,
    session: UploadIngestSession,
    asset: MediaAsset,
    source_path: PathBuf,
    lease_updated_at: String,
}

async fn begin_media_processing_attempt(
    state: &SharedState,
    creator_id: &str,
    job_id: &str,
) -> AppResult<Option<MediaProcessingAttempt>> {
    let job = fetch_upload_job_by_id(&state.pool, creator_id, job_id).await?;
    if job.status != "uploaded" && job.status != "processing" {
        return Ok(None);
    }
    let session = fetch_upload_ingest_session(&state.pool, creator_id, job_id).await?;
    let asset =
        ensure_media_asset_shell(&state.pool, creator_id, &job, &session.relative_path).await?;
    let source_path = media_path_for_relative(state, &session.relative_path);

    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE upload_jobs SET status = 'processing', updated_at = ?, processing_attempt_count = processing_attempt_count + 1 WHERE id = ? AND creator_id = ?")
        .bind(&now)
        .bind(job_id)
        .bind(creator_id)
        .execute(&state.pool)
        .await?;
    sqlx::query("UPDATE media_assets SET status = 'processing', updated_at = ? WHERE upload_job_id = ? AND creator_id = ?")
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

async fn process_media_job(
    state: SharedState,
    creator_id: &str,
    job_id: &str,
) -> Result<(), (AppError, String)> {
    let Some(attempt) = begin_media_processing_attempt(&state, creator_id, job_id)
        .await
        .map_err(|error| (error, String::new()))?
    else {
        return Ok(());
    };

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
    let probed = match probe_media(&attempt.source_path).await {
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
            probed
        }
        Err(error) => {
            let _ = finish_media_processing_run(
                &state.pool,
                &probe_run_id,
                "failed",
                json!({ "error": error.to_string() }),
            )
            .await;
            return Err((error, attempt.lease_updated_at.clone()));
        }
    };
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
    match verify_media_integrity(&attempt.source_path, &probed).await {
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
            return Err((error, attempt.lease_updated_at.clone()));
        }
    }

    let processed_root = format!("processed/{creator_id}/{job_id}");
    let poster_relative_path = if probed.has_video {
        let poster_relative_path = format!("{processed_root}/poster.jpg");
        let poster_full_path = media_path_for_relative(&state, &poster_relative_path);
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
            }
            Err(error) => {
                let _ = finish_media_processing_run(
                    &state.pool,
                    &poster_run_id,
                    "failed",
                    json!({ "target": poster_relative_path, "error": error.to_string() }),
                )
                .await;
                return Err((error, attempt.lease_updated_at.clone()));
            }
        }
        Some(poster_relative_path)
    } else {
        None
    };
    let image_derivative_plans = build_image_derivative_plans(&probed)
        .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    let image_derivatives_relative_paths = if probed.has_video {
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
            let full_path = media_path_for_relative(&state, &relative_path);
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
        derived
    } else {
        Vec::new()
    };
    let timeline_preview_track = if probed.has_video {
        let image_relative_path = format!("{processed_root}/images/timeline_preview.jpg");
        let vtt_relative_path = format!("{processed_root}/images/timeline_preview.vtt");
        let image_full_path = media_path_for_relative(&state, &image_relative_path);
        let vtt_full_path = media_path_for_relative(&state, &vtt_relative_path);
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
                Some(track)
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
                return Err((error, attempt.lease_updated_at.clone()));
            }
        }
    } else {
        None
    };
    let subtitle_variants = if probed.subtitle_streams.is_empty() {
        Vec::new()
    } else {
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
            let full_path = media_path_for_relative(&state, &relative_path);
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
        generated
    };

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
    let hls_full_path = media_path_for_relative(&state, &hls_relative_path);
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
        &probed,
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

    if !job_control::media_processing_lease_is_active(
        &state.pool,
        creator_id,
        job_id,
        &attempt.lease_updated_at,
    )
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))?
    {
        return Ok(());
    }

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
        for (label, relative_path, width, height) in &image_derivatives_relative_paths {
            let full_path = media_path_for_relative(&state, relative_path);
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
            subtitle_variants.iter().cloned()
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
        for track in &generated_package.audio_tracks {
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
                    PathBuf::from(&hls_relative_path)
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
        let highest_height = generated_package
            .variants
            .iter()
            .map(|variant| variant.plan.height)
            .max()
            .unwrap_or_default();
        for variant in &generated_package.variants {
            variants.push(NewMediaVariant {
                variant_type: "playback",
                label: variant.plan.label.clone(),
                relative_path: format!(
                    "{}/{}",
                    PathBuf::from(&hls_relative_path)
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
        &timeline_preview_track.into_iter().collect::<Vec<_>>(),
    )
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))?;

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
    .bind(poster_relative_path)
    .bind(&hls_relative_path)
    .bind(&attempt.job.mime_type)
    .bind(attempt.job.checksum_sha256.clone())
    .bind(probed.container_format)
    .bind(attempt.job.bytes_expected)
    .bind(probed.duration_sec)
    .bind(probed.width)
    .bind(probed.height)
    .bind(probed.frame_rate)
    .bind(probed.video_codec)
    .bind(probed.audio_codec)
    .bind(probed.has_video as i64)
    .bind(probed.has_audio as i64)
    .bind(&completed_at)
    .bind(&completed_at)
    .bind(job_id)
    .bind(creator_id)
    .bind(&attempt.lease_updated_at)
    .execute(&state.pool)
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
    .execute(&state.pool)
    .await
    .map_err(|error| (AppError::from(error), attempt.lease_updated_at.clone()))?;
    if job_update.rows_affected() == 0 {
        return Ok(());
    }

    Ok(())
}
