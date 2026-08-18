use chrono::Utc;
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

use crate::models::ImageSet;

use super::support::{asset, json, slugify};

pub(super) async fn ensure_public_catalog_playback(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    const PLATFORM_USER_ID: &str = "usr-platform";
    const PLATFORM_CREATOR_ID: &str = "crt-platform";

    sqlx::query(
        "INSERT OR IGNORE INTO users (id, handle, display_name, avatar, tier, joined_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(PLATFORM_USER_ID)
    .bind("lifestream")
    .bind("Lifestream")
    .bind(asset("avatar", "platform"))
    .bind("premium")
    .bind("2026-08-01T00:00:00Z")
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO creator_profiles (
            id, user_id, handle, display_name, avatar, banner, tagline, bio, partner_status,
            joined_at, stream_key, rtmp_url, default_category, default_tags_json, followers,
            subscribers, monthly_viewers, total_watch_hours, live_status, current_broadcast_id
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(PLATFORM_CREATOR_ID)
    .bind(PLATFORM_USER_ID)
    .bind("lifestream")
    .bind("Lifestream")
    .bind(asset("avatar", "platform"))
    .bind(asset("backdrop", "platform"))
    .bind("Platform-owned premiere catalog and editorial programming.")
    .bind("Lifestream platform-owned long-form catalog and premiere programming.")
    .bind("partner")
    .bind("2026-08-01T00:00:00Z")
    .bind("live_sk_platform_catalog")
    .bind("rtmp://ingest.lifestream.tv/app")
    .bind("Drama")
    .bind(json(&vec!["platform".to_string(), "catalog".to_string()])?)
    .bind(0_i64)
    .bind(0_i64)
    .bind(0_i64)
    .bind(0_i64)
    .bind("offline")
    .bind(Option::<&str>::None)
    .execute(pool)
    .await?;

    let sample_assets = sqlx::query(
        r#"
        SELECT source_relative_path, poster_relative_path, playback_relative_path, mime_type,
               checksum_sha256, container_format, file_size_bytes, width, height, frame_rate,
               video_codec, audio_codec, has_video, has_audio
        FROM media_assets
        WHERE status IN ('ready', 'published')
          AND poster_relative_path IS NOT NULL
          AND playback_relative_path IS NOT NULL
        ORDER BY created_at ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    if sample_assets.is_empty() {
        return Ok(());
    }

    let now = Utc::now().to_rfc3339();

    let film_rows = sqlx::query(
        r#"
        SELECT id, slug, title, synopsis, duration_sec, images_json
        FROM films
        ORDER BY year DESC, id ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    for (idx, row) in film_rows.into_iter().enumerate() {
        let sample = &sample_assets[idx % sample_assets.len()];
        let content_id: String = row.get("id");
        let images: ImageSet = serde_json::from_str(&row.get::<String, _>("images_json"))
            .map_err(|error| sqlx::Error::Decode(Box::new(error)))?;
        let title: String = row.get("title");
        let synopsis: String = row.get("synopsis");
        let duration_sec: i64 = row.get("duration_sec");
        let slug: String = row.get("slug");
        let file_size_bytes: i64 = sample.get("file_size_bytes");

        sqlx::query(
            r#"
            INSERT OR IGNORE INTO uploads (
                id, creator_id, title, description, kind, duration_sec, uploaded_at, published_at,
                status, visibility, views, likes, comments, watch_hours, thumbnail, series_title,
                season_number, episode_number, size_bytes, resolution, transcode_progress, slug,
                release_at, access_policy, currency
            ) VALUES (?, ?, ?, ?, 'film', ?, ?, ?, 'published', 'public', 0, 0, 0, 0, ?, NULL,
                      NULL, NULL, ?, '1080p', NULL, ?, ?, 'free', 'USD')
            "#,
        )
        .bind(&content_id)
        .bind(PLATFORM_CREATOR_ID)
        .bind(&title)
        .bind(&synopsis)
        .bind(duration_sec)
        .bind(&now)
        .bind(&now)
        .bind(&images.thumbnail)
        .bind(file_size_bytes)
        .bind(&slug)
        .bind(&now)
        .execute(pool)
        .await?;

        upsert_public_catalog_transport_records(
            pool,
            PLATFORM_CREATOR_ID,
            &content_id,
            &content_id,
            "film",
            &title,
            file_size_bytes,
            duration_sec as f64,
            sample,
            &now,
        )
        .await?;
    }

    let episode_rows = sqlx::query(
        r#"
        SELECT e.id, e.series_id, s.title AS series_title, e.season_number, e.episode_number,
               e.title, e.synopsis, e.duration_sec, e.aired_at, e.thumbnail
        FROM episodes e
        JOIN series s ON s.id = e.series_id
        ORDER BY e.series_id ASC, e.season_number ASC, e.episode_number ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    for (idx, row) in episode_rows.into_iter().enumerate() {
        let sample = &sample_assets[(idx + 1) % sample_assets.len()];
        let content_id: String = row.get("id");
        let series_title: String = row.get("series_title");
        let season_number: i64 = row.get("season_number");
        let episode_number: i64 = row.get("episode_number");
        let episode_title: String = row.get("title");
        let synopsis: String = row.get("synopsis");
        let duration_sec: i64 = row.get("duration_sec");
        let aired_at: String = row.get("aired_at");
        let thumbnail: String = row.get("thumbnail");
        let upload_title =
            format!("{series_title} — S{season_number} · E{episode_number} · {episode_title}");
        let file_size_bytes: i64 = sample.get("file_size_bytes");

        sqlx::query(
            r#"
            INSERT OR IGNORE INTO uploads (
                id, creator_id, title, description, kind, duration_sec, uploaded_at, published_at,
                status, visibility, views, likes, comments, watch_hours, thumbnail, series_title,
                season_number, episode_number, size_bytes, resolution, transcode_progress, slug,
                release_at, access_policy, currency
            ) VALUES (?, ?, ?, ?, 'episode', ?, ?, ?, 'published', 'public', 0, 0, 0, 0, ?, ?,
                      ?, ?, ?, '1080p', NULL, ?, ?, 'free', 'USD')
            "#,
        )
        .bind(&content_id)
        .bind(PLATFORM_CREATOR_ID)
        .bind(&upload_title)
        .bind(&synopsis)
        .bind(duration_sec)
        .bind(format!("{aired_at}T00:00:00Z"))
        .bind(format!("{aired_at}T00:00:00Z"))
        .bind(&thumbnail)
        .bind(&series_title)
        .bind(season_number)
        .bind(episode_number)
        .bind(file_size_bytes)
        .bind(slugify(&upload_title))
        .bind(format!("{aired_at}T00:00:00Z"))
        .execute(pool)
        .await?;

        upsert_public_catalog_transport_records(
            pool,
            PLATFORM_CREATOR_ID,
            &format!("job-{content_id}"),
            &content_id,
            "episode",
            &upload_title,
            file_size_bytes,
            duration_sec as f64,
            sample,
            &format!("{aired_at}T00:00:00Z"),
        )
        .await?;
    }

    Ok(())
}

async fn upsert_public_catalog_transport_records(
    pool: &SqlitePool,
    creator_id: &str,
    job_id: &str,
    content_id: &str,
    kind: &str,
    title: &str,
    file_size_bytes: i64,
    duration_sec: f64,
    sample: &SqliteRow,
    timestamp: &str,
) -> Result<(), sqlx::Error> {
    let source_relative_path: String = sample.get("source_relative_path");
    let poster_relative_path: String = sample.get("poster_relative_path");
    let playback_relative_path: String = sample.get("playback_relative_path");

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO upload_jobs (
            id, creator_id, upload_id, series_id, kind, source_type, status, title,
            intended_visibility, bytes_expected, bytes_received, storage_key, created_at,
            updated_at, published_content_id, mime_type, checksum_sha256, completed_at,
            processing_attempt_count
        ) VALUES (?, ?, ?, NULL, ?, 'seeded-catalog', 'published', ?, 'public', ?, ?, ?, ?, ?, ?,
                  ?, ?, ?, 1)
        "#,
    )
    .bind(job_id)
    .bind(creator_id)
    .bind(content_id)
    .bind(kind)
    .bind(title)
    .bind(file_size_bytes)
    .bind(file_size_bytes)
    .bind(&source_relative_path)
    .bind(timestamp)
    .bind(timestamp)
    .bind(content_id)
    .bind(sample.get::<String, _>("mime_type"))
    .bind(sample.get::<Option<String>, _>("checksum_sha256"))
    .bind(timestamp)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO media_assets (
            id, creator_id, upload_job_id, upload_id, series_id, kind, title, status, visibility,
            source_relative_path, poster_relative_path, playback_relative_path, mime_type,
            checksum_sha256, container_format, file_size_bytes, duration_sec, width, height,
            frame_rate, video_codec, audio_codec, has_video, has_audio, created_at, updated_at,
            processed_at, published_content_id
        ) VALUES (?, ?, ?, ?, NULL, ?, ?, 'published', 'public', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                  ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(format!("ast-{content_id}"))
    .bind(creator_id)
    .bind(job_id)
    .bind(content_id)
    .bind(kind)
    .bind(title)
    .bind(&source_relative_path)
    .bind(&poster_relative_path)
    .bind(&playback_relative_path)
    .bind(sample.get::<String, _>("mime_type"))
    .bind(sample.get::<Option<String>, _>("checksum_sha256"))
    .bind(sample.get::<Option<String>, _>("container_format"))
    .bind(file_size_bytes)
    .bind(duration_sec)
    .bind(sample.get::<Option<i64>, _>("width"))
    .bind(sample.get::<Option<i64>, _>("height"))
    .bind(sample.get::<Option<f64>, _>("frame_rate"))
    .bind(sample.get::<Option<String>, _>("video_codec"))
    .bind(sample.get::<Option<String>, _>("audio_codec"))
    .bind(sample.get::<i64, _>("has_video"))
    .bind(sample.get::<i64, _>("has_audio"))
    .bind(timestamp)
    .bind(timestamp)
    .bind(timestamp)
    .bind(content_id)
    .execute(pool)
    .await?;

    sqlx::query(
        "UPDATE upload_jobs SET published_content_id = ?, upload_id = ?, status = 'published', completed_at = COALESCE(completed_at, ?), updated_at = ? WHERE id = ?",
    )
    .bind(content_id)
    .bind(content_id)
    .bind(timestamp)
    .bind(timestamp)
    .bind(job_id)
    .execute(pool)
    .await?;

    sqlx::query(
        "UPDATE media_assets SET upload_id = ?, published_content_id = ?, status = 'published', visibility = 'public', updated_at = ?, processed_at = COALESCE(processed_at, ?) WHERE id = ?",
    )
    .bind(content_id)
    .bind(content_id)
    .bind(timestamp)
    .bind(timestamp)
    .bind(format!("ast-{content_id}"))
    .execute(pool)
    .await?;

    Ok(())
}

pub(super) async fn ensure_live_stream_playback(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let sample_assets = sqlx::query(
        r#"
        SELECT id, poster_relative_path, playback_relative_path
        FROM media_assets
        WHERE status IN ('ready', 'published')
          AND poster_relative_path IS NOT NULL
          AND playback_relative_path IS NOT NULL
        ORDER BY created_at ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    if sample_assets.is_empty() {
        return Ok(());
    }

    let live_stream_rows = sqlx::query(
        r#"
        SELECT id
        FROM live_streams
        WHERE id NOT LIKE '%-live'
        ORDER BY viewers DESC, started_at DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    for (idx, row) in live_stream_rows.into_iter().enumerate() {
        let sample = &sample_assets[idx % sample_assets.len()];
        sqlx::query(
            r#"
            UPDATE live_streams
            SET playback_asset_id = COALESCE(playback_asset_id, ?),
                poster_relative_path = COALESCE(poster_relative_path, ?),
                playback_relative_path = COALESCE(playback_relative_path, ?)
            WHERE id = ?
            "#,
        )
        .bind(sample.get::<String, _>("id"))
        .bind(sample.get::<String, _>("poster_relative_path"))
        .bind(sample.get::<String, _>("playback_relative_path"))
        .bind(row.get::<String, _>("id"))
        .execute(pool)
        .await?;
    }

    Ok(())
}
