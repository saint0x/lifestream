use super::*;

#[tokio::test]
async fn runtime_spec_is_provisioned_and_tracks_live_runtime_transitions() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let auth_token = insert_creator_auth_session(&state.pool, &creator).await?;
    let broadcast = insert_ready_broadcast(&state.pool, &creator).await?;
    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-runtime-spec".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;

    let manifest_relative_path = runtime_manifest_path(&connected.session);
    let archive_relative_path = runtime_archive_path(&connected.session);
    let archive_staging_relative_path = runtime_archive_staging_path(&connected.session);
    let spec_relative_path = runtime_spec_path(&connected.session);
    let spec_full_path = media_path_for_relative(&state, &spec_relative_path);

    let initial_spec: Value = serde_json::from_str(
        &tokio::fs::read_to_string(&spec_full_path)
            .await
            .map_err(AppError::Io)?,
    )?;
    assert_eq!(initial_spec["session"]["id"], connected.session.id);
    assert_eq!(initial_spec["session"]["status"], "connected");
    assert_eq!(initial_spec["session"]["sessionOrdinal"], 1);
    assert_eq!(initial_spec["session"]["reconnectSession"], false);
    assert_eq!(initial_spec["artifactHealth"]["status"], "pending");
    assert_eq!(initial_spec["artifactHealth"]["manifest"]["state"], "pending");
    assert_eq!(initial_spec["artifactHealth"]["archive"]["state"], "pending");
    assert_eq!(initial_spec["runtime"]["state"], "pending_attach");
    assert_eq!(initial_spec["runtime"]["runtimeClass"], "standard_hls");
    assert_eq!(initial_spec["runtime"]["latencyProfile"], "standard");
    assert_eq!(initial_spec["runtime"]["segmentFormat"], "mpegts");
    assert_eq!(initial_spec["runtime"]["partialSegmentsEnabled"], false);
    assert_eq!(initial_spec["runtime"]["blockingReloadEnabled"], false);
    assert_eq!(initial_spec["runtime"]["targetSegmentDurationSec"], 6);
    assert_eq!(initial_spec["runtime"]["holdBackSegments"], 3);
    assert_eq!(initial_spec["runtime"]["discontinuitySequence"], 0);
    assert_eq!(initial_spec["runtime"]["ladderPolicy"], "awaiting_probe");
    assert_eq!(initial_spec["runtime"]["contentClass"], "unknown");
    assert_eq!(
        initial_spec["expectedPaths"]["manifestRelativePath"],
        manifest_relative_path
    );
    assert_eq!(
        initial_spec["expectedPaths"]["archiveRelativePath"],
        archive_relative_path
    );
    assert_eq!(
        initial_spec["expectedPaths"]["specRelativePath"],
        spec_relative_path
    );
    assert_eq!(initial_spec["packaging"]["runtimeClass"], "standard_hls");
    assert_eq!(initial_spec["packaging"]["latencyProfile"], "standard");
    assert_eq!(initial_spec["packaging"]["playlistMode"], "event");
    assert_eq!(initial_spec["packaging"]["segmentFormat"], "mpegts");
    assert_eq!(initial_spec["packaging"]["segmentDurationSec"], 6);
    assert_eq!(initial_spec["packaging"]["partialSegmentsEnabled"], false);
    assert_eq!(initial_spec["packaging"]["blockingReloadEnabled"], false);
    assert_eq!(initial_spec["packaging"]["targetLatencyMs"], 18000);
    assert_eq!(initial_spec["archive"]["enabled"], true);
    assert_eq!(initial_spec["archive"]["recordingMode"], "single_output");
    assert_eq!(initial_spec["archive"]["targetContainer"], "mp4");
    assert_eq!(initial_spec["archive"]["outputCount"], 1);
    assert_eq!(
        initial_spec["archive"]["outputRelativePath"],
        archive_relative_path.clone()
    );
    assert_eq!(
        initial_spec["archive"]["outputRelativePaths"],
        json!([archive_relative_path.clone()])
    );
    assert_eq!(
        initial_spec["reconnectPolicy"]["replacementMode"],
        "new_session_per_reconnect"
    );
    assert_eq!(initial_spec["reconnectPolicy"]["graceWindowSec"], 20);
    assert_eq!(initial_spec["health"]["status"], "critical");
    assert_eq!(initial_spec["health"]["currentCpuPercent"], 0);
    assert_eq!(initial_spec["health"]["currentFreeDiskGb"], 0.0);
    assert_eq!(initial_spec["health"]["cpuWarnPercent"], 85);
    assert_eq!(initial_spec["health"]["cpuCriticalPercent"], 95);
    assert_eq!(initial_spec["health"]["freeDiskWarnGb"], 20.0);
    assert_eq!(initial_spec["health"]["freeDiskCriticalGb"], 5.0);
    assert_eq!(initial_spec["health"]["ingestLatencyWarnMs"], 1500);
    assert_eq!(initial_spec["health"]["ingestLatencyCriticalMs"], 3000);
    assert_eq!(initial_spec["health"]["droppedFramesWarn"], 100);
    assert_eq!(initial_spec["health"]["droppedFramesCritical"], 1000);
    assert_eq!(
        initial_spec["packaging"]["variantStrategy"],
        "awaiting_probe"
    );
    assert_eq!(initial_spec["packaging"]["ladderPolicy"], "awaiting_probe");
    assert_eq!(initial_spec["packaging"]["contentClass"], "unknown");
    assert_eq!(initial_spec["packaging"]["discontinuitySequence"], 0);
    assert_eq!(initial_spec["packaging"]["variants"], json!([]));
    assert!(initial_spec["collaboration"].is_null());
    assert_eq!(
        initial_spec["telemetry"]["heartbeatSampleKind"],
        "heartbeat"
    );
    assert!(
        tokio::fs::try_exists(
            media_path_for_relative(&state, &manifest_relative_path)
                .parent()
                .expect("manifest parent")
        )
        .await?
    );
    assert!(
        tokio::fs::try_exists(
            media_path_for_relative(&state, &archive_relative_path)
                .parent()
                .expect("archive parent")
        )
        .await?
    );
    assert!(
        tokio::fs::try_exists(
            media_path_for_relative(&state, &archive_staging_relative_path)
                .parent()
                .expect("archive staging parent")
        )
        .await?
    );

    let (collaboration_session, collaboration_participant) =
        insert_shared_chat_collaboration_for_current_broadcast(
            &state.pool,
            &creator,
            "crt-atlas",
            "usr-2",
            true,
        )
        .await?;
    let _grant = issue_mirror_grant_for_participant(
        &state,
        &collaboration_session,
        &collaboration_participant,
        &creator.user_id,
    )
    .await?;

    write_test_media_file(
        &state,
        &manifest_relative_path,
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:2\n",
    )
    .await?;
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
            bitrate_kbps: 7200,
            viewers: 88,
            dropped_frames: 3,
            cpu_percent: Some(18),
            free_disk_gb: Some(410.5),
            ingest_latency_ms: Some(480),
            source_probe: Some(crate::models::LiveSourceProbeInput {
                container_format: Some("mpegts".to_string()),
                video_codec: Some("h264".to_string()),
                audio_codec: Some("aac".to_string()),
                width: Some(1920),
                height: Some(1080),
                frame_rate: Some(59.94),
                audio_sample_rate_hz: Some(48_000),
                audio_channels: Some(2),
            }),
        }),
    )
    .await?;

    let heartbeat_spec: Value = serde_json::from_str(
        &tokio::fs::read_to_string(&spec_full_path)
            .await
            .map_err(AppError::Io)?,
    )?;
    assert_eq!(heartbeat_spec["session"]["contributionState"], "healthy");
    assert_eq!(heartbeat_spec["session"]["ingestLatencyMs"], 480);
    assert_eq!(
        heartbeat_spec["session"]["sourceProbe"]["videoCodec"],
        "h264"
    );
    assert_eq!(heartbeat_spec["artifactHealth"]["status"], "pending");
    assert_eq!(heartbeat_spec["runtime"]["state"], "pending_attach");
    assert_eq!(
        heartbeat_spec["collaboration"]["sessionId"],
        collaboration_session.id
    );
    assert_eq!(heartbeat_spec["collaboration"]["mixMinusRequired"], true);
    assert_eq!(heartbeat_spec["collaboration"]["audioMixMode"], "mix_minus");

    let _ = report_live_runtime(
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
    .await?;

    let ready_spec: Value = serde_json::from_str(
        &tokio::fs::read_to_string(&spec_full_path)
            .await
            .map_err(AppError::Io)?,
    )?;
    assert_eq!(ready_spec["runtime"]["state"], "healthy");
    assert_eq!(ready_spec["runtime"]["packagingStatus"], "ready");
    assert_eq!(ready_spec["artifactHealth"]["status"], "checked");
    assert_eq!(ready_spec["artifactHealth"]["manifest"]["state"], "valid");
    assert_eq!(ready_spec["artifactHealth"]["archive"]["state"], "pending");
    assert_eq!(ready_spec["runtime"]["runtimeClass"], "standard_hls");
    assert_eq!(ready_spec["runtime"]["latencyProfile"], "standard");
    assert_eq!(ready_spec["runtime"]["segmentFormat"], "mpegts");
    assert_eq!(ready_spec["runtime"]["ladderPolicy"], "probe_high_motion_1080p");
    assert_eq!(ready_spec["runtime"]["contentClass"], "high_motion");
    assert_eq!(
        ready_spec["runtime"]["manifestRelativePath"],
        manifest_relative_path
    );
    assert_eq!(ready_spec["health"]["status"], "ok");
    assert_eq!(ready_spec["health"]["currentCpuPercent"], 18);
    assert_eq!(ready_spec["health"]["currentFreeDiskGb"], 410.5);
    assert_eq!(ready_spec["health"]["currentIngestLatencyMs"], 480);
    assert_eq!(ready_spec["health"]["currentDroppedFrames"], 3);
    assert_eq!(ready_spec["packaging"]["status"], "ready");
    assert_eq!(ready_spec["packaging"]["latencyProfile"], "standard");
    assert_eq!(ready_spec["packaging"]["segmentFormat"], "mpegts");
    assert_eq!(ready_spec["packaging"]["variantStrategy"], "probe_derived");
    assert_eq!(ready_spec["packaging"]["ladderPolicy"], "probe_high_motion_1080p");
    assert_eq!(ready_spec["packaging"]["contentClass"], "high_motion");
    assert_eq!(ready_spec["packaging"]["variants"][0]["label"], "240p");
    assert_eq!(ready_spec["packaging"]["variants"][4]["label"], "1080p");
    assert_eq!(
        ready_spec["collaboration"]["sessionId"],
        collaboration_session.id
    );
    assert_eq!(ready_spec["collaboration"]["status"], "active");
    assert_eq!(
        ready_spec["collaboration"]["sourceBroadcastId"],
        connected.session.broadcast_id
    );
    assert_eq!(ready_spec["collaboration"]["chatMode"], "shared");
    assert_eq!(
        ready_spec["collaboration"]["recordingPolicy"],
        "host_archive"
    );
    assert_eq!(ready_spec["archive"]["recordingMode"], "host_archive");
    assert_eq!(ready_spec["archive"]["outputCount"], 1);
    assert_eq!(ready_spec["collaboration"]["sharedChat"], true);
    assert_eq!(ready_spec["collaboration"]["mixMinusRequired"], true);
    assert_eq!(ready_spec["collaboration"]["audioMixMode"], "mix_minus");
    assert_eq!(
        ready_spec["telemetry"]["runtimeReportSampleKind"],
        "runtime_report"
    );
    assert_eq!(
        ready_spec["collaboration"]["mirroredCreatorIds"][0],
        "crt-atlas"
    );
    assert!(
        ready_spec["collaboration"]["outputs"]
            .as_array()
            .expect("collaboration outputs array")
            .iter()
            .any(|output| {
                output["outputKind"] == "host_channel"
                    && output["routeState"] == "active"
                    && output["targetBroadcastId"] == connected.session.broadcast_id
            })
    );
    assert!(
        ready_spec["collaboration"]["outputs"]
            .as_array()
            .expect("collaboration outputs array")
            .iter()
            .any(|output| {
                output["outputKind"] == "mirror_channel"
                    && output["routeState"] == "issued"
                    && output["targetCreatorId"] == "crt-atlas"
                    && output["mixMinusRequired"] == true
            })
    );
    assert!(
        ready_spec["collaboration"]["programs"]
            .as_array()
            .expect("collaboration programs array")
            .iter()
            .any(|program| {
                program["programKind"] == "host_program"
                    && program["outputIds"]
                        .as_array()
                        .is_some_and(|outputs| {
                            outputs.iter().any(|output_id| {
                                output_id
                                    == &Value::String(format!(
                                        "col-out-host-{}",
                                        collaboration_session.id
                                    ))
                            })
                        })
            })
    );
    assert!(
        ready_spec["collaboration"]["audio"]
            .as_array()
            .expect("collaboration audio routes array")
            .iter()
            .any(|route| {
                route["participantId"] == collaboration_participant.id
                    && route["routeKind"] == "mix_minus_return"
                    && route["receiveProgramAudio"] == true
                    && route["excludedParticipantIds"] == json!([collaboration_participant.id])
            })
    );
    assert!(
        ready_spec["collaboration"]["contributions"]
            .as_array()
            .expect("collaboration contributions array")
            .iter()
            .any(|contribution| {
                contribution["participantId"] == collaboration_participant.id
                    && contribution["mixMinusRequired"] == true
                    && contribution["attachedOutputIds"]
                        .as_array()
                        .is_some_and(|outputs| {
                            outputs.iter().any(|output| {
                                output
                                    == &Value::String(format!(
                                        "col-out-mirror-{}",
                                        collaboration_participant.id
                                    ))
                            })
                        })
            })
    );
    let runtime = fetch_creator_live_runtime_response(&state.pool, &creator.id).await?;
    assert!(runtime.telemetry_summary.probe_samples >= 1);
    assert!(runtime.telemetry_summary.collaboration_samples >= 1);
    assert!(runtime.telemetry_summary.mix_minus_samples >= 1);
    assert!(runtime.telemetry_summary.packaging_ready_samples >= 1);
    assert_eq!(
        runtime
            .telemetry_summary
            .last_collaboration_session_id
            .as_deref(),
        Some(collaboration_session.id.as_str())
    );
    assert_eq!(
        runtime
            .telemetry_summary
            .last_collaboration_participant_count,
        Some(2)
    );
    assert_eq!(runtime.telemetry_summary.last_active_output_routes, Some(1));
    assert_eq!(
        runtime.telemetry_summary.last_audio_mix_mode.as_deref(),
        Some("mix_minus")
    );
    assert_eq!(runtime.telemetry_summary.ll_hls_samples, 0);
    assert_eq!(runtime.telemetry_summary.peak_discontinuity_sequence, 0);
    assert_eq!(
        runtime
            .artifact_health
            .as_ref()
            .map(|health| health.manifest.state.as_str()),
        Some("declared")
    );
    assert_eq!(
        runtime.telemetry_summary.last_runtime_class.as_deref(),
        Some("standard_hls")
    );
    assert_eq!(
        runtime.telemetry_summary.last_latency_profile.as_deref(),
        Some("standard")
    );
    assert_eq!(
        runtime.telemetry_summary.last_ladder_policy.as_deref(),
        Some("probe_high_motion_1080p")
    );
    assert_eq!(
        runtime.telemetry_summary.last_content_class.as_deref(),
        Some("high_motion")
    );
    let playback_enabled_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| target.playback_enabled)
        .count() as i64;
    let recording_enabled_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| target.recording_enabled)
        .count() as i64;
    let variant_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| target.target_kind == "variant")
        .count() as i64;
    let collaboration_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| {
            matches!(
                target.target_kind.as_str(),
                "host_channel" | "mirror_channel" | "archive"
            )
        })
        .count() as i64;
    let host_channel_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| target.target_kind == "host_channel")
        .count() as i64;
    let mirror_channel_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| target.target_kind == "mirror_channel")
        .count() as i64;
    let shared_program_mirror_channel_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| {
            target.target_kind == "mirror_channel"
                && target.mix_minus_required
                && target.source_participant_ids.len() > 1
        })
        .count() as i64;
    let guest_isolated_mirror_channel_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| {
            target.target_kind == "mirror_channel"
                && !(target.mix_minus_required && target.source_participant_ids.len() > 1)
        })
        .count() as i64;
    let archive_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| target.target_kind == "archive")
        .count() as i64;
    let active_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| target.route_state == "active")
        .count() as i64;
    let degraded_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| target.route_state == "degraded")
        .count() as i64;
    let armed_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| target.route_state == "armed")
        .count() as i64;
    let pending_source_targets = runtime
        .active_runtime_targets
        .iter()
        .filter(|target| target.route_state == "pending_source")
        .count() as i64;
    assert_eq!(
        runtime.telemetry_summary.peak_runtime_target_count,
        runtime.active_runtime_targets.len() as i64
    );
    assert_eq!(
        runtime.telemetry_summary.peak_playback_target_count,
        playback_enabled_targets
    );
    assert_eq!(
        runtime.telemetry_summary.peak_recording_target_count,
        recording_enabled_targets
    );
    assert_eq!(
        runtime.telemetry_summary.peak_variant_target_count,
        variant_targets
    );
    assert_eq!(
        runtime.telemetry_summary.peak_collaboration_target_count,
        collaboration_targets
    );
    assert_eq!(
        runtime.telemetry_summary.peak_host_channel_count,
        host_channel_targets
    );
    assert_eq!(
        runtime.telemetry_summary.peak_mirror_channel_count,
        mirror_channel_targets
    );
    assert_eq!(
        runtime
            .telemetry_summary
            .peak_shared_program_mirror_channel_count,
        shared_program_mirror_channel_targets
    );
    assert_eq!(
        runtime
            .telemetry_summary
            .peak_guest_isolated_mirror_channel_count,
        guest_isolated_mirror_channel_targets
    );
    assert_eq!(
        runtime.telemetry_summary.peak_archive_target_count,
        archive_targets
    );
    assert_eq!(
        runtime.telemetry_summary.peak_active_target_count,
        active_targets
    );
    assert_eq!(
        runtime.telemetry_summary.peak_degraded_target_count,
        degraded_targets
    );
    assert_eq!(
        runtime.telemetry_summary.peak_armed_target_count,
        armed_targets
    );
    assert_eq!(
        runtime.telemetry_summary.peak_pending_source_target_count,
        pending_source_targets
    );
    assert_eq!(
        runtime.telemetry_summary.last_runtime_target_count,
        Some(runtime.active_runtime_targets.len() as i64)
    );
    assert_eq!(
        runtime.telemetry_summary.last_playback_target_count,
        Some(playback_enabled_targets)
    );
    assert_eq!(
        runtime.telemetry_summary.last_recording_target_count,
        Some(recording_enabled_targets)
    );
    assert_eq!(
        runtime.telemetry_summary.last_variant_target_count,
        Some(variant_targets)
    );
    assert_eq!(
        runtime.telemetry_summary.last_collaboration_target_count,
        Some(collaboration_targets)
    );
    assert_eq!(
        runtime.telemetry_summary.last_host_channel_count,
        Some(host_channel_targets)
    );
    assert_eq!(
        runtime.telemetry_summary.last_mirror_channel_count,
        Some(mirror_channel_targets)
    );
    assert_eq!(
        runtime
            .telemetry_summary
            .last_shared_program_mirror_channel_count,
        Some(shared_program_mirror_channel_targets)
    );
    assert_eq!(
        runtime
            .telemetry_summary
            .last_guest_isolated_mirror_channel_count,
        Some(guest_isolated_mirror_channel_targets)
    );
    assert_eq!(
        runtime.telemetry_summary.last_archive_target_count,
        Some(archive_targets)
    );
    assert_eq!(
        runtime.telemetry_summary.last_active_target_count,
        Some(active_targets)
    );
    assert_eq!(
        runtime.telemetry_summary.last_degraded_target_count,
        Some(degraded_targets)
    );
    assert_eq!(
        runtime.telemetry_summary.last_armed_target_count,
        Some(armed_targets)
    );
    assert_eq!(
        runtime.telemetry_summary.last_pending_source_target_count,
        Some(pending_source_targets)
    );
    assert!(runtime.recent_telemetry.iter().any(|sample| {
        sample.sample_kind == "runtime_report"
            && sample.detail["collaboration"]["present"] == true
            && sample.detail["collaboration"]["sessionId"] == collaboration_session.id
            && sample.detail["outputs"]["activeRouteCount"] == 1
            && sample.detail["delivery"]["runtimeClass"] == "standard_hls"
            && sample.detail["delivery"]["segmentFormat"] == "mpegts"
            && sample.detail["delivery"]["ladderPolicy"] == "probe_high_motion_1080p"
            && sample.detail["targets"]["count"] == runtime.active_runtime_targets.len() as i64
            && sample.detail["targets"]["variantCount"] == variant_targets
            && sample.detail["targets"]["playbackEnabledCount"] == playback_enabled_targets
            && sample.detail["targets"]["recordingEnabledCount"] == recording_enabled_targets
            && sample.detail["targets"]["collaborationCount"] == collaboration_targets
            && sample.detail["targets"]["hostChannelCount"] == host_channel_targets
            && sample.detail["targets"]["mirrorChannelCount"] == mirror_channel_targets
            && sample.detail["targets"]["sharedProgramMirrorChannelCount"]
                == shared_program_mirror_channel_targets
            && sample.detail["targets"]["guestIsolatedMirrorChannelCount"]
                == guest_isolated_mirror_channel_targets
            && sample.detail["targets"]["archiveCount"] == archive_targets
            && sample.detail["targets"]["activeCount"] == active_targets
            && sample.detail["targets"]["degradedCount"] == degraded_targets
            && sample.detail["targets"]["armedCount"] == armed_targets
            && sample.detail["targets"]["pendingSourceCount"] == pending_source_targets
    }));
    let expected_variant_playlist = format!(
        "live/{}/{}/{}/1080p/playlist.m3u8",
        connected.session.creator_id, connected.session.broadcast_id, connected.session.id
    );
    let expected_mirror_playlist = format!(
        "live/{}/{}/col-out-mirror-{}/master.m3u8",
        collaboration_participant
            .creator_id
            .as_deref()
            .expect("guest creator id"),
        collaboration_session.source_broadcast_id,
        collaboration_participant.id
    );
    let expected_host_route_archive = format!(
        "archive/{}/{}/col-out-archive-host-{}/final.mp4",
        creator.id, collaboration_session.source_broadcast_id, collaboration_session.id
    );
    assert_eq!(
        ready_spec["archive"]["outputRelativePath"],
        expected_host_route_archive.clone()
    );
    assert_eq!(
        ready_spec["archive"]["outputRelativePaths"],
        json!([expected_host_route_archive.clone()])
    );
    assert!(runtime.active_runtime_targets.iter().any(|target| {
        target.target_kind == "variant"
            && target.target_key == "1080p"
            && target.relative_path.as_deref() == Some(expected_variant_playlist.as_str())
    }));
    assert!(runtime.active_runtime_targets.iter().any(|target| {
        target.target_kind == "host_channel"
            && target.route_state == "active"
            && target.relative_path.as_deref() == Some(manifest_relative_path.as_str())
            && target.target_broadcast_id.as_deref() == Some(connected.session.broadcast_id.as_str())
    }));
    assert!(runtime.active_runtime_targets.iter().any(|target| {
        target.target_kind == "mirror_channel"
            && target.target_key == format!("col-out-mirror-{}", collaboration_participant.id)
            && target.target_creator_id.as_deref()
                == collaboration_participant.creator_id.as_deref()
            && target.relative_path.as_deref() == Some(expected_mirror_playlist.as_str())
            && target.mix_minus_required
    }));
    assert!(runtime.active_runtime_targets.iter().any(|target| {
        target.target_kind == "archive"
            && target.target_key == format!("col-out-archive-host-{}", collaboration_session.id)
            && target.relative_path.as_deref() == Some(expected_host_route_archive.as_str())
    }));
    assert!(!runtime.active_runtime_targets.iter().any(|target| {
        target.target_kind == "archive" && target.target_key == "primary"
    }));
    assert!(
        tokio::fs::metadata(media_path_for_relative(&state, &expected_mirror_playlist))
            .await
            .map_err(AppError::Io)?
            .len()
            > 0
    );
    assert!(
        tokio::fs::metadata(media_path_for_relative(&state, &expected_host_route_archive))
            .await
            .map_err(AppError::Io)?
            .len()
            > 0
    );
    let record =
        fetch_creator_live_ingest_session_record(&state.pool, &creator.id, &connected.session.id)
            .await?;
    assert!(
        record
            .recent_events
            .iter()
            .any(|event| event.event_type == "runtime_targets_synced")
    );
    assert_eq!(
        ready_spec["packaging"]["variants"][4]["outputRelativeDir"],
        format!(
            "live/{}/{}/{}/1080p",
            connected.session.creator_id, connected.session.broadcast_id, connected.session.id
        )
    );
    assert_eq!(
        ready_spec["packaging"]["variants"][4]["relativePlaylistPath"],
        format!(
            "live/{}/{}/{}/1080p/playlist.m3u8",
            connected.session.creator_id, connected.session.broadcast_id, connected.session.id
        )
    );
    assert_eq!(
        ready_spec["packaging"]["variants"][4]["segmentRelativePattern"],
        format!(
            "live/{}/{}/{}/1080p/segment_%03d.ts",
            connected.session.creator_id, connected.session.broadcast_id, connected.session.id
        )
    );
    assert_eq!(
        ready_spec["archive"]["stagingRelativePath"],
        archive_staging_relative_path
    );
    assert_eq!(ready_spec["archive"]["status"], "not_started");
    assert!(
        tokio::fs::try_exists(
            media_path_for_relative(
                &state,
                ready_spec["packaging"]["variants"][4]["relativePlaylistPath"]
                    .as_str()
                    .expect("playlist path in spec"),
            )
            .parent()
            .expect("variant playlist parent")
        )
        .await?
    );

    let _ = terminate_creator_live_ingest(
        State(state.clone()),
        auth_headers(&auth_token),
        Path(connected.session.id.clone()),
        Json(TerminateLiveIngestRequest {
            reason: Some("runtime spec sync validation".to_string()),
        }),
    )
    .await?;

    let terminal_spec: Value = serde_json::from_str(
        &tokio::fs::read_to_string(&spec_full_path)
            .await
            .map_err(AppError::Io)?,
    )?;
    assert_eq!(terminal_spec["session"]["status"], "terminated");
    assert_eq!(terminal_spec["runtime"]["state"], "disconnected");
    assert_eq!(terminal_spec["runtime"]["packagingStatus"], "ready");
    assert_eq!(terminal_spec["archive"]["status"], "not_started");
    assert_eq!(
        terminal_spec["expectedPaths"]["archiveRelativePath"],
        archive_relative_path
    );

    Ok(())
}

