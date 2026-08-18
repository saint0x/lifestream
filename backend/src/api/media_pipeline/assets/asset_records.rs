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
