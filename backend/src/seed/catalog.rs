use chrono::{Duration, Utc};
use sqlx::SqlitePool;

use super::support::{asset, credit, credits, episode_title, images, json};

pub(super) async fn seed_series(pool: &SqlitePool) -> Result<(), sqlx::Error> {
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

pub(super) async fn seed_films(pool: &SqlitePool) -> Result<(), sqlx::Error> {
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

pub(super) async fn seed_streamers(pool: &SqlitePool) -> Result<(), sqlx::Error> {
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

pub(super) async fn seed_live_streams(pool: &SqlitePool) -> Result<(), sqlx::Error> {
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

pub(super) async fn seed_categories(pool: &SqlitePool) -> Result<(), sqlx::Error> {
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
