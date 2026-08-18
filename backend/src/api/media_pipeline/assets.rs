use super::*;

pub(crate) async fn ensure_media_asset_shell(
    pool: &SqlitePool,
    creator_id: &str,
    job: &UploadJob,
    source_relative_path: &str,
) -> AppResult<MediaAsset> {
    let now = Utc::now().to_rfc3339();
    let asset_id = format!("ast-{}", Uuid::new_v4().simple());
    sqlx::query(
        r#"
        INSERT INTO media_assets (
            id, creator_id, upload_job_id, upload_id, series_id, kind, title, status, visibility,
            source_relative_path, poster_relative_path, playback_relative_path, mime_type,
            checksum_sha256, container_format, file_size_bytes, duration_sec, width, height,
            frame_rate, video_codec, audio_codec, has_video, has_audio, created_at, updated_at,
            processed_at, published_content_id
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(upload_job_id) DO UPDATE SET
            upload_id = excluded.upload_id,
            series_id = excluded.series_id,
            kind = excluded.kind,
            title = excluded.title,
            visibility = excluded.visibility,
            source_relative_path = excluded.source_relative_path,
            mime_type = excluded.mime_type,
            checksum_sha256 = excluded.checksum_sha256,
            file_size_bytes = excluded.file_size_bytes,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(&asset_id)
    .bind(creator_id)
    .bind(&job.id)
    .bind(job.upload_id.clone())
    .bind(job.series_id.clone())
    .bind(&job.kind)
    .bind(&job.title)
    .bind(&job.status)
    .bind(&job.intended_visibility)
    .bind(source_relative_path)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(&job.mime_type)
    .bind(job.checksum_sha256.clone())
    .bind(Option::<String>::None)
    .bind(job.bytes_expected)
    .bind(0.0_f64)
    .bind(Option::<i64>::None)
    .bind(Option::<i64>::None)
    .bind(Option::<f64>::None)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(0_i64)
    .bind(0_i64)
    .bind(&now)
    .bind(&now)
    .bind(Option::<String>::None)
    .bind(job.published_content_id.clone())
    .execute(pool)
    .await?;

    fetch_media_asset_by_upload_job(pool, creator_id, &job.id).await
}

pub(crate) async fn fetch_media_assets(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<MediaAsset>> {
    reconcile_stale_media_processing_jobs_for_read(pool, Some(creator_id), None).await?;
    let rows = sqlx::query(
        r#"
        SELECT id, upload_job_id, upload_id, series_id, kind, title, status, visibility,
               source_relative_path, poster_relative_path, playback_relative_path, mime_type,
               checksum_sha256, container_format, file_size_bytes, duration_sec, width, height,
               frame_rate, video_codec, audio_codec, has_video, has_audio, created_at, updated_at,
               processed_at, published_content_id
        FROM media_assets
        WHERE creator_id = ?
        ORDER BY created_at DESC
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    let mut assets = Vec::with_capacity(rows.len());
    for row in rows {
        assets.push(media_asset_from_row(pool, creator_id, row).await?);
    }
    Ok(assets)
}

pub(crate) async fn fetch_media_asset_by_upload_job(
    pool: &SqlitePool,
    creator_id: &str,
    job_id: &str,
) -> AppResult<MediaAsset> {
    reconcile_stale_media_processing_jobs_for_read(pool, Some(creator_id), Some(job_id)).await?;
    let row = sqlx::query(
        r#"
        SELECT id, upload_job_id, upload_id, series_id, kind, title, status, visibility,
               source_relative_path, poster_relative_path, playback_relative_path, mime_type,
               checksum_sha256, container_format, file_size_bytes, duration_sec, width, height,
               frame_rate, video_codec, audio_codec, has_video, has_audio, created_at, updated_at,
               processed_at, published_content_id
        FROM media_assets
        WHERE creator_id = ? AND upload_job_id = ?
        "#,
    )
    .bind(creator_id)
    .bind(job_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    media_asset_from_row(pool, creator_id, row).await
}

pub(crate) async fn fetch_media_asset_by_upload_id(
    pool: &SqlitePool,
    creator_id: &str,
    upload_id: &str,
) -> AppResult<MediaAsset> {
    reconcile_stale_media_processing_jobs_for_read(pool, Some(creator_id), None).await?;
    let row = sqlx::query(
        r#"
        SELECT id, upload_job_id, upload_id, series_id, kind, title, status, visibility,
               source_relative_path, poster_relative_path, playback_relative_path, mime_type,
               checksum_sha256, container_format, file_size_bytes, duration_sec, width, height,
               frame_rate, video_codec, audio_codec, has_video, has_audio, created_at, updated_at,
               processed_at, published_content_id
        FROM media_assets
        WHERE creator_id = ? AND upload_id = ?
        "#,
    )
    .bind(creator_id)
    .bind(upload_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    media_asset_from_row(pool, creator_id, row).await
}

pub(crate) async fn fetch_media_asset_by_id_any_creator(
    pool: &SqlitePool,
    asset_id: &str,
) -> AppResult<MediaAsset> {
    reconcile_stale_media_processing_jobs_for_read(pool, None, None).await?;
    let row = sqlx::query(
        r#"
        SELECT creator_id, id, upload_job_id, upload_id, series_id, kind, title, status, visibility,
               source_relative_path, poster_relative_path, playback_relative_path, mime_type,
               checksum_sha256, container_format, file_size_bytes, duration_sec, width, height,
               frame_rate, video_codec, audio_codec, has_video, has_audio, created_at, updated_at,
               processed_at, published_content_id
        FROM media_assets
        WHERE id = ?
        "#,
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let creator_id: String = row.get("creator_id");

    media_asset_from_row(pool, &creator_id, row).await
}

#[derive(Clone, Debug)]
pub(crate) struct StoredMediaPreviewTrack {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) image_relative_path: String,
    pub(crate) vtt_relative_path: String,
    pub(crate) tile_width: i64,
    pub(crate) tile_height: i64,
    pub(crate) columns_count: i64,
    pub(crate) rows_count: i64,
    pub(crate) interval_sec: f64,
    pub(crate) frame_count: i64,
    pub(crate) is_default: bool,
}

async fn media_asset_from_row(
    pool: &SqlitePool,
    creator_id: &str,
    row: sqlx::sqlite::SqliteRow,
) -> AppResult<MediaAsset> {
    let asset_id: String = row.get("id");
    let source_path: String = row.get("source_relative_path");
    let poster_path: Option<String> = row.get("poster_relative_path");
    let playback_path: Option<String> = row.get("playback_relative_path");
    let status: String = row.get("status");
    let audio_codec: Option<String> = row.get("audio_codec");
    let variants = fetch_media_asset_variants(pool, &asset_id).await?;
    let preview_track_rows = fetch_media_preview_track_rows(pool, &asset_id).await?;
    let audio_tracks = build_media_audio_tracks(
        &status,
        &asset_id,
        &variants,
        audio_codec.as_deref(),
        None,
        None,
        false,
    );
    let caption_tracks = build_media_caption_tracks(&status, &variants, None, None);
    let preview_tracks = build_media_preview_tracks(&status, &preview_track_rows, None);
    let default_audio_track_id = default_audio_track_id(&audio_tracks);
    let default_caption_track_id = default_caption_track_id(&caption_tracks);
    let default_preview_track_id = default_preview_track_id(&preview_tracks);
    Ok(MediaAsset {
        id: asset_id.clone(),
        upload_job_id: row.get("upload_job_id"),
        upload_id: row.get("upload_id"),
        series_id: row.get("series_id"),
        kind: row.get("kind"),
        title: row.get("title"),
        status,
        visibility: row.get("visibility"),
        source_url: media_api_url(&source_path),
        source_path,
        poster_url: poster_path.as_ref().map(|path| media_api_url(path)),
        poster_path,
        playback_url: playback_path.as_ref().map(|path| media_api_url(path)),
        playback_path,
        mime_type: row.get("mime_type"),
        checksum_sha256: row.get("checksum_sha256"),
        container_format: row.get("container_format"),
        file_size_bytes: row.get("file_size_bytes"),
        duration_sec: row.get("duration_sec"),
        width: row.get("width"),
        height: row.get("height"),
        frame_rate: row.get("frame_rate"),
        video_codec: row.get("video_codec"),
        audio_codec,
        has_video: row.get::<i64, _>("has_video") == 1,
        has_audio: row.get::<i64, _>("has_audio") == 1,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        processed_at: row.get("processed_at"),
        published_content_id: row.get("published_content_id"),
        variants,
        audio_tracks,
        caption_tracks,
        preview_tracks,
        default_audio_track_id,
        default_caption_track_id,
        default_preview_track_id,
        processing_runs: fetch_media_processing_runs(pool, creator_id, &asset_id).await?,
    })
}

pub(crate) async fn fetch_media_asset_variants(
    pool: &SqlitePool,
    asset_id: &str,
) -> AppResult<Vec<MediaAssetVariant>> {
    ensure_media_asset_thumbnail_variant(pool, asset_id).await?;
    let rows = sqlx::query(
        r#"
        SELECT id, variant_type, label, relative_path, mime_type, width, height, bitrate_bps,
               file_size_bytes, is_default, created_at
        FROM media_asset_variants
        WHERE asset_id = ?
        ORDER BY created_at ASC
        "#,
    )
    .bind(asset_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let relative_path: String = row.get("relative_path");
            MediaAssetVariant {
                id: row.get("id"),
                variant_type: row.get("variant_type"),
                label: row.get("label"),
                url: media_api_url(&relative_path),
                relative_path,
                mime_type: row.get("mime_type"),
                width: row.get("width"),
                height: row.get("height"),
                bitrate_bps: row.get("bitrate_bps"),
                file_size_bytes: row.get("file_size_bytes"),
                is_default: row.get::<i64, _>("is_default") == 1,
                created_at: row.get("created_at"),
            }
        })
        .collect())
}

async fn ensure_media_asset_thumbnail_variant(pool: &SqlitePool, asset_id: &str) -> AppResult<()> {
    let has_thumbnail = sqlx::query(
        "SELECT 1 FROM media_asset_variants WHERE asset_id = ? AND variant_type = 'thumbnail' LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await?
    .is_some();
    if has_thumbnail {
        return Ok(());
    }

    let row = sqlx::query(
        r#"
        SELECT poster_relative_path, width, height, created_at
        FROM media_assets
        WHERE id = ?
        "#,
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(());
    };
    let Some(poster_relative_path) = row.get::<Option<String>, _>("poster_relative_path") else {
        return Ok(());
    };

    sqlx::query(
        r#"
        INSERT INTO media_asset_variants (
            id, asset_id, variant_type, label, relative_path, mime_type, width, height,
            bitrate_bps, file_size_bytes, is_default, created_at
        ) VALUES (?, ?, 'thumbnail', 'card_thumbnail', ?, 'image/jpeg', ?, ?, NULL, 0, 1, ?)
        "#,
    )
    .bind(format!("var-{}", Uuid::new_v4().simple()))
    .bind(asset_id)
    .bind(poster_relative_path)
    .bind(row.get::<Option<i64>, _>("width"))
    .bind(row.get::<Option<i64>, _>("height"))
    .bind(row.get::<String, _>("created_at"))
    .execute(pool)
    .await?;

    Ok(())
}

pub(crate) async fn fetch_media_processing_runs(
    pool: &SqlitePool,
    creator_id: &str,
    asset_id: &str,
) -> AppResult<Vec<MediaProcessingRun>> {
    let rows = sqlx::query(
        r#"
        SELECT id, stage, status, details_json, started_at, completed_at
        FROM media_processing_runs
        WHERE creator_id = ? AND asset_id = ?
        ORDER BY started_at DESC
        "#,
    )
    .bind(creator_id)
    .bind(asset_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| MediaProcessingRun {
            id: row.get("id"),
            stage: row.get("stage"),
            status: row.get("status"),
            details: serde_json::from_str(&row.get::<String, _>("details_json"))
                .unwrap_or(json!({})),
            started_at: row.get("started_at"),
            completed_at: row.get("completed_at"),
        })
        .collect())
}

pub(crate) async fn fetch_media_preview_track_rows(
    pool: &SqlitePool,
    asset_id: &str,
) -> AppResult<Vec<StoredMediaPreviewTrack>> {
    let rows = sqlx::query(
        r#"
        SELECT id, label, image_relative_path, vtt_relative_path, tile_width, tile_height,
               columns_count, rows_count, interval_sec, frame_count, is_default, created_at
        FROM media_timeline_previews
        WHERE asset_id = ?
        ORDER BY created_at ASC
        "#,
    )
    .bind(asset_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| StoredMediaPreviewTrack {
            id: row.get("id"),
            label: row.get("label"),
            image_relative_path: row.get("image_relative_path"),
            vtt_relative_path: row.get("vtt_relative_path"),
            tile_width: row.get("tile_width"),
            tile_height: row.get("tile_height"),
            columns_count: row.get("columns_count"),
            rows_count: row.get("rows_count"),
            interval_sec: row.get("interval_sec"),
            frame_count: row.get("frame_count"),
            is_default: row.get::<i64, _>("is_default") == 1,
        })
        .collect())
}

pub(crate) struct NewMediaVariant {
    pub(crate) variant_type: &'static str,
    pub(crate) label: String,
    pub(crate) relative_path: String,
    pub(crate) mime_type: String,
    pub(crate) width: Option<i64>,
    pub(crate) height: Option<i64>,
    pub(crate) bitrate_bps: Option<i64>,
    pub(crate) file_size_bytes: i64,
    pub(crate) is_default: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct NewMediaPreviewTrack {
    pub(crate) label: String,
    pub(crate) image_relative_path: String,
    pub(crate) vtt_relative_path: String,
    pub(crate) tile_width: i64,
    pub(crate) tile_height: i64,
    pub(crate) columns_count: i64,
    pub(crate) rows_count: i64,
    pub(crate) interval_sec: f64,
    pub(crate) frame_count: i64,
    pub(crate) is_default: bool,
}

pub(crate) async fn replace_media_variants(
    pool: &SqlitePool,
    asset_id: &str,
    variants: &[NewMediaVariant],
) -> AppResult<()> {
    sqlx::query("DELETE FROM media_asset_variants WHERE asset_id = ?")
        .bind(asset_id)
        .execute(pool)
        .await?;

    let now = Utc::now().to_rfc3339();
    for variant in variants {
        sqlx::query(
            r#"
            INSERT INTO media_asset_variants (
                id, asset_id, variant_type, label, relative_path, mime_type, width, height,
                bitrate_bps, file_size_bytes, is_default, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(format!("var-{}", Uuid::new_v4().simple()))
        .bind(asset_id)
        .bind(variant.variant_type)
        .bind(&variant.label)
        .bind(&variant.relative_path)
        .bind(&variant.mime_type)
        .bind(variant.width)
        .bind(variant.height)
        .bind(variant.bitrate_bps)
        .bind(variant.file_size_bytes)
        .bind(variant.is_default as i64)
        .bind(&now)
        .execute(pool)
        .await?;
    }

    Ok(())
}

pub(crate) async fn replace_media_preview_tracks(
    pool: &SqlitePool,
    asset_id: &str,
    tracks: &[NewMediaPreviewTrack],
) -> AppResult<()> {
    sqlx::query("DELETE FROM media_timeline_previews WHERE asset_id = ?")
        .bind(asset_id)
        .execute(pool)
        .await?;

    let now = Utc::now().to_rfc3339();
    for track in tracks {
        sqlx::query(
            r#"
            INSERT INTO media_timeline_previews (
                id, asset_id, label, image_relative_path, vtt_relative_path, tile_width,
                tile_height, columns_count, rows_count, interval_sec, frame_count, is_default,
                created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(format!("mtp-{}", Uuid::new_v4().simple()))
        .bind(asset_id)
        .bind(&track.label)
        .bind(&track.image_relative_path)
        .bind(&track.vtt_relative_path)
        .bind(track.tile_width)
        .bind(track.tile_height)
        .bind(track.columns_count)
        .bind(track.rows_count)
        .bind(track.interval_sec)
        .bind(track.frame_count)
        .bind(track.is_default as i64)
        .bind(&now)
        .execute(pool)
        .await?;
    }

    Ok(())
}

pub(crate) async fn start_media_processing_run(
    pool: &SqlitePool,
    creator_id: &str,
    job_id: &str,
    asset_id: &str,
    stage: &str,
    details: Value,
) -> AppResult<String> {
    let id = format!("mpr-{}", Uuid::new_v4().simple());
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO media_processing_runs (
            id, creator_id, upload_job_id, asset_id, stage, status, details_json, started_at, completed_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(creator_id)
    .bind(job_id)
    .bind(asset_id)
    .bind(stage)
    .bind("running")
    .bind(details.to_string())
    .bind(&now)
    .bind(Option::<String>::None)
    .execute(pool)
    .await?;
    Ok(id)
}

pub(crate) async fn finish_media_processing_run(
    pool: &SqlitePool,
    run_id: &str,
    status: &str,
    details: Value,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE media_processing_runs SET status = ?, details_json = ?, completed_at = ? WHERE id = ?",
    )
    .bind(status)
    .bind(details.to_string())
    .bind(&now)
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}
