use super::*;

#[tokio::test]
async fn ending_broadcast_with_stale_ingest_records_terminal_ingest_event() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);
    let broadcast = insert_ready_broadcast(state.db.sqlite_adapter(), &creator).await?;

    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-a".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;

    sqlx::query(
        "UPDATE live_ingest_sessions SET status = 'stale', disconnected_at = ?, last_heartbeat_at = ? WHERE id = ?",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(Utc::now().to_rfc3339())
    .bind(&connected.session.id)
    .execute(state.db.sqlite_adapter())
    .await?;
    sqlx::query(
        "UPDATE broadcasts SET status = 'ready', ended_at = NULL, duration_sec = NULL WHERE id = ?",
    )
    .bind(&broadcast.id)
    .execute(state.db.sqlite_adapter())
    .await?;
    sqlx::query(
        "UPDATE creator_profiles SET live_status = 'ready', current_broadcast_id = ? WHERE id = ?",
    )
    .bind(&broadcast.id)
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;
    sqlx::query("UPDATE streamers SET is_live = 0 WHERE handle = ?")
        .bind(&creator.handle)
        .execute(state.db.sqlite_adapter())
        .await?;
    sqlx::query("DELETE FROM live_streams WHERE id = ?")
        .bind(format!("lv-{}-live", creator.handle))
        .execute(state.db.sqlite_adapter())
        .await?;

    let before = live_ingest_event_count_for_session(
        state.db.sqlite_adapter(),
        &connected.session.id,
        "creator_broadcast_ended",
    )
    .await?;

    let ended = end_broadcast(State(state.clone()), headers, Path(broadcast.id.clone()))
        .await?
        .0;
    assert_eq!(ended.status, "ended");

    let after = live_ingest_event_count_for_session(
        state.db.sqlite_adapter(),
        &connected.session.id,
        "creator_broadcast_ended",
    )
    .await?;
    assert_eq!(after, before + 1);

    let events =
        fetch_live_ingest_events_for_session(state.db.sqlite_adapter(), &connected.session.id, 20)
            .await?;
    assert!(events.iter().any(|event| {
        event.event_type == "creator_broadcast_ended"
            && event.payload["details"]["actorUserId"] == Value::String("usr-1".to_string())
    }));

    Ok(())
}

#[tokio::test]
async fn terminating_ingest_resets_creator_live_operational_metrics() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);
    let broadcast = insert_ready_broadcast(state.db.sqlite_adapter(), &creator).await?;

    let connected = connect_live_ingest(
        State(state.clone()),
        Json(IngestConnectRequest {
            stream_key: creator.stream_key.clone(),
            protocol: "rtmp".to_string(),
            ingest_server: "test-ingest-metrics".to_string(),
            broadcast_id: Some(broadcast.id.clone()),
        }),
    )
    .await?
    .0;

    let heartbeat = heartbeat_live_ingest(
        State(state.clone()),
        Path(connected.session.id.clone()),
        {
            let mut headers = HeaderMap::new();
            headers.insert(
                "x-ingest-token",
                HeaderValue::from_str(&connected.ingest_token)
                    .map_err(|error| AppError::Internal(error.to_string()))?,
            );
            headers
        },
        Json(IngestHeartbeatRequest {
            bitrate_kbps: 6400,
            viewers: 1444,
            dropped_frames: 2,
            cpu_percent: Some(31),
            free_disk_gb: Some(712.4),
            ingest_latency_ms: None,
            source_probe: None,
        }),
    )
    .await?
    .0;
    assert_eq!(heartbeat.bitrate_kbps, 6400);

    let terminated = terminate_creator_live_ingest(
        State(state.clone()),
        headers,
        Path(connected.session.id.clone()),
        Json(TerminateLiveIngestRequest {
            reason: Some("metrics cleanup regression".to_string()),
        }),
    )
    .await?
    .0;
    assert_eq!(terminated.status, "terminated");

    let control =
        fetch_creator_live_control_response(state.db.sqlite_adapter(), &creator.id).await?;
    assert_eq!(control.current_viewers, 0);
    assert_eq!(control.health.current_bitrate_kbps, 0);
    assert_eq!(control.health.current_cpu_percent, 0);
    assert_eq!(control.health.current_dropped_frames, 0);
    assert_eq!(control.health.current_free_disk_gb, 0.0);

    let runtime =
        fetch_creator_live_runtime_response(state.db.sqlite_adapter(), &creator.id).await?;
    assert!(runtime.active_session.is_none());
    assert_eq!(runtime.snapshot.profile.live_status, "offline");
    assert_eq!(runtime.health.current_bitrate_kbps, 0);
    assert_eq!(runtime.health.current_cpu_percent, 0);
    assert_eq!(runtime.health.current_dropped_frames, 0);
    assert_eq!(runtime.health.current_free_disk_gb, 0.0);

    Ok(())
}

