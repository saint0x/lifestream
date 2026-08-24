use super::*;

#[tokio::test]
async fn admin_retry_preserves_processing_attempt_history() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);
    let row = sqlx::query(
        r#"
        SELECT upload_jobs.id
        FROM upload_jobs
        INNER JOIN media_assets
            ON media_assets.upload_job_id = upload_jobs.id
           AND media_assets.creator_id = upload_jobs.creator_id
        WHERE upload_jobs.creator_id = ?
        ORDER BY upload_jobs.created_at ASC
        LIMIT 1
        "#,
    )
    .bind(&creator.id)
    .fetch_one(state.db.sqlite_adapter())
    .await?;
    let job_id: String = row.get("id");
    let failed_at = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE upload_jobs SET status = 'failed', processing_attempt_count = 3, last_processing_error = ?, last_failed_at = ?, updated_at = ? WHERE id = ? AND creator_id = ?",
    )
    .bind("operator retry test")
    .bind(&failed_at)
    .bind(&failed_at)
    .bind(&job_id)
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;
    sqlx::query(
        "UPDATE media_assets SET status = 'failed', updated_at = ? WHERE upload_job_id = ? AND creator_id = ?",
    )
    .bind(&failed_at)
    .bind(&job_id)
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;

    let retried = retry_admin_media_job(State(state.clone()), headers, Path(job_id.clone()))
        .await?
        .0;

    assert_eq!(retried.upload_job.status, "uploaded");
    assert_eq!(retried.upload_job.processing_attempt_count, 3);
    assert!(retried.upload_job.last_processing_error.is_none());
    assert!(retried.upload_job.last_failed_at.is_none());
    assert_eq!(retried.asset_status.as_deref(), Some("uploaded"));
    Ok(())
}

