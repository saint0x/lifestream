use super::*;

const CREATOR_LIVE_HEALTH_SAMPLE_LIMIT: i64 = 24;

pub(crate) async fn ensure_creator_live_settings_row(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO creator_live_settings (
            creator_id, subscriber_only, slow_mode_seconds, auto_mod_level, notify_followers_default,
            delivery_class, active_scene_id, scenes_json, bitrate_kbps, cpu_percent, dropped_frames, free_disk_gb
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(creator_id)
    .bind(1_i64)
    .bind(3_i64)
    .bind("standard")
    .bind(1_i64)
    .bind("standard_hls")
    .bind("cam-main")
    .bind(
        json!([
            {"id":"cam-main","label":"Main cam","active":true},
            {"id":"screen","label":"Screen + cam","active":false},
            {"id":"slide","label":"Slideshow","active":false},
            {"id":"brb","label":"BRB loop","active":false}
        ])
        .to_string(),
    )
    .bind(0_i64)
    .bind(0_i64)
    .bind(0_i64)
    .bind(0.0_f64)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn fetch_creator_live_settings(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorLiveSettings> {
    ensure_creator_live_settings_row(pool, creator_id).await?;
    let row = sqlx::query(
        r#"
        SELECT subscriber_only, slow_mode_seconds, auto_mod_level, notify_followers_default,
               delivery_class, active_scene_id, scenes_json
        FROM creator_live_settings
        WHERE creator_id = ?
        "#,
    )
    .bind(creator_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(CreatorLiveSettings {
        subscriber_only: row.get::<i64, _>("subscriber_only") == 1,
        slow_mode_seconds: row.get("slow_mode_seconds"),
        auto_mod_level: row.get("auto_mod_level"),
        notify_followers_default: row.get::<i64, _>("notify_followers_default") == 1,
        delivery_class: row.get("delivery_class"),
        active_scene_id: row.get("active_scene_id"),
        scenes: from_json(row.get::<String, _>("scenes_json"))?,
    })
}

pub(crate) async fn fetch_creator_live_health(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorLiveHealth> {
    ensure_creator_live_settings_row(pool, creator_id).await?;
    let settings_row = sqlx::query(
        "SELECT bitrate_kbps, cpu_percent, dropped_frames, free_disk_gb FROM creator_live_settings WHERE creator_id = ?",
    )
    .bind(creator_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let sample_rows = sqlx::query(
        r#"
        SELECT collected_at, bitrate_kbps, viewers, cpu_percent, dropped_frames, free_disk_gb
        FROM creator_stream_health_samples
        WHERE creator_id = ?
        ORDER BY collected_at DESC
        LIMIT ?
        "#,
    )
    .bind(creator_id)
    .bind(CREATOR_LIVE_HEALTH_SAMPLE_LIMIT)
    .fetch_all(pool)
    .await?;
    let mut samples = sample_rows
        .into_iter()
        .map(|row| CreatorHealthSample {
            collected_at: row.get("collected_at"),
            bitrate_kbps: row.get("bitrate_kbps"),
            viewers: row.get("viewers"),
            cpu_percent: row.get("cpu_percent"),
            dropped_frames: row.get("dropped_frames"),
            free_disk_gb: row.get("free_disk_gb"),
        })
        .collect::<Vec<_>>();
    samples.reverse();

    Ok(CreatorLiveHealth {
        current_bitrate_kbps: settings_row.get("bitrate_kbps"),
        current_cpu_percent: settings_row.get("cpu_percent"),
        current_dropped_frames: settings_row.get("dropped_frames"),
        current_free_disk_gb: settings_row.get("free_disk_gb"),
        samples,
    })
}