#[tokio::test]
async fn playback_token_cannot_access_source_media_path() -> AppResult<()> {
    let (state, _creator) = setup_test_state().await?;
    let (_session_id, playback_token, asset) = insert_playback_session_for_upload(
        state.db.sqlite_adapter(),
        "flm-afterglow",
        None,
        None,
        "free",
    )
    .await?;

    let allowed = validate_playback_session_token_for_path(
        &state.db,
        &playback_token,
        asset
            .playback_path
            .as_deref()
            .expect("fixture asset has playback path"),
    )
    .await?;
    assert_eq!(allowed.content_id, "flm-afterglow");

    let poster_path = asset
        .poster_path
        .as_deref()
        .expect("fixture asset has poster path");
    let poster =
        validate_playback_session_token_for_path(&state.db, &playback_token, poster_path).await?;
    assert_eq!(poster.content_id, "flm-afterglow");
    let thumbnail_path = asset
        .variants
        .iter()
        .find(|variant| variant.variant_type == "thumbnail")
        .map(|variant| variant.relative_path.as_str())
        .expect("fixture asset should have a thumbnail derivative");
    let thumbnail =
        validate_playback_session_token_for_path(&state.db, &playback_token, thumbnail_path)
            .await?;
    assert_eq!(thumbnail.content_id, "flm-afterglow");

    let source_error =
        validate_playback_session_token_for_path(&state.db, &playback_token, &asset.source_path)
            .await
            .expect_err("source path must not be authorized by playback token");
    assert!(matches!(source_error, AppError::Forbidden));
    Ok(())
}

#[tokio::test]
async fn revoking_auth_session_expires_bound_playback_session() -> AppResult<()> {
    let (state, _creator) = setup_test_state().await?;
    let playback_token =
        insert_user_auth_session(state.db.sqlite_adapter(), "usr-viewer", &["user"]).await?;
    let identity = lookup_identity(&state.db, &playback_token).await?;
    let target = fetch_upload_playback_target(state.db.sqlite_adapter(), "flm-afterglow").await?;

    let grant = create_content_playback_session(
        State(state.clone()),
        auth_headers(&playback_token),
        Path(target.upload.id.clone()),
        None,
    )
    .await?
    .0;
    assert_eq!(grant.session.content_id, target.upload.id);

    revoke_session(
        State(state.clone()),
        auth_headers(&playback_token),
        Path(identity.session_id.clone()),
    )
    .await?;

    let session =
        fetch_playback_session_record_by_id(state.db.sqlite_adapter(), &grant.session.id).await?;
    assert!(session.expires_at <= Utc::now().to_rfc3339());

    let error = validate_playback_session(&state.db, &grant.session.id, &grant.playback_token)
        .await
        .expect_err("revoked auth session must invalidate bound playback token");
    assert!(matches!(error, AppError::Unauthorized));
    Ok(())
}

#[tokio::test]
async fn admin_can_inspect_and_reconcile_playback_session_by_id() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);
    let (session_id, playback_token, _asset) = insert_playback_session_for_upload(
        state.db.sqlite_adapter(),
        "flm-afterglow",
        None,
        None,
        "free",
    )
    .await?;

    let inspected = get_admin_playback_session(
        State(state.clone()),
        headers.clone(),
        Path(session_id.clone()),
    )
    .await?
    .0;
    assert_eq!(inspected.session.id, session_id);
    assert!(inspected.active);
    assert!(inspected.valid_access);

    sqlx::query("UPDATE uploads SET visibility = 'private' WHERE id = ?")
        .bind("flm-afterglow")
        .execute(state.db.sqlite_adapter())
        .await?;

    let report =
        reconcile_admin_playback_session(State(state.clone()), headers, Path(session_id.clone()))
            .await?
            .0;
    assert_eq!(report.session_id, session_id);
    assert!(report.actions.iter().any(|action| {
        action.action_type == "session_invalidated"
            && action.previous_state.as_deref() == Some("active")
            && action.next_state.as_deref() == Some("invalid")
    }));
    assert!(!report.record.active);
    assert!(!report.record.valid_access);

    let playback_error = get_playback_session(
        State(state),
        Path(session_id),
        Query(PlaybackAccessQuery {
            playback_token: Some(playback_token),
            hls_msn: None,
            hls_part: None,
        }),
    )
    .await
    .expect_err("invalidated playback session should not remain accessible");
    assert!(matches!(playback_error, AppError::Unauthorized));
    Ok(())
}

