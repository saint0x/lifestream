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

pub(crate) async fn ensure_media_asset_shell_for_database(
    database: &crate::db::Database,
    creator_id: &str,
    job: &UploadJob,
    source_relative_path: &str,
) -> AppResult<MediaAsset> {
    if let Ok(pool) = database.try_postgres_adapter() {
        return ensure_postgres_media_asset_shell(pool, creator_id, job, source_relative_path)
            .await;
    }
    ensure_media_asset_shell(
        database.try_sqlite_adapter()?,
        creator_id,
        job,
        source_relative_path,
    )
    .await
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

pub(crate) async fn fetch_media_assets_for_database(
    database: &crate::db::Database,
    creator_id: &str,
) -> AppResult<Vec<MediaAsset>> {
    if let Ok(pool) = database.try_postgres_adapter() {
        return fetch_postgres_media_assets(pool, creator_id).await;
    }
    fetch_media_assets(database.try_sqlite_adapter()?, creator_id).await
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

pub(crate) async fn fetch_media_asset_by_upload_job_for_database(
    database: &crate::db::Database,
    creator_id: &str,
    job_id: &str,
) -> AppResult<MediaAsset> {
    if let Ok(pool) = database.try_postgres_adapter() {
        return fetch_postgres_media_asset_by_upload_job(pool, creator_id, job_id).await;
    }
    fetch_media_asset_by_upload_job(database.try_sqlite_adapter()?, creator_id, job_id).await
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

pub(crate) async fn fetch_media_asset_by_upload_id_for_database(
    database: &crate::db::Database,
    creator_id: &str,
    upload_id: &str,
) -> AppResult<MediaAsset> {
    if let Ok(pool) = database.try_postgres_adapter() {
        return fetch_postgres_media_asset_by_upload_id(pool, creator_id, upload_id).await;
    }
    fetch_media_asset_by_upload_id(database.try_sqlite_adapter()?, creator_id, upload_id).await
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

async fn ensure_postgres_media_asset_shell(
    pool: &sqlx::PgPool,
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
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28)
        ON CONFLICT(upload_job_id) DO UPDATE SET
            upload_id = EXCLUDED.upload_id,
            series_id = EXCLUDED.series_id,
            kind = EXCLUDED.kind,
            title = EXCLUDED.title,
            visibility = EXCLUDED.visibility,
            source_relative_path = EXCLUDED.source_relative_path,
            mime_type = EXCLUDED.mime_type,
            checksum_sha256 = EXCLUDED.checksum_sha256,
            file_size_bytes = EXCLUDED.file_size_bytes,
            updated_at = EXCLUDED.updated_at
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
    .bind(0_i32)
    .bind(0_i32)
    .bind(&now)
    .bind(&now)
    .bind(Option::<String>::None)
    .bind(job.published_content_id.clone())
    .execute(pool)
    .await?;

    fetch_postgres_media_asset_by_upload_job(pool, creator_id, &job.id).await
}

async fn fetch_postgres_media_assets(
    pool: &sqlx::PgPool,
    creator_id: &str,
) -> AppResult<Vec<MediaAsset>> {
    let query = postgres_media_asset_select("WHERE creator_id = $1 ORDER BY created_at DESC");
    let rows = sqlx::query(&query).bind(creator_id).fetch_all(pool).await?;

    let mut assets = Vec::with_capacity(rows.len());
    for row in rows {
        assets.push(postgres_media_asset_from_row(pool, creator_id, row).await?);
    }
    Ok(assets)
}

async fn fetch_postgres_media_asset_by_upload_job(
    pool: &sqlx::PgPool,
    creator_id: &str,
    job_id: &str,
) -> AppResult<MediaAsset> {
    let query = postgres_media_asset_select("WHERE creator_id = $1 AND upload_job_id = $2");
    let row = sqlx::query(&query)
        .bind(creator_id)
        .bind(job_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;

    postgres_media_asset_from_row(pool, creator_id, row).await
}

async fn fetch_postgres_media_asset_by_upload_id(
    pool: &sqlx::PgPool,
    creator_id: &str,
    upload_id: &str,
) -> AppResult<MediaAsset> {
    let query = postgres_media_asset_select("WHERE creator_id = $1 AND upload_id = $2");
    let row = sqlx::query(&query)
        .bind(creator_id)
        .bind(upload_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;

    postgres_media_asset_from_row(pool, creator_id, row).await
}

fn postgres_media_asset_select(predicate: &str) -> String {
    format!(
        r#"
        SELECT id, upload_job_id, upload_id, series_id, kind, title, status, visibility,
               source_relative_path, poster_relative_path, playback_relative_path, mime_type,
               checksum_sha256, container_format, file_size_bytes::BIGINT AS file_size_bytes,
               duration_sec::DOUBLE PRECISION AS duration_sec,
               width::BIGINT AS width, height::BIGINT AS height,
               frame_rate::DOUBLE PRECISION AS frame_rate,
               video_codec, audio_codec, has_video, has_audio, created_at, updated_at,
               processed_at, published_content_id
        FROM media_assets
        {predicate}
        "#
    )
}

async fn postgres_media_asset_from_row(
    pool: &sqlx::PgPool,
    creator_id: &str,
    row: sqlx::postgres::PgRow,
) -> AppResult<MediaAsset> {
    let asset_id: String = row.get("id");
    let source_path: String = row.get("source_relative_path");
    let poster_path: Option<String> = row.get("poster_relative_path");
    let playback_path: Option<String> = row.get("playback_relative_path");
    let status: String = row.get("status");
    let audio_codec: Option<String> = row.get("audio_codec");
    let variants = fetch_postgres_media_asset_variants(pool, &asset_id).await?;
    let preview_track_rows = fetch_postgres_media_preview_track_rows(pool, &asset_id).await?;
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
        has_video: row.get::<i32, _>("has_video") == 1,
        has_audio: row.get::<i32, _>("has_audio") == 1,
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
        processing_runs: fetch_postgres_media_processing_runs(pool, creator_id, &asset_id).await?,
    })
}

async fn fetch_postgres_media_asset_variants(
    pool: &sqlx::PgPool,
    asset_id: &str,
) -> AppResult<Vec<MediaAssetVariant>> {
    let rows = sqlx::query(
        r#"
        SELECT id, variant_type, label, relative_path, mime_type,
               width::BIGINT AS width, height::BIGINT AS height,
               bitrate_bps::BIGINT AS bitrate_bps, file_size_bytes::BIGINT AS file_size_bytes,
               is_default, created_at
        FROM media_asset_variants
        WHERE asset_id = $1
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
                is_default: row.get::<i32, _>("is_default") == 1,
                created_at: row.get("created_at"),
            }
        })
        .collect())
}

async fn fetch_postgres_media_preview_track_rows(
    pool: &sqlx::PgPool,
    asset_id: &str,
) -> AppResult<Vec<StoredMediaPreviewTrack>> {
    let rows = sqlx::query(
        r#"
        SELECT id, label, image_relative_path, vtt_relative_path,
               tile_width::BIGINT AS tile_width, tile_height::BIGINT AS tile_height,
               columns_count::BIGINT AS columns_count, rows_count::BIGINT AS rows_count,
               interval_sec::DOUBLE PRECISION AS interval_sec,
               frame_count::BIGINT AS frame_count, is_default, created_at
        FROM media_timeline_previews
        WHERE asset_id = $1
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
            is_default: row.get::<i32, _>("is_default") == 1,
        })
        .collect())
}

async fn fetch_postgres_media_processing_runs(
    pool: &sqlx::PgPool,
    creator_id: &str,
    asset_id: &str,
) -> AppResult<Vec<MediaProcessingRun>> {
    let rows = sqlx::query(
        r#"
        SELECT id, stage, status, details_json, started_at, completed_at
        FROM media_processing_runs
        WHERE creator_id = $1 AND asset_id = $2
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
