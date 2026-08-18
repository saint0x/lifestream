use super::*;

#[tokio::test]
async fn ll_hls_delivery_profile_persists_into_runtime_output_spec_and_telemetry() -> AppResult<()>
{
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    sqlx::query("UPDATE creator_live_settings SET delivery_class = 'll_hls' WHERE creator_id = ?")
        .bind(&creator.id)
        .execute(&state.pool)
        .await?;

    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-ll-hls".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;

    let spec_relative_path = runtime_spec_path(&connected.session);
    let spec_full_path = media_path_for_relative(&state, &spec_relative_path);
    let manifest_relative_path = runtime_manifest_path(&connected.session);
    let variant_playlist_relative_path = format!(
        "live/{}/{}/{}/720p/playlist.m3u8",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );
    let init_relative_path = format!(
        "live/{}/{}/{}/720p/init.mp4",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );
    let part_relative_path = format!(
        "live/{}/{}/{}/720p/part_000_000.m4s",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );
    let segment_relative_path = format!(
        "live/{}/{}/{}/720p/segment_000.m4s",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );
    let mut ingest_headers = HeaderMap::new();
    ingest_headers.insert(
        "x-ingest-token",
        HeaderValue::from_str(&connected.ingest_token).unwrap(),
    );
    let _ = heartbeat_live_ingest(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers.clone(),
        Json(IngestHeartbeatRequest {
            bitrate_kbps: 5400,
            viewers: 21,
            dropped_frames: 0,
            cpu_percent: Some(12),
            free_disk_gb: Some(320.0),
            ingest_latency_ms: Some(220),
            source_probe: Some(crate::models::LiveSourceProbeInput {
                container_format: Some("mpegts".to_string()),
                video_codec: Some("h264".to_string()),
                audio_codec: Some("aac".to_string()),
                width: Some(1280),
                height: Some(720),
                frame_rate: Some(30.0),
                audio_sample_rate_hz: Some(48_000),
                audio_channels: Some(2),
            }),
        }),
    )
    .await?;

    let output = report_live_runtime(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers,
        Json(UpdateLiveRuntimeStateRequest {
            runtime_state: "healthy".to_string(),
            packaging_status: "ready".to_string(),
            archive_status: "not_started".to_string(),
            manifest_relative_path: Some(manifest_relative_path.clone()),
            archive_relative_path: None,
            last_error: None,
        }),
    )
    .await?
    .0;

    assert_eq!(output.runtime_class, "ll_hls");
    assert_eq!(output.latency_profile, "low");
    assert_eq!(output.segment_format, "fmp4");
    assert!(output.partial_segments_enabled);
    assert!(output.blocking_reload_enabled);
    assert_eq!(output.target_segment_duration_sec, 2);
    assert_eq!(output.hold_back_segments, 2);
    assert_eq!(output.ladder_policy, "probe_general_hd");
    assert_eq!(output.content_class, "general_hd");
    let manifest = tokio::fs::read_to_string(media_path_for_relative(&state, &manifest_relative_path))
        .await
        .map_err(AppError::Io)?;
    let variant_playlist = tokio::fs::read_to_string(media_path_for_relative(
        &state,
        &variant_playlist_relative_path,
    ))
    .await
    .map_err(AppError::Io)?;
    assert!(manifest.contains("#EXTM3U"));
    assert!(manifest.contains("720p/playlist.m3u8"));
    assert!(variant_playlist.contains("#EXT-X-SERVER-CONTROL:CAN-BLOCK-RELOAD=YES"));
    assert!(variant_playlist.contains("#EXT-X-PART:"));
    assert!(variant_playlist.contains("segment_000.m4s"));
    assert!(
        tokio::fs::metadata(media_path_for_relative(&state, &init_relative_path))
            .await
            .map_err(AppError::Io)?
            .len()
            > 0
    );
    assert!(
        tokio::fs::metadata(media_path_for_relative(&state, &part_relative_path))
            .await
            .map_err(AppError::Io)?
            .len()
            > 0
    );
    assert!(
        tokio::fs::metadata(media_path_for_relative(&state, &segment_relative_path))
            .await
            .map_err(AppError::Io)?
            .len()
            > 0
    );

    let spec: Value = serde_json::from_str(
        &tokio::fs::read_to_string(&spec_full_path)
            .await
            .map_err(AppError::Io)?,
    )?;
    assert_eq!(spec["runtime"]["runtimeClass"], "ll_hls");
    assert_eq!(spec["runtime"]["latencyProfile"], "low");
    assert_eq!(spec["runtime"]["segmentFormat"], "fmp4");
    assert_eq!(spec["runtime"]["partialSegmentsEnabled"], true);
    assert_eq!(spec["runtime"]["blockingReloadEnabled"], true);
    assert_eq!(spec["runtime"]["targetSegmentDurationSec"], 2);
    assert_eq!(spec["runtime"]["holdBackSegments"], 2);
    assert_eq!(spec["packaging"]["runtimeClass"], "ll_hls");
    assert_eq!(spec["packaging"]["latencyProfile"], "low");
    assert_eq!(spec["packaging"]["segmentFormat"], "fmp4");
    assert_eq!(spec["packaging"]["partialSegmentsEnabled"], true);
    assert_eq!(spec["packaging"]["blockingReloadEnabled"], true);
    assert_eq!(spec["packaging"]["targetLatencyMs"], 4000);
    assert_eq!(spec["packaging"]["ladderPolicy"], "probe_general_hd");
    assert_eq!(spec["packaging"]["contentClass"], "general_hd");
    assert_eq!(
        spec["packaging"]["variants"][3]["segmentRelativePattern"],
        format!(
            "live/{}/{}/{}/720p/segment_%03d.m4s",
            connected.session.creator_id, connected.session.broadcast_id, connected.session.id
        )
    );

    let runtime = fetch_creator_live_runtime_response(&state.pool, &creator.id).await?;
    assert!(runtime.telemetry_summary.ll_hls_samples >= 1);
    assert_eq!(runtime.telemetry_summary.peak_discontinuity_sequence, 0);
    assert_eq!(
        runtime.telemetry_summary.last_runtime_class.as_deref(),
        Some("ll_hls")
    );
    assert_eq!(
        runtime.telemetry_summary.last_latency_profile.as_deref(),
        Some("low")
    );
    assert_eq!(
        runtime.telemetry_summary.last_ladder_policy.as_deref(),
        Some("probe_general_hd")
    );
    assert_eq!(
        runtime.telemetry_summary.last_content_class.as_deref(),
        Some("general_hd")
    );
    assert!(runtime.recent_telemetry.iter().any(|sample| {
        sample.sample_kind == "runtime_report"
            && sample.detail["delivery"]["runtimeClass"] == "ll_hls"
            && sample.detail["delivery"]["segmentFormat"] == "fmp4"
            && sample.detail["delivery"]["partialSegmentsEnabled"] == true
            && sample.detail["delivery"]["ladderPolicy"] == "probe_general_hd"
    }));

    Ok(())
}