#[tokio::test]
async fn refreshing_playback_session_rotates_token_and_invalidates_old_token() -> AppResult<()> {
    let (state, _creator) = setup_test_state().await?;
    let (session_id, playback_token, asset) = insert_playback_session_for_upload(
        state.db.sqlite_adapter(),
        "flm-afterglow",
        None,
        None,
        "free",
    )
    .await?;
    let before =
        fetch_playback_session_record_by_id(state.db.sqlite_adapter(), &session_id).await?;

    let refreshed = refresh_playback_session(
        State(state.clone()),
        Path(session_id.clone()),
        Query(PlaybackAccessQuery {
            playback_token: Some(playback_token.clone()),
            hls_msn: None,
            hls_part: None,
        }),
    )
    .await?
    .0;

    assert_eq!(refreshed.session.id, session_id);
    assert_ne!(refreshed.playback_token, playback_token);
    assert!(refreshed.session.expires_at > before.expires_at);
    assert!(refreshed.manifest_url.contains(&refreshed.playback_token));

    let old_error = get_playback_session(
        State(state.clone()),
        Path(session_id.clone()),
        Query(PlaybackAccessQuery {
            playback_token: Some(playback_token.clone()),
            hls_msn: None,
            hls_part: None,
        }),
    )
    .await
    .expect_err("old playback token should be invalid after refresh");
    assert!(matches!(old_error, AppError::Unauthorized));

    let fetched = get_playback_session(
        State(state.clone()),
        Path(session_id.clone()),
        Query(PlaybackAccessQuery {
            playback_token: Some(refreshed.playback_token.clone()),
            hls_msn: None,
            hls_part: None,
        }),
    )
    .await?
    .0;
    assert_eq!(fetched.playback_token, refreshed.playback_token);
    assert_eq!(fetched.audio_tracks.len(), refreshed.audio_tracks.len());

    let allowed = validate_playback_session_token_for_path(
        &state.db,
        &refreshed.playback_token,
        asset
            .playback_path
            .as_deref()
            .expect("fixture asset has playback path"),
    )
    .await?;
    assert_eq!(allowed.content_id, "flm-afterglow");

    let old_path_error = validate_playback_session_token_for_path(
        &state.db,
        &playback_token,
        asset
            .playback_path
            .as_deref()
            .expect("fixture asset has playback path"),
    )
    .await
    .expect_err("old playback token should not authorize media paths");
    assert!(matches!(old_path_error, AppError::Unauthorized));
    Ok(())
}

#[tokio::test]
async fn refreshing_playback_session_fails_closed_if_token_was_rotated_concurrently()
-> AppResult<()> {
    let (state, _creator) = setup_test_state().await?;
    let (session_id, playback_token, _asset) = insert_playback_session_for_upload(
        state.db.sqlite_adapter(),
        "flm-afterglow",
        None,
        None,
        "free",
    )
    .await?;
    let validated =
        validate_playback_session_record(&state.db, &session_id, &playback_token).await?;

    sqlx::query("UPDATE playback_sessions SET token_hash = ? WHERE id = ?")
        .bind(hash_token("test-concurrent-rotation"))
        .bind(&session_id)
        .execute(state.db.sqlite_adapter())
        .await?;

    let result = super::super::playback::rotate_playback_session_token_for_refresh(
        &state.db,
        validated,
        &playback_token,
    )
    .await;
    match result {
        Err(AppError::Unauthorized) => {}
        Err(other) => panic!("unexpected refresh error: {other:?}"),
        Ok(_) => panic!("refresh should fail if the persisted token changed after validation"),
    }

    Ok(())
}

