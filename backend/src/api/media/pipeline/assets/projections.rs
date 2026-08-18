use super::*;

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