#[tokio::test]
async fn live_media_playlist_rewrites_tokenized_ll_hls_uris_and_honors_blocking_reload()
-> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    sqlx::query("UPDATE creator_live_settings SET delivery_class = 'll_hls' WHERE creator_id = ?")
        .bind(&creator.id)
        .execute(&state.pool)
        .await?;

    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-live-playlist-blocking".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;

    let mut ingest_headers = HeaderMap::new();
    ingest_headers.insert(
        "x-ingest-token",
        HeaderValue::from_str(&connected.ingest_token).unwrap(),
    );
    let manifest_relative_path = runtime_manifest_path(&connected.session);
    let variant_playlist_relative_path = format!(
        "live/{}/{}/{}/720p/playlist.m3u8",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );
    let variant_playlist_path = media_path_for_relative(&state, &variant_playlist_relative_path);
    let stream_id = format!("lv-{}-live", creator.handle);

    let _ = heartbeat_live_ingest(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers.clone(),
        Json(IngestHeartbeatRequest {
            bitrate_kbps: 5400,
            viewers: 42,
            dropped_frames: 0,
            cpu_percent: Some(11),
            free_disk_gb: Some(280.0),
            ingest_latency_ms: Some(240),
            source_probe: Some(crate::models::LiveSourceProbeInput {
                container_format: Some("mpegts".to_string()),
                video_codec: Some("h264".to_string()),
                audio_codec: Some("aac".to_string()),
                width: Some(1280),
                height: Some(720),
                frame_rate: Some(30.0),
                audio_sample_rate_hz: Some(48_000),
                audio_channels: Some(2),
            }),
        }),
    )
    .await?;
    let _ = report_live_runtime(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers,
        Json(UpdateLiveRuntimeStateRequest {
            runtime_state: "healthy".to_string(),
            packaging_status: "ready".to_string(),
            archive_status: "not_started".to_string(),
            manifest_relative_path: Some(manifest_relative_path),
            archive_relative_path: None,
            last_error: None,
        }),
    )
    .await?;

    let playback = create_live_playback_session(
        State(state.clone()),
        HeaderMap::new(),
        Path(stream_id),
    )
    .await?
    .0;

    let first_response = serve_media_file(
        State(state.clone()),
        Path(variant_playlist_relative_path.clone()),
        HeaderMap::new(),
        Query(PlaybackAccessQuery {
            playback_token: Some(playback.playback_token.clone()),
            hls_msn: Some(0),
            hls_part: Some(0),
        }),
    )
    .await?;
    let first_body = axum::body::to_bytes(first_response.into_body(), usize::MAX)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let first_text = String::from_utf8(first_body.to_vec())
        .map_err(|error| AppError::Internal(error.to_string()))?;
    assert!(first_text.contains(&format!(
        "init.mp4?playbackToken={}",
        playback.playback_token
    )));
    assert!(first_text.contains(&format!(
        "part_000_000.m4s?playbackToken={}",
        playback.playback_token
    )));
    assert!(first_text.contains(&format!(
        "segment_000.m4s?playbackToken={}",
        playback.playback_token
    )));

    let state_for_update = state.clone();
    let playlist_for_update = variant_playlist_path.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let body = concat!(
            "#EXTM3U\n",
            "#EXT-X-VERSION:9\n",
            "#EXT-X-TARGETDURATION:2\n",
            "#EXT-X-PLAYLIST-TYPE:EVENT\n",
            "#EXT-X-MEDIA-SEQUENCE:1\n",
            "#EXT-X-DISCONTINUITY-SEQUENCE:0\n",
            "#EXT-X-INDEPENDENT-SEGMENTS\n",
            "#EXT-X-MAP:URI=\"init.mp4\"\n",
            "#EXT-X-SERVER-CONTROL:CAN-BLOCK-RELOAD=YES,PART-HOLD-BACK=2.000\n",
            "#EXT-X-PART-INF:PART-TARGET=1.000\n",
            "#EXT-X-PART:DURATION=1.000,URI=\"part_001_000.m4s\",INDEPENDENT=YES\n",
            "#EXTINF:2.000,\n",
            "segment_001.m4s\n"
        );
        let _ = tokio::fs::write(&playlist_for_update, body).await;
        let _ = tokio::fs::write(
            media_path_for_relative(
                &state_for_update,
                &format!(
                    "live/{}/{}/{}/720p/part_001_000.m4s",
                    connected.session.creator_id, connected.session.broadcast_id, connected.session.id
                ),
            ),
            b"updated-part",
        )
        .await;
        let _ = tokio::fs::write(
            media_path_for_relative(
                &state_for_update,
                &format!(
                    "live/{}/{}/{}/720p/segment_001.m4s",
                    connected.session.creator_id, connected.session.broadcast_id, connected.session.id
                ),
            ),
            b"updated-segment",
        )
        .await;
    });

    let started = tokio::time::Instant::now();
    let blocked_response = serve_media_file(
        State(state.clone()),
        Path(variant_playlist_relative_path),
        HeaderMap::new(),
        Query(PlaybackAccessQuery {
            playback_token: Some(playback.playback_token.clone()),
            hls_msn: Some(1),
            hls_part: Some(0),
        }),
    )
    .await?;
    assert!(started.elapsed() >= Duration::from_millis(200));
    let blocked_body = axum::body::to_bytes(blocked_response.into_body(), usize::MAX)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let blocked_text = String::from_utf8(blocked_body.to_vec())
        .map_err(|error| AppError::Internal(error.to_string()))?;
    assert!(blocked_text.contains("#EXT-X-MEDIA-SEQUENCE:1"));
    assert!(blocked_text.contains(&format!(
        "part_001_000.m4s?playbackToken={}",
        playback.playback_token
    )));
    assert!(blocked_text.contains(&format!(
        "segment_001.m4s?playbackToken={}",
        playback.playback_token
    )));

    Ok(())
}

