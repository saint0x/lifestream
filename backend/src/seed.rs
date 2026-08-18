use chrono::{Duration, NaiveDate, Utc};
use sqlx::{Row, SqlitePool};

use crate::{config::Config, models::ImageSet};

mod local;
mod support;

use local::seed_local_auth_session;
use support::{asset, credit, credits, episode_title, images, json, slugify};

pub async fn seed_if_empty(pool: &SqlitePool, config: &Config) -> Result<(), sqlx::Error> {
    let count: i64 = sqlx::query("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?
        .get(0);

    if count != 0 {
        return Ok(());
    }

    seed_series(pool).await?;
    seed_films(pool).await?;
    seed_streamers(pool).await?;
    seed_live_streams(pool).await?;
    seed_categories(pool).await?;
    seed_users(pool).await?;
    seed_creator(pool).await?;
    seed_chat(pool).await?;
    ensure_extended_contract_data(pool).await?;
    seed_local_auth_session(pool, config).await?;

    Ok(())
}

async fn seed_series(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let series_rows = vec![
        (
            "ser-northlight",
            "northlight",
            "Northlight",
            Some("The dark has a memory."),
            "When a dormant signal reactivates beneath the polar ice, a fractured research team must decide what they owe the people who built it — and to each other.",
            2025_i64,
            "TV-MA",
            json(&vec!["Drama", "Thriller", "Sci-Fi"])?,
            images("northlight"),
            credits(&[
                credit("c-1", "Ava Reyes", "creator", None),
                credit("c-2", "Theo Kask", "cast", Some("Dr. Halden")),
                credit("c-3", "Mira Okafor", "cast", Some("Lt. Voss")),
            ])?,
            94_i64,
            1_i64,
            1_i64,
            "#4ea1ff",
            "ongoing",
            16_i64,
        ),
        (
            "ser-halcyon-drift",
            "halcyon-drift",
            "Halcyon Drift",
            Some("The map was always wrong."),
            "A salvage crew in a decaying solar economy takes one last contract aboard a ship that no one is supposed to know exists.",
            2025_i64,
            "TV-MA",
            json(&vec!["Sci-Fi", "Action"])?,
            images("halcyon"),
            credits(&[
                credit("c-4", "Omar Bellmar", "creator", None),
                credit("c-5", "Rhea Sol", "cast", Some("Captain Vale")),
            ])?,
            88_i64,
            1_i64,
            1_i64,
            "#3dffd8",
            "ongoing",
            10_i64,
        ),
        (
            "ser-the-long-quiet",
            "the-long-quiet",
            "The Long Quiet",
            Some("Not every war is loud."),
            "In a city where sound has become a weapon, a former translator for the silenced must relearn how to speak — and what her words are worth.",
            2024_i64,
            "TV-MA",
            json(&vec!["Drama", "Sci-Fi"])?,
            images("longquiet"),
            credits(&[
                credit("c-6", "Hanako Imai", "creator", None),
                credit("c-7", "Leo Sato", "cast", Some("Ren")),
            ])?,
            91_i64,
            1_i64,
            1_i64,
            "#9b6bff",
            "ended",
            10_i64,
        ),
    ];

    for row in series_rows {
        sqlx::query(
            r#"
            INSERT INTO series (
                id, slug, title, tagline, synopsis, year, rating, genres_json, images_json,
                credits_json, score, is_original, trending, hero_color, status, total_episodes
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(row.0)
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
        .bind(row.14)
        .bind(row.15)
        .execute(pool)
        .await?;
    }

    for (series_id, season_count, episodes_per_season) in [
        ("ser-northlight", 2_i64, 8_i64),
        ("ser-halcyon-drift", 1_i64, 10_i64),
        ("ser-the-long-quiet", 1_i64, 10_i64),
    ] {
        for season_number in 1..=season_count {
            sqlx::query("INSERT INTO seasons (series_id, season_number, title) VALUES (?, ?, ?)")
                .bind(series_id)
                .bind(season_number)
                .bind(format!("Season {}", season_number))
                .execute(pool)
                .await?;

            for episode_number in 1..=episodes_per_season {
                let id = format!("{series_id}-s{season_number}e{episode_number}");
                sqlx::query(
                    r#"
                    INSERT INTO episodes (
                        id, series_id, season_number, episode_number, title,
                        synopsis, duration_sec, aired_at, thumbnail
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind(id)
                .bind(series_id)
                .bind(season_number)
                .bind(episode_number)
                .bind(episode_title(episode_number))
                .bind("A pivotal turn forces the crew to confront a secret that has been hiding in plain sight since the day they arrived.")
                .bind(2700_i64 + (episode_number * 97))
                .bind(format!(
                    "2025-{:02}-{:02}",
                    season_number + 1,
                    ((episode_number * 3) % 27) + 1
                ))
                .bind(asset("thumb", &format!("{series_id}-{season_number}-{episode_number}")))
                .execute(pool)
                .await?;
            }
        }
    }

    Ok(())
}

async fn seed_films(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let rows = vec![
        (
            "flm-afterglow",
            "afterglow",
            "Afterglow",
            Some("What burns never really leaves."),
            "A widowed lighthouse keeper begins receiving radio transmissions from a station that was decommissioned thirty years ago.",
            2025_i64,
            "R",
            json(&vec!["Drama", "Thriller"])?,
            images("afterglow"),
            credits(&[
                credit("c-f1", "Noa Winters", "director", None),
                credit("c-f2", "Javier Ruiz", "cast", Some("Emil")),
            ])?,
            92_i64,
            1_i64,
            1_i64,
            "#ff6a3d",
            7140_i64,
        ),
        (
            "flm-paper-moon",
            "paper-moon",
            "Paper Moon, Iron Sky",
            None,
            "An astronaut returns from a twelve-year mission to find the only person who remembers her is her younger brother.",
            2025_i64,
            "TV-MA",
            json(&vec!["Sci-Fi", "Drama"])?,
            images("papermoon"),
            credits(&[credit("c-f7", "Ama Owusu", "director", None)])?,
            93_i64,
            1_i64,
            1_i64,
            "#8a3dff",
            7620_i64,
        ),
        (
            "flm-silver-house",
            "silver-house",
            "Silver House",
            None,
            "A tightly wound heist thriller set inside a private bank that exists only on the fifty-eighth floor after closing.",
            2024_i64,
            "R",
            json(&vec!["Thriller", "Crime"])?,
            images("silverhouse"),
            credits(&[credit("c-f3", "Celine Park", "director", None)])?,
            85_i64,
            1_i64,
            1_i64,
            "#d4d4d4",
            6900_i64,
        ),
    ];

    for row in rows {
        sqlx::query(
            r#"
            INSERT INTO films (
                id, slug, title, tagline, synopsis, year, rating, genres_json, images_json,
                credits_json, score, is_original, trending, hero_color, duration_sec
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(row.0)
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
        .bind(row.14)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn seed_streamers(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let rows = vec![
        (
            "str-atlas",
            "atlas_codes",
            "atlas_codes",
            "Systems programming, rust, and terminals with too many panes.",
            412_803_i64,
            1_i64,
            1_i64,
        ),
        (
            "str-noctis",
            "noctis",
            "NOCTIS",
            "Horror game archaeologist. I go where the floppies fear to load.",
            98_221_i64,
            1_i64,
            1_i64,
        ),
        (
            "str-paper",
            "paper.radio",
            "paper.radio",
            "Low bitrate, high intent. Late night music and interviews.",
            61_540_i64,
            0_i64,
            1_i64,
        ),
        (
            "str-kai",
            "kai.builds",
            "kai.builds",
            "Hardware hacker. Currently: a tiny CRT playing a tinier Doom.",
            172_009_i64,
            1_i64,
            1_i64,
        ),
        (
            "str-mira",
            "miraLIVE",
            "mira LIVE",
            "Talk show host. I interview interesting strangers at 2am.",
            233_100_i64,
            1_i64,
            1_i64,
        ),
        (
            "str-gridline",
            "gridline",
            "GRIDLINE",
            "Racing sims, endurance events, and caffeine.",
            329_901_i64,
            1_i64,
            1_i64,
        ),
        (
            "str-deepsaint",
            "deepsaint",
            "deepsaint",
            "Systems & cinematic storytelling · live + recorded",
            412_803_i64,
            1_i64,
            1_i64,
        ),
    ];

    for row in rows {
        sqlx::query(
            "INSERT INTO streamers (id, handle, display_name, avatar, bio, followers, is_partner, is_live) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(row.0)
        .bind(row.1)
        .bind(row.2)
        .bind(asset("avatar", row.1))
        .bind(row.3)
        .bind(row.4)
        .bind(row.5)
        .bind(row.6)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn seed_live_streams(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let now = Utc::now();
    let rows = vec![
        (
            "lv-atlas-rust",
            "atlas-rust-async",
            "writing a distributed job queue in rust from scratch",
            "Tech",
            vec!["rust", "tokio", "systems", "english"],
            "str-atlas",
            14_204_i64,
            now - Duration::minutes(142),
            "lv-atlas",
            "EN",
            0_i64,
        ),
        (
            "lv-noctis-resident",
            "noctis-silent-hill",
            "silent hill 2 — blind run, one life, bad idea",
            "Gaming",
            vec!["horror", "blind", "retro"],
            "str-noctis",
            8_891_i64,
            now - Duration::minutes(63),
            "lv-noctis",
            "EN",
            1_i64,
        ),
        (
            "lv-paper-radio",
            "paper-radio-night",
            "paper.radio — rainy tuesday / slow jazz",
            "Music",
            vec!["jazz", "ambient", "lofi"],
            "str-paper",
            3_109_i64,
            now - Duration::minutes(305),
            "lv-paper",
            "EN",
            0_i64,
        ),
        (
            "lv-mira-talk",
            "mira-late-night",
            "late night w/ mira — guest: a competitive sleeper",
            "Talk",
            vec!["interview", "conversation"],
            "str-mira",
            11_450_i64,
            now - Duration::minutes(27),
            "lv-mira",
            "EN",
            0_i64,
        ),
        (
            "lv-gridline-endurance",
            "gridline-endurance",
            "24h endurance race — hour 6 — le mans prototype",
            "Sports",
            vec!["racing", "sim", "endurance"],
            "str-gridline",
            22_108_i64,
            now - Duration::minutes(360),
            "lv-gridline",
            "EN",
            0_i64,
        ),
        (
            "lv-deepsaint-live",
            "deepsaint-live",
            "writing a distributed job queue in rust from scratch",
            "Tech",
            vec!["rust", "systems", "english"],
            "str-deepsaint",
            12_810_i64,
            now - Duration::minutes(138),
            "bcast-now",
            "EN",
            0_i64,
        ),
    ];

    for row in rows {
        sqlx::query(
            r#"
            INSERT INTO live_streams (
                id, slug, title, category, tags_json, streamer_id, viewers, started_at,
                thumbnail, language, is_mature
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(row.0)
        .bind(row.1)
        .bind(row.2)
        .bind(row.3)
        .bind(json(&row.4)?)
        .bind(row.5)
        .bind(row.6)
        .bind(row.7.to_rfc3339())
        .bind(asset("thumb", row.8))
        .bind(row.9)
        .bind(row.10)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn seed_categories(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let rows = vec![
        (
            "tech",
            "Tech",
            38_221_i64,
            540_i64,
            vec!["rust", "systems", "web", "linux"],
        ),
        (
            "gaming",
            "Gaming",
            212_080_i64,
            6_720_i64,
            vec!["speedrun", "horror", "indie", "ranked"],
        ),
        (
            "music",
            "Music",
            18_092_i64,
            201_i64,
            vec!["jazz", "ambient", "synth", "piano"],
        ),
        (
            "talk",
            "Talk",
            44_550_i64,
            120_i64,
            vec!["podcast", "interview", "late-night"],
        ),
        (
            "sports",
            "Sports",
            88_401_i64,
            230_i64,
            vec!["racing", "esports", "analysis"],
        ),
        (
            "drama",
            "Drama",
            12_450_i64,
            312_i64,
            vec!["prestige", "slow-burn", "character"],
        ),
    ];

    for row in rows {
        sqlx::query(
            "INSERT INTO categories (slug, name, cover_image, live_viewers, live_channels, tags_json) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(row.0)
        .bind(row.1)
        .bind(asset("square", row.0))
        .bind(row.2)
        .bind(row.3)
        .bind(json(&row.4)?)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn seed_users(pool: &SqlitePool) -> Result<(), sqlx::Error> {
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

async fn seed_creator(pool: &SqlitePool) -> Result<(), sqlx::Error> {
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

async fn seed_chat(pool: &SqlitePool) -> Result<(), sqlx::Error> {
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

async fn ensure_extended_contract_data(pool: &SqlitePool) -> Result<(), sqlx::Error> {
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

async fn ensure_public_catalog_playback(pool: &SqlitePool) -> Result<(), sqlx::Error> {
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
    sample: &sqlx::sqlite::SqliteRow,
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

async fn ensure_live_stream_playback(pool: &SqlitePool) -> Result<(), sqlx::Error> {
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
