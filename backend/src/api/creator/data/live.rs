use super::*;
use crate::models::CreatorScene;

const CREATOR_LIVE_HEALTH_SAMPLE_LIMIT: i64 = 12;

fn default_creator_live_scenes() -> Vec<CreatorScene> {
    vec![
        CreatorScene {
            id: "cam-main".to_string(),
            label: "Main cam".to_string(),
            active: true,
        },
        CreatorScene {
            id: "screen".to_string(),
            label: "Screen + cam".to_string(),
            active: false,
        },
        CreatorScene {
            id: "slide".to_string(),
            label: "Slideshow".to_string(),
            active: false,
        },
        CreatorScene {
            id: "brb".to_string(),
            label: "BRB loop".to_string(),
            active: false,
        },
    ]
}

fn default_creator_live_settings() -> CreatorLiveSettings {
    CreatorLiveSettings {
        subscriber_only: true,
        slow_mode_seconds: 3,
        auto_mod_level: "standard".to_string(),
        notify_followers_default: true,
        delivery_class: "standard_hls".to_string(),
        active_scene_id: "cam-main".to_string(),
        scenes: default_creator_live_scenes(),
    }
}

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
    .bind(to_json(&default_creator_live_scenes())?)
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
    .await?;

    match row {
        Some(row) => Ok(CreatorLiveSettings {
            subscriber_only: row.get::<i64, _>("subscriber_only") == 1,
            slow_mode_seconds: row.get("slow_mode_seconds"),
            auto_mod_level: row.get("auto_mod_level"),
            notify_followers_default: row.get::<i64, _>("notify_followers_default") == 1,
            delivery_class: row.get("delivery_class"),
            active_scene_id: row.get("active_scene_id"),
            scenes: from_json(row.get::<String, _>("scenes_json"))?,
        }),
        None => Ok(default_creator_live_settings()),
    }
}

pub(crate) async fn fetch_creator_live_health(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorLiveHealth> {
    let settings_row = sqlx::query(
        "SELECT bitrate_kbps, cpu_percent, dropped_frames, free_disk_gb FROM creator_live_settings WHERE creator_id = ?",
    )
    .bind(creator_id)
    .fetch_optional(pool)
    .await?;

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

    Ok(match settings_row {
        Some(settings_row) => CreatorLiveHealth {
            current_bitrate_kbps: settings_row.get("bitrate_kbps"),
            current_cpu_percent: settings_row.get("cpu_percent"),
            current_dropped_frames: settings_row.get("dropped_frames"),
            current_free_disk_gb: settings_row.get("free_disk_gb"),
            samples,
        },
        None => CreatorLiveHealth {
            current_bitrate_kbps: 0,
            current_cpu_percent: 0,
            current_dropped_frames: 0,
            current_free_disk_gb: 0.0,
            samples,
        },
    })
}