#[tokio::test]
async fn runtime_report_emits_backend_owned_hls_and_archive_artifacts() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-runtime-owned-artifacts".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;

    let manifest_relative_path = runtime_manifest_path(&connected.session);
    let archive_relative_path = runtime_archive_path(&connected.session);
    let staging_relative_path = runtime_archive_staging_path(&connected.session);
    let variant_playlist_relative_path = format!(
        "live/{}/{}/{}/1080p/playlist.m3u8",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );
    let segment_relative_path = format!(
        "live/{}/{}/{}/1080p/segment_000.ts",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );
    let mut ingest_headers = HeaderMap::new();
    ingest_headers.insert(
        "x-ingest-token",
        HeaderValue::from_str(&connected.ingest_token).unwrap(),
    );

    let _ = heartbeat_live_ingest(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers.clone(),
        Json(IngestHeartbeatRequest {
            bitrate_kbps: 7100,
            viewers: 144,
            dropped_frames: 1,
            cpu_percent: Some(14),
            free_disk_gb: Some(480.0),
            ingest_latency_ms: Some(310),
            source_probe: Some(crate::models::LiveSourceProbeInput {
                container_format: Some("mpegts".to_string()),
                video_codec: Some("h264".to_string()),
                audio_codec: Some("aac".to_string()),
                width: Some(1920),
                height: Some(1080),
                frame_rate: Some(60.0),
                audio_sample_rate_hz: Some(48_000),
                audio_channels: Some(2),
            }),
        }),
    )
    .await?;

    let output = report_live_runtime(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers.clone(),
        Json(UpdateLiveRuntimeStateRequest {
            runtime_state: "healthy".to_string(),
            packaging_status: "ready".to_string(),
            archive_status: "not_started".to_string(),
            manifest_relative_path: Some(manifest_relative_path.clone()),
            archive_relative_path: None,
            last_error: None,
        }),
    )
    .await?
    .0;
    assert_eq!(output.packaging_status, "ready");

    let manifest = tokio::fs::read_to_string(media_path_for_relative(&state, &manifest_relative_path))
        .await
        .map_err(AppError::Io)?;
    let variant_playlist = tokio::fs::read_to_string(media_path_for_relative(
        &state,
        &variant_playlist_relative_path,
    ))
    .await
    .map_err(AppError::Io)?;
    assert!(manifest.contains("1080p/playlist.m3u8"));
    assert!(variant_playlist.contains("#EXT-X-DISCONTINUITY-SEQUENCE:0"));
    assert!(variant_playlist.contains("segment_000.ts"));
    assert!(
        tokio::fs::metadata(media_path_for_relative(&state, &segment_relative_path))
            .await
            .map_err(AppError::Io)?
            .len()
            > 0
    );

    let archive_output = report_live_runtime(
        State(state.clone()),
        Path(connected.session.id.clone()),
        ingest_headers,
        Json(UpdateLiveRuntimeStateRequest {
            runtime_state: "archive_finalizing".to_string(),
            packaging_status: "ready".to_string(),
            archive_status: "finalizing".to_string(),
            manifest_relative_path: Some(manifest_relative_path.clone()),
            archive_relative_path: Some(archive_relative_path.clone()),
            last_error: None,
        }),
    )
    .await?
    .0;

    assert_eq!(archive_output.runtime_state, "archive_complete");
    assert_eq!(archive_output.archive_status, "complete");
    assert_eq!(
        archive_output.archive_relative_path.as_deref(),
        Some(archive_relative_path.as_str())
    );
    assert!(
        tokio::fs::metadata(media_path_for_relative(&state, &archive_relative_path))
            .await
            .map_err(AppError::Io)?
            .len()
            > 0
    );
    assert!(
        tokio::fs::metadata(media_path_for_relative(&state, &staging_relative_path))
            .await
            .map_err(AppError::Io)?
            .len()
            > 0
    );

    Ok(())
}