#[tokio::test]
async fn stale_media_worker_cannot_overwrite_newer_processing_attempt() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
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
    let old_lease = (Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
    let newer_lease = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE upload_jobs SET status = 'processing', processing_attempt_count = 1, updated_at = ? WHERE id = ? AND creator_id = ?",
    )
    .bind(&old_lease)
    .bind(&job_id)
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;
    sqlx::query(
        "UPDATE media_assets SET status = 'processing', updated_at = ? WHERE upload_job_id = ? AND creator_id = ?",
    )
    .bind(&old_lease)
    .bind(&job_id)
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;

    sqlx::query(
        "UPDATE upload_jobs SET status = 'uploaded', last_processing_error = NULL, last_failed_at = NULL, updated_at = ? WHERE id = ? AND creator_id = ?",
    )
    .bind(&newer_lease)
    .bind(&job_id)
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;
    sqlx::query(
        "UPDATE media_assets SET status = 'uploaded', updated_at = ? WHERE upload_job_id = ? AND creator_id = ?",
    )
    .bind(&newer_lease)
    .bind(&job_id)
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;

    let updated = fail_media_job_for_lease(
        state.db.sqlite_adapter(),
        &creator.id,
        &job_id,
        "stale worker failure",
        true,
        Some(&old_lease),
    )
    .await?;

    let refreshed_job =
        fetch_upload_job_by_id(state.db.sqlite_adapter(), &creator.id, &job_id).await?;
    let refreshed_asset =
        fetch_media_asset_by_upload_job(state.db.sqlite_adapter(), &creator.id, &job_id).await?;

    assert!(!updated);
    assert_eq!(refreshed_job.status, "uploaded");
    assert_eq!(refreshed_job.updated_at, newer_lease);
    assert_eq!(refreshed_asset.status, "uploaded");
    assert_eq!(refreshed_asset.updated_at, newer_lease);
    assert!(refreshed_job.last_processing_error.is_none());
    Ok(())
}

#[tokio::test]
async fn stale_processing_upload_job_materializes_as_uploaded_on_creator_read() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
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
    let stale_lease = (Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();

    sqlx::query(
        "UPDATE upload_jobs SET status = 'processing', processing_attempt_count = 1, last_processing_error = NULL, last_failed_at = NULL, updated_at = ? WHERE id = ? AND creator_id = ?",
    )
    .bind(&stale_lease)
    .bind(&job_id)
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;
    sqlx::query(
        "UPDATE media_assets SET status = 'processing', updated_at = ? WHERE upload_job_id = ? AND creator_id = ?",
    )
    .bind(&stale_lease)
    .bind(&job_id)
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;

    let jobs = fetch_upload_jobs(state.db.sqlite_adapter(), &creator.id).await?;
    let refreshed_job = jobs
        .into_iter()
        .find(|job| job.id == job_id)
        .expect("upload job must still be readable");
    let refreshed_asset =
        fetch_media_asset_by_upload_job(state.db.sqlite_adapter(), &creator.id, &job_id).await?;

    assert_eq!(refreshed_job.status, "uploaded");
    assert_eq!(refreshed_asset.status, "uploaded");
    assert!(refreshed_job.last_failed_at.is_some());
    assert!(
        refreshed_job
            .last_processing_error
            .as_deref()
            .is_some_and(|message| message.contains("watchdog timed out"))
    );
    Ok(())
}

