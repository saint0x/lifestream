use super::*;

async fn create_ready_film_upload(
    state: &SharedState,
    creator: &CreatorProfile,
    headers: &HeaderMap,
    title: &str,
    storage_label: &str,
) -> AppResult<UploadJob> {
    let temp_root =
        std::env::temp_dir().join(format!("vanta-upload-{storage_label}-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_root).await?;
    let media_path = temp_root.join("source.mp4");

    let ffmpeg = Command::new("ffmpeg")
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
        ffmpeg.status.success(),
        "ffmpeg fixture generation failed: {}",
        String::from_utf8_lossy(&ffmpeg.stderr)
    );

    let payload = tokio::fs::read(&media_path).await?;
    let created = create_upload_job(
        State(state.clone()),
        headers.clone(),
        Json(CreateUploadJobRequest {
            upload_id: None,
            series_id: None,
            kind: "film".to_string(),
            source_type: "resumable-upload".to_string(),
            title: title.to_string(),
            intended_visibility: "public".to_string(),
            bytes_expected: payload.len() as i64,
            storage_key: format!(
                "uploads/creator/{}/features/{}-{}.mp4",
                creator.handle,
                storage_label,
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
    let _ = append_upload_chunk(
        State(state.clone()),
        ingest_headers.clone(),
        Path(created.id.clone()),
        Query(AppendUploadChunkQuery { offset: 0 }),
        Bytes::from(payload),
    )
    .await?;
    let _ = complete_upload_ingest(
        State(state.clone()),
        ingest_headers,
        Path(created.id.clone()),
    )
    .await?;

    for _ in 0..30 {
        let current =
            fetch_media_asset_by_upload_job(state.db.sqlite_adapter(), &creator.id, &created.id)
                .await?;
        if current.status == "ready" {
            let _ = tokio::fs::remove_dir_all(&temp_root).await;
            return Ok(created);
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    let _ = tokio::fs::remove_dir_all(&temp_root).await;
    Err(AppError::Internal(
        "processed upload did not become ready in time".to_string(),
    ))
}

#[tokio::test]
async fn processed_upload_materializes_thumbnail_variant_and_publish_uses_it() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);
    let temp_root = std::env::temp_dir().join(format!("vanta-upload-thumb-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_root).await?;
    let media_path = temp_root.join("source.mp4");

    let ffmpeg = Command::new("ffmpeg")
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
        ffmpeg.status.success(),
        "ffmpeg fixture generation failed: {}",
        String::from_utf8_lossy(&ffmpeg.stderr)
    );

    let payload = tokio::fs::read(&media_path).await?;
    let created = create_upload_job(
        State(state.clone()),
        headers.clone(),
        Json(CreateUploadJobRequest {
            upload_id: None,
            series_id: None,
            kind: "film".to_string(),
            source_type: "resumable-upload".to_string(),
            title: "Thumbnail Derivative Validation".to_string(),
            intended_visibility: "public".to_string(),
            bytes_expected: payload.len() as i64,
            storage_key: format!(
                "uploads/creator/{}/features/thumb-derivative-{}.mp4",
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
    let _ = append_upload_chunk(
        State(state.clone()),
        ingest_headers.clone(),
        Path(created.id.clone()),
        Query(AppendUploadChunkQuery { offset: 0 }),
        Bytes::from(payload),
    )
    .await?;
    let _ = complete_upload_ingest(
        State(state.clone()),
        ingest_headers,
        Path(created.id.clone()),
    )
    .await?;

    let mut asset = None;
    for _ in 0..30 {
        let current =
            fetch_media_asset_by_upload_job(state.db.sqlite_adapter(), &creator.id, &created.id)
                .await?;
        if current.status == "ready" {
            asset = Some(current);
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let asset = asset.expect("processed asset should become ready");
    assert!(
        asset
            .playback_path
            .as_deref()
            .is_some_and(|path| path.contains(&format!("/{}/gen-0001/", asset.id))),
        "playback path should be generation-scoped by asset id: {:?}",
        asset.playback_path
    );
    assert!(
        asset
            .poster_path
            .as_deref()
            .is_some_and(|path| path.contains(&format!("/{}/gen-0001/", asset.id))),
        "poster path should be generation-scoped by asset id: {:?}",
        asset.poster_path
    );
    let thumbnail_variant = asset
        .variants
        .iter()
        .find(|variant| variant.variant_type == "thumbnail")
        .expect("processed asset should include thumbnail derivative");
    assert_eq!(asset.variants.len(), 4);
    assert_eq!(thumbnail_variant.label, "card_thumbnail");
    assert!(
        asset
            .variants
            .iter()
            .any(|variant| variant.variant_type == "playback")
    );
    assert!(
        asset
            .variants
            .iter()
            .any(|variant| variant.variant_type == "audio")
    );
    assert!(thumbnail_variant.file_size_bytes > 0);
    assert_eq!(asset.preview_tracks.len(), 1);
    assert_eq!(asset.preview_tracks[0].label, "timeline_preview");
    assert!(asset.preview_tracks[0].frame_count >= 1);
    assert!(asset.preview_tracks[0].interval_sec >= 1.0);
    assert!(!asset.preview_tracks[0].published);
    assert_eq!(
        asset.default_preview_track_id.as_deref(),
        Some(asset.preview_tracks[0].id.as_str())
    );
    let preview_image_body = tokio::fs::read(media_path_for_relative(
        &state,
        &asset.preview_tracks[0].image_path,
    ))
    .await?;
    assert!(!preview_image_body.is_empty());
    let preview_vtt_body = tokio::fs::read_to_string(media_path_for_relative(
        &state,
        &asset.preview_tracks[0].vtt_path,
    ))
    .await?;
    assert!(preview_vtt_body.starts_with("WEBVTT"));
    assert!(preview_vtt_body.contains("#xywh="));

    let published = publish_upload_job(
        State(state.clone()),
        headers,
        Path(created.id.clone()),
        Json(PublishUploadJobRequest {
            description: Some("thumbnail publish".to_string()),
            visibility: Some("public".to_string()),
            slug: Some(format!("thumb-derivative-{}", Uuid::new_v4().simple())),
            release_at: None,
            access_policy: Some("free".to_string()),
            access_tier_id: None,
            price_cents: None,
            currency: None,
            rental_window_hours: None,
            season_number: None,
            episode_number: None,
            season_title: None,
            season_synopsis: None,
        }),
    )
    .await?
    .0;
    assert_eq!(published.thumbnail, thumbnail_variant.url);
    let grant = create_content_playback_session(
        State(state.clone()),
        HeaderMap::new(),
        Path(published.id.clone()),
        None,
    )
    .await?
    .0;
    assert_eq!(
        grant.thumbnail_url.as_deref(),
        Some(published.thumbnail.as_str())
    );
    assert_eq!(grant.preview_tracks.len(), 1);
    assert_eq!(grant.preview_tracks[0].label, "timeline_preview");
    assert!(grant.preview_tracks[0].published);
    assert_eq!(
        grant.default_preview_track_id.as_deref(),
        Some(grant.preview_tracks[0].id.as_str())
    );
    assert_eq!(
        grant.preview_tracks[0].image_url,
        format!(
            "/api/v1/media/{}?playbackToken={}",
            asset.preview_tracks[0].image_path, grant.playback_token
        )
    );
    assert_eq!(
        grant.preview_tracks[0].vtt_url,
        format!(
            "/api/v1/media/{}?playbackToken={}",
            asset.preview_tracks[0].vtt_path, grant.playback_token
        )
    );
    let preview_image_session = validate_playback_session_token_for_path(
        &state.db,
        &grant.playback_token,
        &asset.preview_tracks[0].image_path,
    )
    .await?;
    assert_eq!(preview_image_session.content_id, published.id);
    let preview_vtt_session = validate_playback_session_token_for_path(
        &state.db,
        &grant.playback_token,
        &asset.preview_tracks[0].vtt_path,
    )
    .await?;
    assert_eq!(preview_vtt_session.content_id, published.id);
    let preview_vtt_response = serve_media_file(
        State(state.clone()),
        Path(asset.preview_tracks[0].vtt_path.clone()),
        HeaderMap::new(),
        Query(PlaybackAccessQuery {
            playback_token: Some(grant.playback_token.clone()),
            hls_msn: None,
            hls_part: None,
        }),
    )
    .await?;
    let preview_vtt_body = axum::body::to_bytes(preview_vtt_response.into_body(), usize::MAX)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let preview_vtt_text = String::from_utf8(preview_vtt_body.to_vec())
        .map_err(|error| AppError::Internal(error.to_string()))?;
    assert!(preview_vtt_text.contains(&format!(
        "/api/v1/media/{}?playbackToken={}",
        asset.preview_tracks[0].image_path, grant.playback_token
    )));

    let _ = tokio::fs::remove_dir_all(temp_root).await;
    Ok(())
}

#[tokio::test]
async fn repeated_title_publish_without_explicit_slug_allocates_suffix() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);
    let title = "Repeated Title Validation";

    let first =
        create_ready_film_upload(&state, &creator, &headers, title, "repeat-title-a").await?;
    let first_publish = publish_upload_job(
        State(state.clone()),
        headers.clone(),
        Path(first.id.clone()),
        Json(PublishUploadJobRequest {
            description: Some("first repeated-title publish".to_string()),
            visibility: None,
            slug: None,
            release_at: None,
            access_policy: Some("free".to_string()),
            access_tier_id: None,
            price_cents: None,
            currency: None,
            rental_window_hours: None,
            season_number: None,
            episode_number: None,
            season_title: None,
            season_synopsis: None,
        }),
    )
    .await?
    .0;

    let second =
        create_ready_film_upload(&state, &creator, &headers, title, "repeat-title-b").await?;
    let second_publish = publish_upload_job(
        State(state.clone()),
        headers,
        Path(second.id.clone()),
        Json(PublishUploadJobRequest {
            description: Some("second repeated-title publish".to_string()),
            visibility: None,
            slug: None,
            release_at: None,
            access_policy: Some("free".to_string()),
            access_tier_id: None,
            price_cents: None,
            currency: None,
            rental_window_hours: None,
            season_number: None,
            episode_number: None,
            season_title: None,
            season_synopsis: None,
        }),
    )
    .await?
    .0;

    assert_eq!(
        first_publish.slug.as_deref(),
        Some("repeated-title-validation")
    );
    assert_eq!(
        second_publish.slug.as_deref(),
        Some("repeated-title-validation-2")
    );
    assert_ne!(first_publish.id, second_publish.id);
    Ok(())
}

#[tokio::test]
async fn processed_upload_materializes_webvtt_caption_variant_from_embedded_subtitles()
-> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);
    let temp_root = std::env::temp_dir().join(format!("vanta-upload-caption-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_root).await?;
    let subtitle_path = temp_root.join("captions.srt");
    let media_path = temp_root.join("source-with-subs.mp4");
    tokio::fs::write(
        &subtitle_path,
        "1\n00:00:00,000 --> 00:00:00,800\nSignal detected.\n",
    )
    .await?;

    let ffmpeg = Command::new("ffmpeg")
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("testsrc=size=320x240:rate=24:duration=1")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("sine=frequency=1000:sample_rate=48000:duration=1")
        .arg("-i")
        .arg(&subtitle_path)
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("1:a:0")
        .arg("-map")
        .arg("2:0")
        .arg("-c:v")
        .arg("libx264")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-c:a")
        .arg("aac")
        .arg("-c:s")
        .arg("mov_text")
        .arg("-metadata:s:s:0")
        .arg("language=eng")
        .arg("-shortest")
        .arg(&media_path)
        .output()
        .await?;
    assert!(
        ffmpeg.status.success(),
        "ffmpeg subtitle fixture generation failed: {}",
        String::from_utf8_lossy(&ffmpeg.stderr)
    );

    let payload = tokio::fs::read(&media_path).await?;
    let created = create_upload_job(
        State(state.clone()),
        headers.clone(),
        Json(CreateUploadJobRequest {
            upload_id: None,
            series_id: None,
            kind: "film".to_string(),
            source_type: "resumable-upload".to_string(),
            title: "Caption Variant Validation".to_string(),
            intended_visibility: "private".to_string(),
            bytes_expected: payload.len() as i64,
            storage_key: format!(
                "uploads/creator/{}/features/caption-variant-{}.mp4",
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
    let _ = append_upload_chunk(
        State(state.clone()),
        ingest_headers.clone(),
        Path(created.id.clone()),
        Query(AppendUploadChunkQuery { offset: 0 }),
        Bytes::from(payload),
    )
    .await?;
    let _ = complete_upload_ingest(
        State(state.clone()),
        ingest_headers,
        Path(created.id.clone()),
    )
    .await?;

    let mut asset = None;
    for _ in 0..30 {
        let current =
            fetch_media_asset_by_upload_job(state.db.sqlite_adapter(), &creator.id, &created.id)
                .await?;
        if current.status == "ready" {
            asset = Some(current);
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let asset = asset.expect("caption-enabled asset should become ready");
    let caption_variant = asset
        .variants
        .iter()
        .find(|variant| variant.variant_type == "caption")
        .expect("processed asset should include normalized caption variant");
    let audio_variant = asset
        .variants
        .iter()
        .find(|variant| variant.variant_type == "audio")
        .expect("processed asset should include packaged audio variant");
    assert_eq!(asset.audio_tracks.len(), 1);
    assert_eq!(asset.audio_tracks[0].id, audio_variant.id);
    assert_eq!(asset.audio_tracks[0].label, "audio-und");
    assert_eq!(asset.audio_tracks[0].language, "und");
    assert_eq!(asset.audio_tracks[0].codec.as_deref(), Some("aac"));
    assert_eq!(audio_variant.bitrate_bps, Some(128_000));
    assert!(audio_variant.label.ends_with(":aac"));
    assert_eq!(
        asset.audio_tracks[0].playlist_path.as_deref(),
        Some(audio_variant.relative_path.as_str())
    );
    assert_eq!(
        asset.audio_tracks[0].playlist_url.as_deref(),
        Some(audio_variant.url.as_str())
    );
    assert_eq!(
        asset.default_audio_track_id.as_deref(),
        Some(asset.audio_tracks[0].id.as_str())
    );
    assert_eq!(asset.caption_tracks.len(), 1);
    assert_eq!(asset.caption_tracks[0].id, caption_variant.id);
    assert_eq!(asset.caption_tracks[0].language, "eng");
    assert_eq!(asset.caption_tracks[0].role, "standard");
    assert_eq!(asset.caption_tracks[0].source, "source-provided");
    assert!(!asset.caption_tracks[0].published);
    assert_eq!(
        asset.default_caption_track_id.as_deref(),
        Some(asset.caption_tracks[0].id.as_str())
    );
    assert!(caption_variant.label.starts_with("captions-eng"));
    assert_eq!(caption_variant.mime_type, "text/vtt");
    assert!(caption_variant.file_size_bytes > 0);
    let caption_path = media_path_for_relative(&state, &caption_variant.relative_path);
    let caption_body = tokio::fs::read_to_string(&caption_path).await?;
    assert!(caption_body.contains("WEBVTT"));
    assert!(caption_body.contains("Signal detected."));
    let published = publish_upload_job(
        State(state.clone()),
        headers.clone(),
        Path(created.id.clone()),
        Json(PublishUploadJobRequest {
            description: Some("caption publish".to_string()),
            visibility: Some("public".to_string()),
            slug: Some(format!("caption-variant-{}", Uuid::new_v4().simple())),
            release_at: None,
            access_policy: Some("free".to_string()),
            access_tier_id: None,
            price_cents: None,
            currency: None,
            rental_window_hours: None,
            season_number: None,
            episode_number: None,
            season_title: None,
            season_synopsis: None,
        }),
    )
    .await?
    .0;
    let grant = create_content_playback_session(
        State(state.clone()),
        HeaderMap::new(),
        Path(published.id.clone()),
        None,
    )
    .await?
    .0;
    assert_eq!(grant.audio_tracks.len(), 1);
    assert_eq!(grant.audio_tracks[0].id, audio_variant.id);
    assert!(grant.audio_tracks[0].published);
    assert_eq!(grant.audio_tracks[0].label, "audio-und");
    assert_eq!(grant.audio_tracks[0].language, "und");
    assert_eq!(grant.audio_tracks[0].codec.as_deref(), Some("aac"));
    assert_eq!(
        grant.audio_tracks[0].playlist_path.as_deref(),
        Some(audio_variant.relative_path.as_str())
    );
    assert_eq!(
        grant.audio_tracks[0].playlist_url.as_deref(),
        Some(
            format!(
                "/api/v1/media/{}?playbackToken={}",
                audio_variant.relative_path, grant.playback_token
            )
            .as_str()
        )
    );
    assert_eq!(
        grant.default_audio_track_id.as_deref(),
        Some(grant.audio_tracks[0].id.as_str())
    );
    assert_eq!(grant.caption_tracks.len(), 1);
    assert_eq!(grant.caption_tracks[0].id, caption_variant.id);
    assert_eq!(grant.caption_tracks[0].language, "eng");
    assert_eq!(grant.caption_tracks[0].role, "standard");
    assert_eq!(grant.caption_tracks[0].source, "source-provided");
    assert!(grant.caption_tracks[0].published);
    assert_eq!(
        grant.default_caption_track_id.as_deref(),
        Some(grant.caption_tracks[0].id.as_str())
    );
    assert_eq!(
        grant.caption_tracks[0].url,
        format!(
            "/api/v1/media/{}?playbackToken={}",
            caption_variant.relative_path, grant.playback_token
        )
    );
    let manifest_response = get_playback_manifest(
        State(state.clone()),
        Path(grant.session.id.clone()),
        Query(PlaybackAccessQuery {
            playback_token: Some(grant.playback_token.clone()),
            hls_msn: None,
            hls_part: None,
        }),
    )
    .await?;
    let manifest_body = axum::body::to_bytes(manifest_response.into_body(), usize::MAX)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let manifest_text = String::from_utf8(manifest_body.to_vec())
        .map_err(|error| AppError::Internal(error.to_string()))?;
    assert!(manifest_text.contains("#EXT-X-MEDIA:TYPE=SUBTITLES"));
    assert!(manifest_text.contains(&format!(
        "/api/v1/media/{}?playbackToken={}",
        caption_variant.relative_path, grant.playback_token
    )));
    let caption_session = validate_playback_session_token_for_path(
        &state.db,
        &grant.playback_token,
        &caption_variant.relative_path,
    )
    .await?;
    assert_eq!(caption_session.content_id, published.id);

    let _ = tokio::fs::remove_dir_all(temp_root).await;
    Ok(())
}

#[tokio::test]
async fn processed_upload_materializes_multi_audio_variants_and_playback_honors_preferences()
-> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);
    let temp_root =
        std::env::temp_dir().join(format!("vanta-upload-multi-audio-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_root).await?;
    let media_path = temp_root.join("source-multi-audio.mp4");

    let ffmpeg = Command::new("ffmpeg")
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("testsrc=size=640x360:rate=24:duration=2")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("sine=frequency=880:sample_rate=48000:duration=2")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("sine=frequency=440:sample_rate=48000:duration=2")
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("1:a:0")
        .arg("-map")
        .arg("2:a:0")
        .arg("-c:v")
        .arg("libx264")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-c:a")
        .arg("aac")
        .arg("-metadata:s:a:0")
        .arg("language=eng")
        .arg("-metadata:s:a:1")
        .arg("language=jpn")
        .arg("-shortest")
        .arg(&media_path)
        .output()
        .await?;
    assert!(
        ffmpeg.status.success(),
        "ffmpeg multi-audio fixture generation failed: {}",
        String::from_utf8_lossy(&ffmpeg.stderr)
    );

    let payload = tokio::fs::read(&media_path).await?;
    let created = create_upload_job(
        State(state.clone()),
        headers.clone(),
        Json(CreateUploadJobRequest {
            upload_id: None,
            series_id: None,
            kind: "film".to_string(),
            source_type: "resumable-upload".to_string(),
            title: "Multi Audio Validation".to_string(),
            intended_visibility: "private".to_string(),
            bytes_expected: payload.len() as i64,
            storage_key: format!(
                "uploads/creator/{}/features/multi-audio-{}.mp4",
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
    let _ = append_upload_chunk(
        State(state.clone()),
        ingest_headers.clone(),
        Path(created.id.clone()),
        Query(AppendUploadChunkQuery { offset: 0 }),
        Bytes::from(payload),
    )
    .await?;
    let _ = complete_upload_ingest(
        State(state.clone()),
        ingest_headers,
        Path(created.id.clone()),
    )
    .await?;

    let mut asset = None;
    for _ in 0..30 {
        let current =
            fetch_media_asset_by_upload_job(state.db.sqlite_adapter(), &creator.id, &created.id)
                .await?;
        if current.status == "ready" {
            asset = Some(current);
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let asset = asset.expect("multi-audio asset should become ready");
    let audio_variants = asset
        .variants
        .iter()
        .filter(|variant| variant.variant_type == "audio")
        .collect::<Vec<_>>();
    assert_eq!(audio_variants.len(), 2);
    assert_eq!(asset.audio_tracks.len(), 2);
    let default_asset_track = asset
        .audio_tracks
        .iter()
        .find(|track| track.is_default)
        .expect("asset should mark one default audio track");
    assert_eq!(
        asset
            .audio_tracks
            .iter()
            .filter(|track| track.is_default)
            .count(),
        1
    );
    assert_eq!(
        asset.default_audio_track_id.as_deref(),
        Some(default_asset_track.id.as_str())
    );
    assert_eq!(
        asset
            .audio_tracks
            .iter()
            .map(|track| track.language.as_str())
            .collect::<Vec<_>>(),
        vec!["eng", "jpn"]
    );
    assert_eq!(default_asset_track.language, "eng");
    assert!(asset.audio_tracks[0].playlist_path.is_some());
    assert!(asset.audio_tracks[1].playlist_path.is_some());
    assert!(asset.audio_tracks[0].playlist_url.is_some());
    assert!(asset.audio_tracks[1].playlist_url.is_some());

    let published = publish_upload_job(
        State(state.clone()),
        headers.clone(),
        Path(created.id.clone()),
        Json(PublishUploadJobRequest {
            description: Some("multi-audio publish".to_string()),
            visibility: Some("public".to_string()),
            slug: Some(format!("multi-audio-{}", Uuid::new_v4().simple())),
            release_at: None,
            access_policy: Some("free".to_string()),
            access_tier_id: None,
            price_cents: None,
            currency: None,
            rental_window_hours: None,
            season_number: None,
            episode_number: None,
            season_title: None,
            season_synopsis: None,
        }),
    )
    .await?
    .0;

    let anonymous_grant = create_content_playback_session(
        State(state.clone()),
        HeaderMap::new(),
        Path(published.id.clone()),
        None,
    )
    .await?
    .0;
    assert_eq!(anonymous_grant.audio_tracks.len(), 2);
    let default_anonymous_track = anonymous_grant
        .audio_tracks
        .iter()
        .find(|track| track.is_default)
        .expect("anonymous playback should expose one default audio track");
    assert_eq!(
        anonymous_grant.default_audio_track_id.as_deref(),
        Some(default_anonymous_track.id.as_str())
    );
    assert_eq!(default_anonymous_track.language, "eng");
    assert!(default_anonymous_track.published);
    assert!(
        default_anonymous_track
            .playlist_url
            .as_deref()
            .is_some_and(|url| url.contains(&anonymous_grant.playback_token))
    );

    sqlx::query(
        r#"
        INSERT INTO user_playback_settings (
            user_id, default_quality, audio_language, subtitle_language, subtitle_style,
            autoplay_next_episode, autoplay_trailers, reduced_motion, prefer_dubbed, playback_speed
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(user_id) DO UPDATE SET
            audio_language = excluded.audio_language,
            prefer_dubbed = excluded.prefer_dubbed
        "#,
    )
    .bind("usr-viewer")
    .bind("auto")
    .bind("jpn")
    .bind("off")
    .bind("system")
    .bind(1_i64)
    .bind(1_i64)
    .bind(0_i64)
    .bind(1_i64)
    .bind("1.0")
    .execute(state.db.sqlite_adapter())
    .await?;
    let viewer_token =
        insert_user_auth_session(state.db.sqlite_adapter(), "usr-viewer", &["user"]).await?;
    let preferred_grant = create_content_playback_session(
        State(state.clone()),
        auth_headers(&viewer_token),
        Path(published.id.clone()),
        None,
    )
    .await?
    .0;
    assert_eq!(preferred_grant.audio_tracks.len(), 2);
    let default_preferred_track = preferred_grant
        .audio_tracks
        .iter()
        .find(|track| track.is_default)
        .expect("preferred playback should expose one default audio track");
    assert_eq!(
        preferred_grant.default_audio_track_id.as_deref(),
        Some(default_preferred_track.id.as_str())
    );
    assert_eq!(default_preferred_track.language, "jpn");
    assert!(default_preferred_track.is_dubbed);
    assert!(
        default_preferred_track
            .playlist_url
            .as_deref()
            .is_some_and(|url| url.contains(&preferred_grant.playback_token))
    );

    let manifest_response = get_playback_manifest(
        State(state.clone()),
        Path(preferred_grant.session.id.clone()),
        Query(PlaybackAccessQuery {
            playback_token: Some(preferred_grant.playback_token.clone()),
            hls_msn: None,
            hls_part: None,
        }),
    )
    .await?;
    let manifest_body = axum::body::to_bytes(manifest_response.into_body(), usize::MAX)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let manifest_text = String::from_utf8(manifest_body.to_vec())
        .map_err(|error| AppError::Internal(error.to_string()))?;
    assert!(manifest_text.contains("#EXT-X-MEDIA:TYPE=AUDIO"));
    assert!(manifest_text.contains("LANGUAGE=\"eng\""));
    assert!(manifest_text.contains("LANGUAGE=\"jpn\""));
    let preferred_playlist_path = default_preferred_track
        .playlist_path
        .as_deref()
        .expect("preferred track should expose playlist path");
    assert!(manifest_text.contains(&format!(
        "/api/v1/media/{}?playbackToken={}",
        preferred_playlist_path, preferred_grant.playback_token
    )));
    let audio_session = validate_playback_session_token_for_path(
        &state.db,
        &preferred_grant.playback_token,
        preferred_playlist_path,
    )
    .await?;
    assert_eq!(audio_session.content_id, published.id);

    let _ = tokio::fs::remove_dir_all(temp_root).await;
    Ok(())
}

#[test]
fn caption_track_selection_prefers_matching_language_when_available() {
    let variants = vec![
        MediaAssetVariant {
            id: "var-eng".to_string(),
            variant_type: "caption".to_string(),
            label: "captions-eng:eng".to_string(),
            relative_path: "processed/crt/asset/captions/captions-eng.vtt".to_string(),
            url: "/api/v1/media/processed/crt/asset/captions/captions-eng.vtt".to_string(),
            mime_type: "text/vtt".to_string(),
            width: None,
            height: None,
            bitrate_bps: None,
            file_size_bytes: 128,
            is_default: true,
            created_at: Utc::now().to_rfc3339(),
        },
        MediaAssetVariant {
            id: "var-deu".to_string(),
            variant_type: "caption".to_string(),
            label: "captions-deu:deu".to_string(),
            relative_path: "processed/crt/asset/captions/captions-deu.vtt".to_string(),
            url: "/api/v1/media/processed/crt/asset/captions/captions-deu.vtt".to_string(),
            mime_type: "text/vtt".to_string(),
            width: None,
            height: None,
            bitrate_bps: None,
            file_size_bytes: 128,
            is_default: false,
            created_at: Utc::now().to_rfc3339(),
        },
    ];

    let playback_media_url =
        |relative_path: &str| format!("/api/v1/media/{relative_path}?playbackToken=playback-token");
    let tracks = build_media_caption_tracks(
        "published",
        &variants,
        Some(&playback_media_url),
        Some("deu"),
    );

    assert_eq!(tracks.len(), 2);
    assert!(
        tracks
            .iter()
            .any(|track| track.id == "var-deu" && track.is_default)
    );
    assert!(
        tracks
            .iter()
            .any(|track| track.id == "var-eng" && !track.is_default)
    );
    assert!(
        tracks
            .iter()
            .find(|track| track.id == "var-deu")
            .is_some_and(|track| track.url.contains("playback-token"))
    );
}
