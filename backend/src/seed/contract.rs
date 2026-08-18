use chrono::{Duration, Utc};
use sqlx::{Row, SqlitePool};

use super::{
    playback::{ensure_live_stream_playback, ensure_public_catalog_playback},
    support::{asset, json, slugify},
};

pub(super) async fn ensure_extended_contract_data(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let creator_id = "crt-deepsaint";
    let now = Utc::now();

    sqlx::query(
        "INSERT OR IGNORE INTO users (id, handle, display_name, avatar, tier, joined_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind("usr-2")
    .bind("atlas_codes")
    .bind("atlas_codes")
    .bind(asset("avatar", "atlas"))
    .bind("standard")
    .bind("2025-02-17")
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO creator_profiles (
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
        INSERT OR IGNORE INTO creator_operational_state (
            creator_id, legal_name, support_email, business_type, payout_country, payout_provider,
            onboarding_status, identity_status, tax_status, payout_status, hold_reasons_json,
            created_at, updated_at, last_reviewed_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("crt-deepsaint")
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
        INSERT OR IGNORE INTO creator_operational_state (
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

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO user_profiles (
            user_id, email, email_verified, mature_content_allowed, default_audio,
            subtitle_preset, autoplay_trailers, live_chat_filter, hours_watched
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("usr-1")
    .bind("deepsaint@lifestream.tv")
    .bind(1_i64)
    .bind(1_i64)
    .bind("English 5.1 (Dolby Atmos)")
    .bind("English · Large · High contrast")
    .bind(0_i64)
    .bind("Standard")
    .bind(142_i64)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO user_playback_settings (
            user_id, default_quality, audio_language, subtitle_language, subtitle_style,
            autoplay_next_episode, autoplay_trailers, reduced_motion, prefer_dubbed, playback_speed
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("usr-1")
    .bind("Auto (up to 4K HDR)")
    .bind("English · 5.1 (Dolby Atmos)")
    .bind("English")
    .bind("English · Large")
    .bind(1_i64)
    .bind(0_i64)
    .bind(0_i64)
    .bind(0_i64)
    .bind("1× (normal)")
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO user_notification_settings (
            user_id, series_push, series_email, live_push, live_email, originals_push,
            originals_email, watchlist_push, watchlist_email, creator_push, creator_email,
            security_push, security_email
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("usr-1")
    .bind(1_i64)
    .bind(0_i64)
    .bind(1_i64)
    .bind(0_i64)
    .bind(1_i64)
    .bind(1_i64)
    .bind(0_i64)
    .bind(0_i64)
    .bind(0_i64)
    .bind(1_i64)
    .bind(1_i64)
    .bind(1_i64)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO user_privacy_settings (
            user_id, show_friend_activity, improve_recommendations, personalized_ads,
            ab_tests, data_export_size_mb, delete_cooldown_days
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("usr-1")
    .bind(0_i64)
    .bind(1_i64)
    .bind(0_i64)
    .bind(1_i64)
    .bind(12.0_f64)
    .bind(30_i64)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO user_parental_controls (
            user_id, max_rating, require_pin_for_mature, hide_live_chat_for_kids,
            block_mature_live_streams, pin_set
        ) VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("usr-1")
    .bind("TV-MA / R")
    .bind(1_i64)
    .bind(1_i64)
    .bind(1_i64)
    .bind(1_i64)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO user_download_settings (
            user_id, video_quality, wifi_only, smart_downloads, storage_used_gb,
            storage_limit_gb, device_limit, active_devices
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("usr-1")
    .bind("High (1080p)")
    .bind(1_i64)
    .bind(1_i64)
    .bind(4.2_f64)
    .bind(50.0_f64)
    .bind(4_i64)
    .bind(2_i64)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO user_language_settings (
            user_id, interface_language, subtitle_language, catalog_region, date_format, clock_format
        ) VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("usr-1")
    .bind("English (US)")
    .bind("English")
    .bind("United States")
    .bind("MMM D, YYYY")
    .bind("Auto")
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO billing_profiles (
            user_id, plan_name, monthly_price, next_renewal_date, payment_brand, payment_last4,
            billing_city, billing_region, billing_country, invoices_count, screens, features_json,
            average_revenue_per_user
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("usr-1")
    .bind("LIFESTREAM Premium")
    .bind(19.99_f64)
    .bind("2026-09-03")
    .bind("Visa")
    .bind("4821")
    .bind("San Francisco")
    .bind("CA")
    .bind("USA")
    .bind(12_i64)
    .bind(4_i64)
    .bind(json(&vec![
        "4K HDR",
        "Dolby Atmos",
        "4 screens",
        "no ads on live",
        "priority chat",
    ])?)
    .bind(7.40_f64)
    .execute(pool)
    .await?;

    for row in [
        ("acct-gh", "GitHub", "deepsaint", "2025-03-18T10:22:00Z"),
        (
            "acct-discord",
            "Discord",
            "deepsaint#0421",
            "2025-06-02T08:10:00Z",
        ),
    ] {
        sqlx::query(
            "INSERT OR IGNORE INTO connected_accounts (id, user_id, provider, display_name, connected_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(row.0)
        .bind("usr-1")
        .bind(row.1)
        .bind(row.2)
        .bind(row.3)
        .execute(pool)
        .await?;
    }

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO creator_live_settings (
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
            INSERT OR IGNORE INTO creator_stream_health_samples (
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
            "INSERT OR IGNORE INTO creator_subscriber_tiers (id, creator_id, tier_name, rank, monthly_price, subscriber_count, accent_color) VALUES (?, ?, ?, ?, ?, ?, ?)",
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

    for (tier_id, rank) in [("tier-1", 1_i64), ("tier-2", 2_i64), ("tier-3", 3_i64)] {
        sqlx::query("UPDATE creator_subscriber_tiers SET rank = ? WHERE id = ?")
            .bind(rank)
            .bind(tier_id)
            .execute(pool)
            .await?;
    }

    sqlx::query(
        "INSERT OR IGNORE INTO live_stream_notification_preferences (user_id, streamer_id, enabled, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind("usr-1")
    .bind("str-deepsaint")
    .bind(1_i64)
    .bind("2026-08-13T19:04:00Z")
    .execute(pool)
    .await?;

    for row in [
        (
            "series-northlight",
            "northlight-studio",
            "Northlight",
            "Prestige sci-fi episodic original run from the deepsaint studio slate.",
            "TV-MA",
            vec!["Drama", "Thriller", "Sci-Fi"],
            "#4ea1ff",
            asset("poster", "northlight"),
            asset("backdrop", "northlight"),
            "ongoing",
            "2026-07-01T12:00:00Z",
            "2026-08-13T12:00:00Z",
        ),
        (
            "series-halcyon",
            "halcyon-drift-studio",
            "Halcyon Drift",
            "Serialized spacefaring drama produced and published as creator-owned episodic content.",
            "TV-MA",
            vec!["Sci-Fi", "Action"],
            "#3dffd8",
            asset("poster", "halcyon"),
            asset("backdrop", "halcyon"),
            "ongoing",
            "2026-07-04T12:00:00Z",
            "2026-08-14T10:00:00Z",
        ),
    ] {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO creator_series_projects (
                id, creator_id, slug, title, synopsis, rating, genres_json, hero_color,
                poster_url, backdrop_url, status, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(row.0)
        .bind(creator_id)
        .bind(row.1)
        .bind(row.2)
        .bind(row.3)
        .bind(row.4)
        .bind(json(&row.5)?)
        .bind(row.6)
        .bind(row.7)
        .bind(row.8)
        .bind(row.9)
        .bind(row.10)
        .bind(row.11)
        .execute(pool)
        .await?;
    }

    sqlx::query("UPDATE uploads SET series_id = ? WHERE id = ?")
        .bind("series-northlight")
        .bind("up-northlight-s2e4")
        .execute(pool)
        .await?;
    sqlx::query("UPDATE uploads SET series_id = ? WHERE id IN (?, ?)")
        .bind("series-halcyon")
        .bind("up-halcyon-s1e2")
        .bind("up-draft-ep")
        .execute(pool)
        .await?;

    for row in [
        (
            "season-series-northlight-2",
            "series-northlight",
            2_i64,
            "Season 2",
            "Northlight enters its second season with the team fractured and the signal no longer dormant.",
        ),
        (
            "season-series-halcyon-1",
            "series-halcyon",
            1_i64,
            "Season 1",
            "The Halcyon crew takes a salvage job that turns into a conspiracy threaded through the whole solar economy.",
        ),
    ] {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO creator_series_seasons (
                id, series_id, season_number, title, synopsis, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(row.0)
        .bind(row.1)
        .bind(row.2)
        .bind(row.3)
        .bind(row.4)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(pool)
        .await?;
    }

    for row in [
        (
            "up-northlight-s2e4",
            "northlight-s2e4",
            "2026-08-13T18:00:00Z",
        ),
        (
            "up-halcyon-s1e2",
            "halcyon-drift-s1e2-cold-start",
            "2026-08-14T16:00:00Z",
        ),
        (
            "up-vod-queue",
            "distributed-job-queue-rust-full-stream",
            "2026-08-05T20:00:00Z",
        ),
        (
            "up-festival-cut",
            "festival-cut-afterlight",
            "2026-08-10T18:00:00Z",
        ),
        (
            "up-draft-ep",
            "halcyon-drift-s1e3-draft",
            "2026-08-16T18:00:00Z",
        ),
    ] {
        sqlx::query("UPDATE uploads SET slug = COALESCE(slug, ?), release_at = COALESCE(release_at, published_at, ?) WHERE id = ?")
            .bind(row.1)
            .bind(row.2)
            .bind(row.0)
            .execute(pool)
            .await?;
    }

    for row in [
        (
            "job-halcyon-e2",
            Some("up-halcyon-s1e2"),
            Some("series-halcyon"),
            "episode",
            "resumable-upload",
            "processing",
            "Halcyon Drift — S1 · E2 · Cold Start",
            "public",
            4_120_000_000_i64,
            2_554_400_000_i64,
            "uploads/creator/deepsaint/halcyon-s1e2/master.mov",
            "2026-08-14T14:20:00Z",
            "2026-08-14T20:45:00Z",
            None::<&str>,
        ),
        (
            "job-film-submission",
            None,
            None,
            "film",
            "resumable-upload",
            "created",
            "Feature cut intake — untitled festival export",
            "private",
            12_400_000_000_i64,
            0_i64,
            "uploads/creator/deepsaint/features/untitled-festival-export.mov",
            "2026-08-14T20:30:00Z",
            "2026-08-14T20:30:00Z",
            None::<&str>,
        ),
    ] {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO upload_jobs (
                id, creator_id, upload_id, series_id, kind, source_type, status, title,
                intended_visibility, bytes_expected, bytes_received, storage_key,
                created_at, updated_at, published_content_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(row.0)
        .bind(creator_id)
        .bind(row.1)
        .bind(row.2)
        .bind(row.3)
        .bind(row.4)
        .bind(row.5)
        .bind(row.6)
        .bind(row.7)
        .bind(row.8)
        .bind(row.9)
        .bind(row.10)
        .bind(row.11)
        .bind(row.12)
        .bind(row.13)
        .execute(pool)
        .await?;
    }

    let upload_rows = sqlx::query(
        r#"
        SELECT id, title, published_at, season_number, series_id
        FROM uploads
        WHERE creator_id = ?
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    for row in upload_rows {
        let upload_id: String = row.get("id");
        let title: String = row.get("title");
        let published_at: Option<String> = row.get("published_at");
        let slug = slugify(&title);
        sqlx::query(
            "UPDATE uploads SET slug = COALESCE(slug, ?), release_at = COALESCE(release_at, published_at, ?), access_policy = COALESCE(access_policy, 'free'), currency = COALESCE(currency, 'USD') WHERE id = ?",
        )
        .bind(slug)
        .bind(published_at.clone().unwrap_or_else(|| now.to_rfc3339()))
        .bind(&upload_id)
        .execute(pool)
        .await?;

        if let (Some(series_id), Some(season_number)) = (
            row.get::<Option<String>, _>("series_id"),
            row.get::<Option<i64>, _>("season_number"),
        ) {
            sqlx::query(
                r#"
                INSERT OR IGNORE INTO creator_series_seasons (
                    id, series_id, season_number, title, synopsis, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(format!("season-{series_id}-{season_number}"))
            .bind(&series_id)
            .bind(season_number)
            .bind(format!("Season {season_number}"))
            .bind("")
            .bind(now.to_rfc3339())
            .bind(now.to_rfc3339())
            .execute(pool)
            .await?;
        }
    }

    ensure_public_catalog_playback(pool).await?;
    ensure_live_stream_playback(pool).await?;

    Ok(())
}