#[tokio::test]
async fn stale_processing_upload_job_by_id_and_media_asset_reads_self_heal() -> AppResult<()> {
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
    let stale_lease = (Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();

    sqlx::query(
        "UPDATE upload_jobs SET status = 'processing', processing_attempt_count = 1, last_processing_error = NULL, last_failed_at = NULL, updated_at = ? WHERE id = ? AND creator_id = ?",
    )
    .bind(&stale_lease)
    .bind(&job_id)
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;
    sqlx::query(
        "UPDATE media_assets SET status = 'processing', updated_at = ? WHERE upload_job_id = ? AND creator_id = ?",
    )
    .bind(&stale_lease)
    .bind(&job_id)
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;

    let refreshed_job =
        fetch_upload_job_by_id(state.db.sqlite_adapter(), &creator.id, &job_id).await?;
    let refreshed_asset =
        fetch_media_asset_by_upload_job(state.db.sqlite_adapter(), &creator.id, &job_id).await?;
    let asset_via_route =
        get_media_asset_for_upload_job(State(state.clone()), headers, Path(job_id.clone()))
            .await?
            .0;

    assert_eq!(refreshed_job.status, "uploaded");
    assert_eq!(refreshed_asset.status, "uploaded");
    assert_eq!(asset_via_route.status, "uploaded");
    assert!(refreshed_job.last_failed_at.is_some());
    assert!(
        refreshed_job
            .last_processing_error
            .as_deref()
            .is_some_and(|message| message.contains("watchdog timed out"))
    );
    Ok(())
}

#[tokio::test]
async fn stale_processing_admin_media_record_materializes_as_failed_at_attempt_ceiling()
-> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
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
    let stale_lease = (Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();

    sqlx::query(
        "UPDATE upload_jobs SET status = 'processing', processing_attempt_count = ?, last_processing_error = NULL, last_failed_at = NULL, updated_at = ? WHERE id = ? AND creator_id = ?",
    )
    .bind(MAX_MEDIA_PROCESSING_ATTEMPTS)
    .bind(&stale_lease)
    .bind(&job_id)
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;
    sqlx::query(
        "UPDATE media_assets SET status = 'processing', updated_at = ? WHERE upload_job_id = ? AND creator_id = ?",
    )
    .bind(&stale_lease)
    .bind(&job_id)
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;

    let record =
        fetch_admin_media_job_record(state.db.sqlite_adapter(), &creator.id, &job_id).await?;

    assert_eq!(record.upload_job.status, "failed");
    assert_eq!(record.asset_status.as_deref(), Some("failed"));
    assert!(!record.stale_processing);
    assert!(record.repair_required);
    assert!(
        record
            .upload_job
            .last_processing_error
            .as_deref()
            .is_some_and(|message| message.contains("watchdog timed out"))
    );
    Ok(())
}

#[tokio::test]
async fn admin_can_inspect_and_reconcile_media_job_by_id() -> AppResult<()> {
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

    let inspected =
        get_admin_media_job(State(state.clone()), headers.clone(), Path(job_id.clone()))
            .await?
            .0;
    assert_eq!(inspected.upload_job.id, job_id);

    let stale_lease = (Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
    sqlx::query(
        "UPDATE upload_jobs SET status = 'processing', processing_attempt_count = 1, last_processing_error = NULL, last_failed_at = NULL, updated_at = ? WHERE id = ? AND creator_id = ?",
    )
    .bind(&stale_lease)
    .bind(&job_id)
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;
    sqlx::query(
        "UPDATE media_assets SET status = 'processing', updated_at = ? WHERE upload_job_id = ? AND creator_id = ?",
    )
    .bind(&stale_lease)
    .bind(&job_id)
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;

    let report = reconcile_admin_media_job(State(state.clone()), headers, Path(job_id.clone()))
        .await?
        .0;

    assert_eq!(report.job_id, job_id);
    assert!(report.actions.iter().any(|action| {
        action.action_type == "job_reconciled"
            && action.previous_status.as_deref() == Some("processing")
            && action.next_status.as_deref() == Some("uploaded")
    }));
    assert_eq!(report.record.upload_job.status, "uploaded");
    assert_eq!(report.record.asset_status.as_deref(), Some("uploaded"));
    assert!(!report.record.stale_processing);
    assert!(!report.record.repair_required);
    Ok(())
}

#[tokio::test]
async fn creator_retry_preserves_processing_attempt_history() -> AppResult<()> {
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
        "UPDATE upload_jobs SET status = 'failed', processing_attempt_count = 2, last_processing_error = ?, last_failed_at = ?, updated_at = ? WHERE id = ? AND creator_id = ?",
    )
    .bind("transcode failed")
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

    let retried = retry_upload_job_processing(State(state.clone()), headers, Path(job_id.clone()))
        .await?
        .0;

    let asset =
        fetch_media_asset_by_upload_job(state.db.sqlite_adapter(), &creator.id, &job_id).await?;

    assert_eq!(retried.status, "uploaded");
    assert_eq!(retried.processing_attempt_count, 2);
    assert!(retried.last_processing_error.is_none());
    assert!(retried.last_failed_at.is_none());
    assert_eq!(asset.status, "uploaded");
    Ok(())
}
