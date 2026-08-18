use chrono::{Duration, NaiveDate, Utc};
use sqlx::SqlitePool;

use super::support::{asset, json};

pub(super) async fn seed_creator(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let creator_id = "crt-deepsaint";
    sqlx::query(
        r#"
        INSERT INTO creator_profiles (
            id, user_id, handle, display_name, avatar, banner, tagline, bio, partner_status,
            joined_at, stream_key, rtmp_url, default_category, default_tags_json,
            followers, subscribers, monthly_viewers, total_watch_hours, live_status, current_broadcast_id
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(creator_id)
    .bind("usr-1")
    .bind("deepsaint")
    .bind("deepsaint")
    .bind(asset("avatar", "deepsaint"))
    .bind(asset("backdrop", "deepsaint-banner"))
    .bind("Systems & cinematic storytelling · live + recorded")
    .bind("I ship prestige series on the same deck I run systems streams from. Expect rust, sci-fi, and a lot of terminal pane re-tiling.")
    .bind("partner")
    .bind("2024-11-03")
    .bind("live_sk_83f2b1d7c9a4e5f6b8c2d4e6f8a0b1c3")
    .bind("rtmp://ingest.lifestream.tv/app")
    .bind("Tech")
    .bind(json(&vec!["rust", "systems", "english"])?)
    .bind(412_803_i64)
    .bind(3_412_i64)
    .bind(1_204_551_i64)
    .bind(84_210_i64)
    .bind("live")
    .bind("bcast-now")
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO creator_operational_state (
            creator_id, legal_name, support_email, business_type, payout_country, payout_provider,
            onboarding_status, identity_status, tax_status, payout_status, hold_reasons_json,
            created_at, updated_at, last_reviewed_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(creator_id)
    .bind("Deep Saint Media LLC")
    .bind("ops@deepsaint.media")
    .bind("company")
    .bind("US")
    .bind("stripe")
    .bind("approved")
    .bind("verified")
    .bind("verified")
    .bind("active")
    .bind(json(&Vec::<String>::new())?)
    .bind("2026-08-01T12:00:00Z")
    .bind("2026-08-16T12:00:00Z")
    .bind(Some("2026-08-16T12:00:00Z"))
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO creator_profiles (
            id, user_id, handle, display_name, avatar, banner, tagline, bio, partner_status,
            joined_at, stream_key, rtmp_url, default_category, default_tags_json,
            followers, subscribers, monthly_viewers, total_watch_hours, live_status, current_broadcast_id
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("crt-atlas")
    .bind("usr-2")
    .bind("atlas_codes")
    .bind("atlas_codes")
    .bind(asset("avatar", "atlas"))
    .bind(asset("backdrop", "atlas-banner"))
    .bind("Competitive systems, co-streams, and late-night code")
    .bind("I build multiplayer systems, host co-stream labs, and jump into collaboration sessions when something gnarly needs to ship.")
    .bind("affiliate")
    .bind("2025-02-17")
    .bind("live_sk_2d1b7f83c0e84577a14d01ef0038c842")
    .bind("rtmp://ingest.lifestream.tv/app")
    .bind("Gaming")
    .bind(json(&vec!["collab", "fps", "systems"])?)
    .bind(188_204_i64)
    .bind(1_108_i64)
    .bind(402_118_i64)
    .bind(21_404_i64)
    .bind("offline")
    .bind(Option::<&str>::None)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO creator_operational_state (
            creator_id, legal_name, support_email, business_type, payout_country, payout_provider,
            onboarding_status, identity_status, tax_status, payout_status, hold_reasons_json,
            created_at, updated_at, last_reviewed_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("crt-atlas")
    .bind("Atlas Codes")
    .bind("hello@atlascodes.dev")
    .bind("individual")
    .bind("US")
    .bind("stripe")
    .bind("in_review")
    .bind("submitted")
    .bind("pending")
    .bind("pending")
    .bind(json(&vec!["tax_profile_missing".to_string()])?)
    .bind("2026-08-02T09:00:00Z")
    .bind("2026-08-16T09:00:00Z")
    .bind(Some("2026-08-16T09:00:00Z"))
    .execute(pool)
    .await?;

    let now = Utc::now();
    let broadcasts = vec![
        (
            "bcast-now",
            creator_id,
            "writing a distributed job queue in rust from scratch",
            "Tech",
            vec!["rust", "tokio", "systems"],
            "live",
            now - Duration::minutes(138),
            None,
            None,
            15_402_i64,
            12_810_i64,
            18_422_i64,
            842_i64,
            61_i64,
            1_402.55_f64,
            "bcast-now",
            0_i64,
        ),
        (
            "bcast-scheduled-1",
            creator_id,
            "northlight s2e5 — post-episode commentary",
            "Talk",
            vec!["series", "commentary", "spoilers"],
            "scheduled",
            now + Duration::hours(22),
            None,
            None,
            0_i64,
            0_i64,
            0_i64,
            0_i64,
            0_i64,
            0.0_f64,
            "bcast-schedule-1",
            0_i64,
        ),
        (
            "bcast-ended-1",
            creator_id,
            "rust error handling — thiserror vs anyhow in 2026",
            "Tech",
            vec!["rust", "errors"],
            "ended",
            now - Duration::days(2),
            Some(now - Duration::days(2) + Duration::hours(6)),
            Some(21_600_i64),
            11_204_i64,
            8_741_i64,
            14_209_i64,
            512_i64,
            34_i64,
            948.12_f64,
            "bcast-ended-1",
            0_i64,
        ),
        (
            "bcast-ended-2",
            creator_id,
            "building a tiny OS in 4 hours — live coding",
            "Tech",
            vec!["os", "rust", "unsafe"],
            "ended",
            now - Duration::days(5),
            Some(now - Duration::days(5) + Duration::hours(4)),
            Some(14_400_i64),
            18_902_i64,
            14_220_i64,
            22_104_i64,
            1_221_i64,
            88_i64,
            1_854.30_f64,
            "bcast-ended-2",
            0_i64,
        ),
    ];

    for row in broadcasts {
        sqlx::query(
            r#"
            INSERT INTO broadcasts (
                id, creator_id, title, category, tags_json, status, started_at, ended_at, duration_sec,
                peak_viewers, average_viewers, chat_messages, new_followers, new_subscribers,
                revenue, thumbnail, is_mature
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(row.0)
        .bind(row.1)
        .bind(row.2)
        .bind(row.3)
        .bind(json(&row.4)?)
        .bind(row.5)
        .bind(row.6.to_rfc3339())
        .bind(row.7.map(|value| value.to_rfc3339()))
        .bind(row.8)
        .bind(row.9)
        .bind(row.10)
        .bind(row.11)
        .bind(row.12)
        .bind(row.13)
        .bind(row.14)
        .bind(asset("thumb", row.15))
        .bind(row.16)
        .execute(pool)
        .await?;
    }

    let uploads = vec![
        (
            "up-northlight-s2e4",
            creator_id,
            "Northlight — S2 · E4 · Meridian",
            "Halden digs into the ice archive. Voss takes a call nobody else hears. The signal returns.",
            "episode",
            3_120_i64,
            now - Duration::hours(26),
            Some(now - Duration::hours(24)),
            "published",
            "public",
            412_402_i64,
            38_211_i64,
            2_021_i64,
            14_400_i64,
            "up-nl-s2e4",
            Some("Northlight"),
            Some(2_i64),
            Some(4_i64),
            4_820_000_000_i64,
            "4K",
            None,
        ),
        (
            "up-halcyon-s1e2",
            creator_id,
            "Halcyon Drift — S1 · E2 · Cold Start",
            "The salvage contract opens a hatch that shouldn't be there.",
            "episode",
            2_880_i64,
            now - Duration::hours(4),
            None,
            "processing",
            "public",
            0_i64,
            0_i64,
            0_i64,
            0_i64,
            "up-hal-s1e2",
            Some("Halcyon Drift"),
            Some(1_i64),
            Some(2_i64),
            4_120_000_000_i64,
            "4K",
            Some(0.62_f64),
        ),
        (
            "up-vod-rust-queue",
            creator_id,
            "VOD — distributed job queue in rust (full 4h stream)",
            "Full replay of the live build. Chapters: setup, storage layer, scheduler, backpressure, retries.",
            "vod",
            14_200_i64,
            now - Duration::days(2),
            Some(now - Duration::days(2)),
            "published",
            "public",
            88_421_i64,
            7_021_i64,
            441_i64,
            21_402_i64,
            "up-vod-rust-queue",
            None,
            None,
            None,
            12_800_000_000_i64,
            "1080p",
            None,
        ),
        (
            "up-clip-linker",
            creator_id,
            "Clip — linker error arc (4 minutes of pain)",
            "The infamous LLD cascade. Shared in 2800 bookmarks.",
            "clip",
            240_i64,
            now - Duration::days(3),
            Some(now - Duration::days(3)),
            "published",
            "public",
            221_402_i64,
            14_220_i64,
            801_i64,
            1_840_i64,
            "up-clip-linker",
            None,
            None,
            None,
            210_000_000_i64,
            "1080p",
            None,
        ),
        (
            "up-draft-ep",
            creator_id,
            "Halcyon Drift — S1 · E3 · (untitled draft)",
            "Working cut — color not finalized, ADR pending on two scenes.",
            "episode",
            3_060_i64,
            now - Duration::days(1),
            None,
            "draft",
            "private",
            0_i64,
            0_i64,
            0_i64,
            0_i64,
            "up-draft-ep",
            Some("Halcyon Drift"),
            Some(1_i64),
            Some(3_i64),
            4_240_000_000_i64,
            "4K",
            None,
        ),
    ];

    for row in uploads {
        sqlx::query(
            r#"
            INSERT INTO uploads (
                id, creator_id, title, description, kind, duration_sec, uploaded_at, published_at, status,
                visibility, views, likes, comments, watch_hours, thumbnail, series_title,
                season_number, episode_number, size_bytes, resolution, transcode_progress
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(row.0)
        .bind(row.1)
        .bind(row.2)
        .bind(row.3)
        .bind(row.4)
        .bind(row.5)
        .bind(row.6.to_rfc3339())
        .bind(row.7.map(|value| value.to_rfc3339()))
        .bind(row.8)
        .bind(row.9)
        .bind(row.10)
        .bind(row.11)
        .bind(row.12)
        .bind(row.13)
        .bind(asset("thumb", row.14))
        .bind(row.15)
        .bind(row.16)
        .bind(row.17)
        .bind(row.18)
        .bind(row.19)
        .bind(row.20)
        .execute(pool)
        .await?;
    }

    for offset in 0..30_i64 {
        let day = Utc::now().date_naive() - Duration::days(29 - offset);
        sqlx::query(
            "INSERT INTO analytics_points (id, date, viewers, watch_minutes, revenue, new_followers, creator_id) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(format!("ap-{offset}"))
        .bind(day.to_string())
        .bind(21_000_i64 + offset * 890_i64)
        .bind(84_000_i64 + offset * 2_200_i64)
        .bind(420.0_f64 + (offset as f64 * 28.5_f64))
        .bind(90_i64 + offset * 5_i64)
        .bind(creator_id)
        .execute(pool)
        .await?;
    }

    for row in [
        ("home", 84_220_i64, 0.39_f64),
        ("search", 46_110_i64, 0.21_f64),
        ("recommendations", 33_200_i64, 0.15_f64),
        ("following", 28_420_i64, 0.13_f64),
        ("external", 24_110_i64, 0.11_f64),
    ] {
        sqlx::query(
            "INSERT INTO traffic_sources (source, sessions, share, creator_id) VALUES (?, ?, ?, ?)",
        )
        .bind(row.0)
        .bind(row.1)
        .bind(row.2)
        .bind(creator_id)
        .execute(pool)
        .await?;
    }

    for row in [
        (
            "up-trending-1",
            "Clip — linker error arc (4 minutes of pain)",
            "clip",
            221_402_i64,
            1_840_i64,
            18.4_f64,
            asset("thumb", "up-clip-linker"),
        ),
        (
            "up-trending-2",
            "VOD — distributed job queue in rust (full 4h stream)",
            "vod",
            88_421_i64,
            21_402_i64,
            9.2_f64,
            asset("thumb", "up-vod-rust-queue"),
        ),
        (
            "bcast-now",
            "writing a distributed job queue in rust from scratch",
            "live",
            54_210_i64,
            12_810_i64,
            6.8_f64,
            asset("thumb", "bcast-now"),
        ),
    ] {
        sqlx::query(
            "INSERT INTO top_content (id, title, kind, views, watch_hours, trend, thumbnail, creator_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(row.0)
        .bind(row.1)
        .bind(row.2)
        .bind(row.3)
        .bind(row.4)
        .bind(row.5)
        .bind(row.6)
        .bind(creator_id)
        .execute(pool)
        .await?;
    }

    for row in [
        (
            "rev-1",
            NaiveDate::from_ymd_opt(2026, 8, 13).unwrap(),
            "subscriptions",
            "August partner subscriptions",
            2_841.42_f64,
        ),
        (
            "rev-2",
            NaiveDate::from_ymd_opt(2026, 8, 12).unwrap(),
            "ads",
            "Mid-roll ad revenue",
            612.10_f64,
        ),
        (
            "rev-3",
            NaiveDate::from_ymd_opt(2026, 8, 12).unwrap(),
            "tips",
            "Live tips and cheers",
            428.00_f64,
        ),
        (
            "rev-4",
            NaiveDate::from_ymd_opt(2026, 8, 10).unwrap(),
            "payout",
            "Weekly creator payout transfer",
            -1_950.00_f64,
        ),
    ] {
        sqlx::query(
            "INSERT INTO revenue_entries (id, date, source, description, amount, creator_id) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(row.0)
        .bind(row.1.to_string())
        .bind(row.2)
        .bind(row.3)
        .bind(row.4)
        .bind(creator_id)
        .execute(pool)
        .await?;
    }

    for row in [
        (
            "nt-1",
            "milestone",
            "Crossed 400k followers.",
            "2026-08-13T14:22:00Z",
            None,
            None,
        ),
        (
            "nt-2",
            "subscriber",
            "New gifted subscription wave from atlas_codes",
            "2026-08-13T13:10:00Z",
            None,
            Some("atlas_codes"),
        ),
        (
            "nt-3",
            "tip",
            "New tip during the Rust queue stream",
            "2026-08-13T12:44:00Z",
            Some(25.0_f64),
            Some("vector_lane"),
        ),
        (
            "nt-4",
            "system",
            "Your VOD transcode finished at 1080p and 720p.",
            "2026-08-13T11:02:00Z",
            None,
            None,
        ),
    ] {
        sqlx::query(
            "INSERT INTO creator_notifications (id, kind, body, sent_at, amount, actor, creator_id) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(row.0)
        .bind(row.1)
        .bind(row.2)
        .bind(row.3)
        .bind(row.4)
        .bind(row.5)
        .bind(creator_id)
        .execute(pool)
        .await?;
    }

    sqlx::query(
        r#"
        INSERT INTO creator_live_settings (
            creator_id, subscriber_only, slow_mode_seconds, auto_mod_level, notify_followers_default,
            active_scene_id, scenes_json, bitrate_kbps, cpu_percent, dropped_frames, free_disk_gb
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(creator_id)
    .bind(1_i64)
    .bind(3_i64)
    .bind("standard")
    .bind(1_i64)
    .bind("cam-main")
    .bind(json(&vec![
        serde_json::json!({"id":"cam-main","label":"Main cam","active":true}),
        serde_json::json!({"id":"screen","label":"Screen + cam","active":false}),
        serde_json::json!({"id":"slide","label":"Slideshow","active":false}),
        serde_json::json!({"id":"brb","label":"BRB loop","active":false}),
    ])?)
    .bind(6_020_i64)
    .bind(38_i64)
    .bind(0_i64)
    .bind(1_200.0_f64)
    .execute(pool)
    .await?;

    let viewer_history = [8421_i64, 9012, 10220, 11890, 13402, 14102, 15402];
    let bitrate_history = [5800_i64, 5920, 6040, 6000, 6100, 6080, 6020];
    for (idx, (viewers, bitrate)) in viewer_history
        .into_iter()
        .zip(bitrate_history.into_iter())
        .enumerate()
    {
        sqlx::query(
            r#"
            INSERT INTO creator_stream_health_samples (
                id, creator_id, collected_at, bitrate_kbps, viewers, cpu_percent, dropped_frames, free_disk_gb
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(format!("health-{idx}"))
        .bind(creator_id)
        .bind((now - Duration::minutes((6 - idx) as i64)).to_rfc3339())
        .bind(bitrate)
        .bind(viewers)
        .bind(34_i64 + idx as i64)
        .bind(0_i64)
        .bind(1_200.0_f64 - (idx as f64 * 0.8_f64))
        .execute(pool)
        .await?;
    }

    for row in [
        ("tier-1", "Tier 1", 1_i64, 4.99_f64, 2_412_i64, "#4ea1ff"),
        ("tier-2", "Tier 2", 2_i64, 9.99_f64, 812_i64, "#ffd83d"),
        ("tier-3", "Tier 3", 3_i64, 24.99_f64, 188_i64, "#ff3d7a"),
    ] {
        sqlx::query(
            "INSERT INTO creator_subscriber_tiers (id, creator_id, tier_name, rank, monthly_price, subscriber_count, accent_color) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(row.0)
        .bind(creator_id)
        .bind(row.1)
        .bind(row.2)
        .bind(row.3)
        .bind(row.4)
        .bind(row.5)
        .execute(pool)
        .await?;
    }

    sqlx::query(
        "INSERT INTO live_stream_notification_preferences (user_id, streamer_id, enabled, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind("usr-1")
    .bind("str-deepsaint")
    .bind(1_i64)
    .bind("2026-08-13T19:04:00Z")
    .execute(pool)
    .await?;

    Ok(())
}