#[test]
fn hls_variant_planner_scales_without_upscaling() -> AppResult<()> {
    let hd_media = ProbedMedia {
        container_format: Some("mov,mp4,m4a,3gp,3g2,mj2".to_string()),
        duration_sec: 3.0,
        width: Some(1280),
        height: Some(720),
        frame_rate: Some(24.0),
        video_codec: Some("h264".to_string()),
        audio_codec: Some("aac".to_string()),
        audio_sample_rate_hz: Some(48_000),
        audio_channels: Some(2),
        has_video: true,
        has_audio: true,
        bitrate_bps: Some(4_000_000),
        audio_streams: vec![ProbedAudioStream {
            stream_index: 1,
            codec: Some("aac".to_string()),
            language: Some("eng".to_string()),
            sample_rate_hz: Some(48_000),
            channels: Some(2),
        }],
        subtitle_streams: Vec::new(),
    };
    let hd_variants = plan_hls_variants(&hd_media)?;
    let hd_labels = hd_variants
        .iter()
        .map(|variant| variant.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(hd_labels, vec!["240p", "360p", "480p", "720p"]);
    assert!(hd_variants.iter().all(|variant| variant.width <= 1280));
    assert!(hd_variants.iter().all(|variant| variant.height <= 720));

    let small_media = ProbedMedia {
        container_format: Some("mov,mp4,m4a,3gp,3g2,mj2".to_string()),
        duration_sec: 3.0,
        width: Some(320),
        height: Some(240),
        frame_rate: Some(24.0),
        video_codec: Some("h264".to_string()),
        audio_codec: Some("aac".to_string()),
        audio_sample_rate_hz: Some(48_000),
        audio_channels: Some(2),
        has_video: true,
        has_audio: true,
        bitrate_bps: Some(800_000),
        audio_streams: vec![ProbedAudioStream {
            stream_index: 1,
            codec: Some("aac".to_string()),
            language: Some("eng".to_string()),
            sample_rate_hz: Some(48_000),
            channels: Some(2),
        }],
        subtitle_streams: Vec::new(),
    };
    let small_variants = plan_hls_variants(&small_media)?;
    assert_eq!(small_variants.len(), 1);
    assert_eq!(small_variants[0].label, "240p");
    assert_eq!(small_variants[0].width, 320);
    assert_eq!(small_variants[0].height, 240);

    Ok(())
}

#[test]
fn media_validation_rejects_invalid_audio_sample_rate_and_channel_count() {
    let job = UploadJob {
        id: "job-test".to_string(),
        upload_id: None,
        series_id: None,
        kind: "film".to_string(),
        source_type: "resumable-upload".to_string(),
        status: "uploaded".to_string(),
        title: "Validation".to_string(),
        intended_visibility: "private".to_string(),
        bytes_expected: 1024,
        bytes_received: 1024,
        storage_key: "uploads/test.mp4".to_string(),
        created_at: "2026-08-17T00:00:00+00:00".to_string(),
        updated_at: "2026-08-17T00:00:00+00:00".to_string(),
        published_content_id: None,
        mime_type: "video/mp4".to_string(),
        checksum_sha256: Some("abc".to_string()),
        completed_at: Some("2026-08-17T00:00:00+00:00".to_string()),
        processing_attempt_count: 0,
        last_processing_error: None,
        last_failed_at: None,
    };

    let invalid_sample_rate = ProbedMedia {
        container_format: Some("mov,mp4,m4a,3gp,3g2,mj2".to_string()),
        duration_sec: 3.0,
        width: Some(1280),
        height: Some(720),
        frame_rate: Some(24.0),
        video_codec: Some("h264".to_string()),
        audio_codec: Some("aac".to_string()),
        audio_sample_rate_hz: Some(4_000),
        audio_channels: Some(2),
        has_video: true,
        has_audio: true,
        bitrate_bps: Some(4_000_000),
        audio_streams: vec![ProbedAudioStream {
            stream_index: 1,
            codec: Some("aac".to_string()),
            language: Some("eng".to_string()),
            sample_rate_hz: Some(4_000),
            channels: Some(2),
        }],
        subtitle_streams: Vec::new(),
    };
    let invalid_channels = ProbedMedia {
        container_format: Some("mov,mp4,m4a,3gp,3g2,mj2".to_string()),
        duration_sec: 3.0,
        width: Some(1280),
        height: Some(720),
        frame_rate: Some(24.0),
        video_codec: Some("h264".to_string()),
        audio_codec: Some("aac".to_string()),
        audio_sample_rate_hz: Some(48_000),
        audio_channels: Some(16),
        has_video: true,
        has_audio: true,
        bitrate_bps: Some(4_000_000),
        audio_streams: vec![ProbedAudioStream {
            stream_index: 1,
            codec: Some("aac".to_string()),
            language: Some("eng".to_string()),
            sample_rate_hz: Some(48_000),
            channels: Some(16),
        }],
        subtitle_streams: Vec::new(),
    };

    let sample_rate_error = validate_probed_media(&job, &invalid_sample_rate)
        .expect_err("sample rate below production floor must be rejected");
    let channels_error = validate_probed_media(&job, &invalid_channels)
        .expect_err("audio channel count above supported ceiling must be rejected");

    match sample_rate_error {
        AppError::BadRequest(message) => {
            assert!(message.contains("audio sample rate"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
    match channels_error {
        AppError::BadRequest(message) => {
            assert!(message.contains("audio channel count"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn hls_master_manifest_contains_variant_entries() -> AppResult<()> {
    let temp_root = std::env::temp_dir().join(format!("vanta-hls-test-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_root).await?;
    let master_path = temp_root.join("master.m3u8");

    write_hls_master_manifest(
        &master_path,
        &[
            GeneratedHlsVariant {
                plan: HlsVariantPlan {
                    label: "240p".to_string(),
                    width: 320,
                    height: 240,
                    video_bitrate_bps: 700_000,
                    bandwidth_bps: 796_000,
                },
                relative_playlist_path: "240p/playlist.m3u8".to_string(),
                file_size_bytes: 12_000,
            },
            GeneratedHlsVariant {
                plan: HlsVariantPlan {
                    label: "720p".to_string(),
                    width: 1280,
                    height: 720,
                    video_bitrate_bps: 4_500_000,
                    bandwidth_bps: 4_628_000,
                },
                relative_playlist_path: "720p/playlist.m3u8".to_string(),
                file_size_bytes: 48_000,
            },
        ],
        &[],
        &[GeneratedHlsSubtitleTrack {
            relative_path: "../captions/captions-eng.vtt".to_string(),
            language: "eng".to_string(),
            name: "captions-eng".to_string(),
            is_default: true,
        }],
    )
    .await?;

    let manifest = tokio::fs::read_to_string(&master_path).await?;
    assert!(manifest.contains("#EXT-X-MEDIA:TYPE=SUBTITLES"));
    assert!(manifest.contains("URI=\"../captions/captions-eng.vtt\""));
    assert!(manifest.contains("SUBTITLES=\"captions\""));
    assert!(manifest.contains("#EXT-X-STREAM-INF:BANDWIDTH=796000"));
    assert!(manifest.contains("RESOLUTION=320x240"));
    assert!(manifest.contains("240p/playlist.m3u8"));
    assert!(manifest.contains("#EXT-X-STREAM-INF:BANDWIDTH=4628000"));
    assert!(manifest.contains("RESOLUTION=1280x720"));
    assert!(manifest.contains("720p/playlist.m3u8"));

    let _ = tokio::fs::remove_dir_all(temp_root).await;
    Ok(())
}

#[tokio::test]
async fn generated_hls_package_validation_rejects_missing_variant_playlist() -> AppResult<()> {
    let temp_root = std::env::temp_dir().join(format!("vanta-hls-invalid-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(temp_root.join("240p")).await?;
    let master_path = temp_root.join("master.m3u8");

    write_hls_master_manifest(
        &master_path,
        &[GeneratedHlsVariant {
            plan: HlsVariantPlan {
                label: "240p".to_string(),
                width: 320,
                height: 240,
                video_bitrate_bps: 700_000,
                bandwidth_bps: 796_000,
            },
            relative_playlist_path: "240p/playlist.m3u8".to_string(),
            file_size_bytes: 12_000,
        }],
        &[],
        &[],
    )
    .await?;

    let error = validate_generated_hls_package(
        &master_path,
        &GeneratedHlsPackage {
            master_relative_path: master_path.to_string_lossy().to_string(),
            variants: vec![GeneratedHlsVariant {
                plan: HlsVariantPlan {
                    label: "240p".to_string(),
                    width: 320,
                    height: 240,
                    video_bitrate_bps: 700_000,
                    bandwidth_bps: 796_000,
                },
                relative_playlist_path: "240p/playlist.m3u8".to_string(),
                file_size_bytes: 12_000,
            }],
            audio_tracks: Vec::new(),
            subtitle_tracks: Vec::new(),
        },
    )
    .await
    .expect_err("missing child playlist must fail package validation");

    match error {
        AppError::Io(_) => {}
        AppError::MediaPipeline(message) => {
            assert!(message.contains("variant playlist"));
        }
        other => panic!("unexpected error: {other:?}"),
    }

    let _ = tokio::fs::remove_dir_all(temp_root).await;
    Ok(())
}

#[tokio::test]
async fn generated_hls_package_validation_accepts_complete_artifact_graph() -> AppResult<()> {
    let temp_root = std::env::temp_dir().join(format!("vanta-hls-valid-{}", Uuid::new_v4()));
    let variant_dir = temp_root.join("240p");
    tokio::fs::create_dir_all(&variant_dir).await?;
    let master_path = temp_root.join("master.m3u8");
    let playlist_path = variant_dir.join("playlist.m3u8");
    let segment_path = variant_dir.join("segment_000.ts");

    tokio::fs::write(&segment_path, b"segment-bytes").await?;
    tokio::fs::write(
        &playlist_path,
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXTINF:6.0,\nsegment_000.ts\n#EXT-X-ENDLIST\n",
    )
    .await?;
    write_hls_master_manifest(
        &master_path,
        &[GeneratedHlsVariant {
            plan: HlsVariantPlan {
                label: "240p".to_string(),
                width: 320,
                height: 240,
                video_bitrate_bps: 700_000,
                bandwidth_bps: 796_000,
            },
            relative_playlist_path: "240p/playlist.m3u8".to_string(),
            file_size_bytes: 12_000,
        }],
        &[],
        &[],
    )
    .await?;

    validate_generated_hls_package(
        &master_path,
        &GeneratedHlsPackage {
            master_relative_path: master_path.to_string_lossy().to_string(),
            variants: vec![GeneratedHlsVariant {
                plan: HlsVariantPlan {
                    label: "240p".to_string(),
                    width: 320,
                    height: 240,
                    video_bitrate_bps: 700_000,
                    bandwidth_bps: 796_000,
                },
                relative_playlist_path: "240p/playlist.m3u8".to_string(),
                file_size_bytes: 12_000,
            }],
            audio_tracks: Vec::new(),
            subtitle_tracks: Vec::new(),
        },
    )
    .await?;

    let _ = tokio::fs::remove_dir_all(temp_root).await;
    Ok(())
}

#[tokio::test]
async fn media_integrity_verification_accepts_decodable_mp4() -> AppResult<()> {
    let temp_root = std::env::temp_dir().join(format!("vanta-integrity-valid-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_root).await?;
    let media_path = temp_root.join("valid.mp4");

    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("testsrc=size=320x240:rate=24:duration=1")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("sine=frequency=1000:sample_rate=48000:duration=1")
        .arg("-c:v")
        .arg("libx264")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-c:a")
        .arg("aac")
        .arg("-shortest")
        .arg(&media_path)
        .output()
        .await?;
    assert!(
        output.status.success(),
        "ffmpeg fixture generation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let probed = probe_media(&media_path).await?;
    verify_media_integrity(&media_path, &probed).await?;

    let _ = tokio::fs::remove_dir_all(temp_root).await;
    Ok(())
}

#[tokio::test]
async fn media_integrity_verification_rejects_truncated_mp4() -> AppResult<()> {
    let temp_root =
        std::env::temp_dir().join(format!("vanta-integrity-invalid-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_root).await?;
    let valid_path = temp_root.join("source.mp4");
    let truncated_path = temp_root.join("truncated.mp4");

    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("testsrc=size=320x240:rate=24:duration=1")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("sine=frequency=1000:sample_rate=48000:duration=1")
        .arg("-c:v")
        .arg("libx264")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-c:a")
        .arg("aac")
        .arg("-shortest")
        .arg(&valid_path)
        .output()
        .await?;
    assert!(
        output.status.success(),
        "ffmpeg fixture generation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let payload = tokio::fs::read(&valid_path).await?;
    let truncated_len = (payload.len() / 2).max(1);
    tokio::fs::write(&truncated_path, &payload[..truncated_len]).await?;

    let probed = probe_media(&valid_path).await?;
    let error = verify_media_integrity(&truncated_path, &probed)
        .await
        .expect_err("truncated media must fail decode integrity verification");
    match error {
        AppError::MediaPipeline(message) => {
            assert!(message.contains("integrity verification failed"));
        }
        other => panic!("unexpected error: {other:?}"),
    }

    let _ = tokio::fs::remove_dir_all(temp_root).await;
    Ok(())
}

#[tokio::test]
async fn upload_job_patch_only_updates_creator_metadata() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);

    let created = create_upload_job(
        State(state.clone()),
        headers.clone(),
        Json(CreateUploadJobRequest {
            upload_id: None,
            series_id: None,
            kind: "film".to_string(),
            source_type: "resumable-upload".to_string(),
            title: "Original upload title".to_string(),
            intended_visibility: "private".to_string(),
            bytes_expected: 32,
            storage_key: format!(
                "uploads/creator/{}/features/metadata-update-{}.mp4",
                creator.handle,
                Uuid::new_v4().simple()
            ),
            mime_type: Some("video/mp4".to_string()),
        }),
    )
    .await?
    .0;

    let updated = update_upload_job(
        State(state.clone()),
        headers.clone(),
        Path(created.id.clone()),
        Json(UpdateUploadJobRequest {
            title: Some("Retitled upload".to_string()),
            intended_visibility: Some("unlisted".to_string()),
            series_id: None,
            mime_type: Some("video/quicktime".to_string()),
        }),
    )
    .await?
    .0;

    assert_eq!(updated.title, "Retitled upload");
    assert_eq!(updated.intended_visibility, "unlisted");
    assert!(updated.series_id.is_none());
    assert_eq!(updated.mime_type, "video/quicktime");
    assert_eq!(updated.status, "created");
    assert_eq!(updated.bytes_received, 0);
    assert!(updated.published_content_id.is_none());

    sqlx::query(
        "UPDATE upload_jobs SET status = 'processing', updated_at = ? WHERE id = ? AND creator_id = ?",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(&created.id)
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;

    let error = update_upload_job(
        State(state.clone()),
        headers,
        Path(created.id.clone()),
        Json(UpdateUploadJobRequest {
            title: Some("Should fail".to_string()),
            intended_visibility: None,
            series_id: None,
            mime_type: None,
        }),
    )
    .await
    .expect_err("processing job edits should be rejected");

    match error {
        AppError::BadRequest(message) => {
            assert!(message.contains("processing upload jobs"));
        }
        other => panic!("unexpected error: {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn upload_chunk_rejects_bytes_beyond_declared_upload_size() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);

    let created = create_upload_job(
        State(state.clone()),
        headers.clone(),
        Json(CreateUploadJobRequest {
            upload_id: None,
            series_id: None,
            kind: "film".to_string(),
            source_type: "resumable-upload".to_string(),
            title: "Overflow validation upload".to_string(),
            intended_visibility: "private".to_string(),
            bytes_expected: 4,
            storage_key: format!(
                "uploads/creator/{}/features/overflow-check-{}.mp4",
                creator.handle,
                Uuid::new_v4().simple()
            ),
            mime_type: Some("video/mp4".to_string()),
        }),
    )
    .await?
    .0;

    let ticket = start_upload_ingest_session(
        State(state.clone()),
        headers.clone(),
        Path(created.id.clone()),
    )
    .await?
    .0;

    let mut ingest_headers = headers.clone();
    ingest_headers.insert("x-upload-token", ticket.upload_token.parse().unwrap());

    let error = append_upload_chunk(
        State(state.clone()),
        ingest_headers,
        Path(created.id.clone()),
        Query(AppendUploadChunkQuery { offset: 0 }),
        Bytes::from_static(b"12345"),
    )
    .await
    .expect_err("oversized upload chunk should be rejected");

    match error {
        AppError::BadRequest(message) => {
            assert!(message.contains("chunk exceeds declared upload size"));
        }
        other => panic!("unexpected error: {other:?}"),
    }

    let refreshed_job =
        fetch_upload_job_by_id(state.db.sqlite_adapter(), &creator.id, &created.id).await?;
    let refreshed_session =
        fetch_upload_ingest_session(state.db.sqlite_adapter(), &creator.id, &created.id).await?;
    let upload_path = media_path_for_relative(&state, &refreshed_session.relative_path);
    let file_size = tokio::fs::metadata(upload_path).await?.len();

    assert_eq!(refreshed_job.bytes_received, 0);
    assert_eq!(refreshed_session.bytes_received, 0);
    assert_eq!(file_size, 0);

    Ok(())
}
