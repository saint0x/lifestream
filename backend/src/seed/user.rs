use sqlx::SqlitePool;

use super::support::{asset, json};

pub(super) async fn seed_users(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO users (id, handle, display_name, avatar, tier, joined_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind("usr-1")
    .bind("deepsaint")
    .bind("deepsaint")
    .bind(asset("avatar", "deepsaint"))
    .bind("premium")
    .bind("2024-11-03")
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT INTO users (id, handle, display_name, avatar, tier, joined_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind("usr-2")
    .bind("atlas_codes")
    .bind("atlas_codes")
    .bind(asset("avatar", "atlas"))
    .bind("standard")
    .bind("2025-02-17")
    .execute(pool)
    .await?;

    for content_id in ["ser-northlight", "flm-paper-moon", "ser-halcyon-drift"] {
        sqlx::query("INSERT INTO user_watchlist (user_id, content_id) VALUES (?, ?)")
            .bind("usr-1")
            .bind(content_id)
            .execute(pool)
            .await?;
    }

    for streamer_id in ["str-atlas", "str-kai", "str-mira"] {
        sqlx::query("INSERT INTO user_following (user_id, streamer_id) VALUES (?, ?)")
            .bind("usr-1")
            .bind(streamer_id)
            .execute(pool)
            .await?;
    }

    let continue_rows = vec![
        (
            "ser-northlight",
            "series",
            Some("ser-northlight-s2e3"),
            1280_i64,
            3120_i64,
            "2026-08-12T22:10:00Z",
        ),
        (
            "flm-afterglow",
            "film",
            None,
            4210_i64,
            7140_i64,
            "2026-08-11T20:40:00Z",
        ),
        (
            "ser-the-long-quiet",
            "series",
            Some("ser-the-long-quiet-s1e6"),
            400_i64,
            3000_i64,
            "2026-08-10T01:04:00Z",
        ),
        (
            "ser-halcyon-drift",
            "series",
            Some("ser-halcyon-drift-s1e2"),
            2100_i64,
            3000_i64,
            "2026-08-09T19:22:00Z",
        ),
    ];

    for row in continue_rows {
        sqlx::query(
            r#"
            INSERT INTO continue_watching (
                user_id, content_id, kind, episode_id, progress_sec, duration_sec, last_watched_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind("usr-1")
        .bind(row.0)
        .bind(row.1)
        .bind(row.2)
        .bind(row.3)
        .bind(row.4)
        .bind(row.5)
        .execute(pool)
        .await?;
    }

    sqlx::query(
        r#"
        INSERT INTO user_profiles (
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
        INSERT INTO user_playback_settings (
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
        INSERT INTO user_notification_settings (
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
        INSERT INTO user_privacy_settings (
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
        INSERT INTO user_parental_controls (
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
        INSERT INTO user_download_settings (
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
        INSERT INTO user_language_settings (
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
        INSERT INTO billing_profiles (
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
            "INSERT INTO connected_accounts (id, user_id, provider, display_name, connected_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(row.0)
        .bind("usr-1")
        .bind(row.1)
        .bind(row.2)
        .bind(row.3)
        .execute(pool)
        .await?;
    }

    Ok(())
}

pub(super) async fn seed_chat(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let rows = vec![
        (
            "m1",
            "lv-atlas-rust",
            "kestrel_22",
            "kestrel_22",
            "#4ea1ff",
            vec!["subscriber"],
            "chat the tokio docs for this are actually unreal",
            "2026-08-13T18:59:10Z",
        ),
        (
            "m2",
            "lv-atlas-rust",
            "orbit_moth",
            "orbit_moth",
            "#ff6a3d",
            Vec::<&str>::new(),
            "PLEASE explain the backpressure part again I got lost",
            "2026-08-13T18:59:22Z",
        ),
        (
            "m3",
            "lv-atlas-rust",
            "mod_sable",
            "sable",
            "#55d6a3",
            vec!["mod", "subscriber"],
            "reminder: no spoilers for today's episode in chat",
            "2026-08-13T18:59:35Z",
        ),
        (
            "m4",
            "lv-deepsaint-live",
            "silvershade",
            "silvershade",
            "#4ea1ff",
            vec!["staff"],
            "following from the dashboard",
            "2026-08-13T19:01:48Z",
        ),
    ];

    for row in rows {
        sqlx::query(
            "INSERT INTO chat_messages (id, stream_id, user_handle, display_name, color, badges_json, body, sent_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(row.0)
        .bind(row.1)
        .bind(row.2)
        .bind(row.3)
        .bind(row.4)
        .bind(json(&row.5)?)
        .bind(row.6)
        .bind(row.7)
        .execute(pool)
        .await?;
    }

    Ok(())
}
