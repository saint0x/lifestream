use super::*;
use crate::{db::Database, storage::Storage};

pub(crate) async fn setup_test_state() -> AppResult<(SharedState, CreatorProfile)> {
    let test_id = Uuid::new_v4().to_string();
    let db_path = std::env::temp_dir().join(format!("vanta-test-{test_id}.db"));
    let media_root = std::env::temp_dir().join(format!("vanta-media-{test_id}"));
    let source_db_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    copy_sqlite_fixture(source_db_dir.join("vanta.db"), &db_path).await?;
    let database_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;

    sqlx::raw_sql(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;
        PRAGMA trusted_schema = ON;
        PRAGMA busy_timeout = 5000;
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    tokio::fs::create_dir_all(&media_root)
        .await
        .map_err(AppError::Io)?;

    let config = Config {
        bind_addr: "127.0.0.1:0".parse().expect("test bind"),
        database_kind: DatabaseKind::Sqlite,
        database_url,
        max_db_connections: 1,
        storage_kind: StorageKind::Local,
        media_scratch_root: std::env::temp_dir().join(format!("vanta-scratch-{test_id}")),
        media_root: PathBuf::from(&media_root),
        object_storage_bucket: None,
        object_storage_endpoint_url: None,
        object_storage_access_key_id: None,
        object_storage_secret_access_key: None,
        object_storage_region: "auto".to_string(),
        object_storage_cdn_base_url: None,
        cdn_cookie_domain: None,
        admin_api_enabled: true,
        token_hash_secret: None,
        allowed_origins: vec!["http://localhost:3000".to_string()],
        environment: RuntimeEnvironment::Development,
    };
    let storage = Storage::from_config(&config)?;
    let state = Arc::new(AppState::new(
        Database::from_sqlite(pool.clone()),
        storage,
        config,
        vec![HeaderValue::from_static("http://localhost:3000")],
    ));
    let creator = fetch_creator_profile(&pool, "crt-deepsaint").await?;
    seed_playback_test_fixtures(&pool, &creator).await?;
    reset_creator_live_state(&pool, &creator).await?;
    Ok((state, creator))
}

async fn seed_playback_test_fixtures(pool: &SqlitePool, creator: &CreatorProfile) -> AppResult<()> {
    let now = Utc::now();
    let now_rfc3339 = now.to_rfc3339();
    let next_month = (now + chrono::Duration::days(30)).to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO users (id, handle, display_name, avatar, tier, joined_at)
        VALUES (?, 'viewer', 'Viewer QA', '', 'free', ?)
        ON CONFLICT(id) DO UPDATE SET
            handle = excluded.handle,
            display_name = excluded.display_name,
            avatar = excluded.avatar,
            tier = excluded.tier
        "#,
    )
    .bind("usr-viewer")
    .bind(&now_rfc3339)
    .execute(pool)
    .await?;
    for (user_id, handle, display_name) in [
        ("usr-1", "hostviewer", "Host Viewer QA"),
        ("usr-2", "atlas_codes", "Atlas Viewer QA"),
    ] {
        sqlx::query(
            r#"
            INSERT INTO users (id, handle, display_name, avatar, tier, joined_at)
            VALUES (?, ?, ?, '', 'free', ?)
            ON CONFLICT(id) DO UPDATE SET
                handle = excluded.handle,
                display_name = excluded.display_name,
                avatar = excluded.avatar,
                tier = excluded.tier
            "#,
        )
        .bind(user_id)
        .bind(handle)
        .bind(display_name)
        .bind(&now_rfc3339)
        .execute(pool)
        .await?;
    }
    seed_creator_identity_fixture(
        pool,
        "crt-atlas",
        "usr-2",
        "atlas",
        "Atlas",
        "sk_atlas",
        &now_rfc3339,
    )
    .await?;
    seed_public_catalog_test_fixtures(pool).await?;

    sqlx::query(
        r#"
        INSERT INTO creator_subscriber_tiers (
            id, creator_id, tier_name, monthly_price, subscriber_count,
            accent_color, rank, status, retired_at
        ) VALUES (?, ?, 'Studio Pass', 9.99, 0, '#c7f5ff', 1, 'active', NULL)
        ON CONFLICT(id) DO UPDATE SET
            creator_id = excluded.creator_id,
            tier_name = excluded.tier_name,
            monthly_price = excluded.monthly_price,
            rank = excluded.rank,
            status = excluded.status,
            retired_at = NULL
        "#,
    )
    .bind("tier-test-studio-pass")
    .bind(&creator.id)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO creator_operational_state (
            creator_id, legal_name, support_email, business_type, payout_country,
            payout_provider, onboarding_status, identity_status, tax_status,
            payout_status, hold_reasons_json, created_at, updated_at, last_reviewed_at
        ) VALUES (?, 'deepsaint QA LLC', 'support@streamvanta.test', 'company', 'US',
            'whop', 'approved', 'verified', 'verified', 'active', '[]', ?, ?, ?)
        ON CONFLICT(creator_id) DO UPDATE SET
            legal_name = excluded.legal_name,
            support_email = excluded.support_email,
            business_type = excluded.business_type,
            payout_country = excluded.payout_country,
            payout_provider = excluded.payout_provider,
            onboarding_status = excluded.onboarding_status,
            identity_status = excluded.identity_status,
            tax_status = excluded.tax_status,
            payout_status = excluded.payout_status,
            hold_reasons_json = excluded.hold_reasons_json,
            updated_at = excluded.updated_at,
            last_reviewed_at = excluded.last_reviewed_at
        "#,
    )
    .bind(&creator.id)
    .bind(&now_rfc3339)
    .bind(&now_rfc3339)
    .bind(&now_rfc3339)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO streamers (
            id, handle, display_name, avatar, bio, followers, is_partner, is_live
        ) VALUES (?, ?, ?, ?, ?, ?, 1, 0)
        ON CONFLICT(handle) DO UPDATE SET
            display_name = excluded.display_name,
            avatar = excluded.avatar,
            bio = excluded.bio,
            followers = excluded.followers,
            is_partner = excluded.is_partner,
            is_live = excluded.is_live
        "#,
    )
    .bind(format!("str-{}", creator.handle))
    .bind(&creator.handle)
    .bind(&creator.display_name)
    .bind(&creator.avatar)
    .bind(&creator.bio)
    .bind(creator.followers)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO creator_live_settings (
            creator_id, subscriber_only, slow_mode_seconds, auto_mod_level,
            notify_followers_default, active_scene_id, scenes_json, bitrate_kbps,
            cpu_percent, dropped_frames, free_disk_gb, delivery_class
        ) VALUES (?, 0, 0, 'standard', 1, 'scene-main', '[]', 0, 0, 0, 100.0, 'standard_hls')
        ON CONFLICT(creator_id) DO UPDATE SET
            subscriber_only = excluded.subscriber_only,
            slow_mode_seconds = excluded.slow_mode_seconds,
            auto_mod_level = excluded.auto_mod_level,
            notify_followers_default = excluded.notify_followers_default,
            active_scene_id = excluded.active_scene_id,
            scenes_json = excluded.scenes_json,
            bitrate_kbps = excluded.bitrate_kbps,
            cpu_percent = excluded.cpu_percent,
            dropped_frames = excluded.dropped_frames,
            free_disk_gb = excluded.free_disk_gb,
            delivery_class = excluded.delivery_class
        "#,
    )
    .bind(&creator.id)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO creator_memberships (
            user_id, creator_id, tier_id, status, started_at, renews_at, ends_at, canceled_at
        ) VALUES (?, ?, 'tier-test-studio-pass', 'active', ?, ?, NULL, NULL)
        ON CONFLICT(user_id, creator_id) DO UPDATE SET
            tier_id = excluded.tier_id,
            status = excluded.status,
            started_at = excluded.started_at,
            renews_at = excluded.renews_at,
            ends_at = NULL,
            canceled_at = NULL
        "#,
    )
    .bind("usr-viewer")
    .bind(&creator.id)
    .bind(&now_rfc3339)
    .bind(&next_month)
    .execute(pool)
    .await?;

    let fixtures = [
        TestPlaybackFixture {
            upload_id: "flm-afterglow",
            job_id: "upj-test-afterglow",
            asset_id: "ast-test-afterglow",
            title: "Afterglow",
            description: "A fixture film with protected playback media.",
            slug: "afterglow",
            access_policy: "free",
            access_tier_id: None,
            price_cents: None,
            currency: None,
            rental_window_hours: None,
            status: "published",
            visibility: "public",
            job_status: "published",
            asset_status: "published",
        },
        TestPlaybackFixture {
            upload_id: "upl-57fd50bbb54a44f58fe10605f97eeead",
            job_id: "upj-test-purchase",
            asset_id: "ast-test-purchase",
            title: "Purchase Gate Fixture",
            description: "A fixture film for purchase entitlement playback.",
            slug: "purchase-gate-fixture",
            access_policy: "purchase",
            access_tier_id: None,
            price_cents: Some(1499),
            currency: Some("USD"),
            rental_window_hours: Some(24),
            status: "published",
            visibility: "public",
            job_status: "published",
            asset_status: "published",
        },
        TestPlaybackFixture {
            upload_id: "upl-6f378951e0ee4526b13333f470db77e3",
            job_id: "upj-test-subscription-or-purchase",
            asset_id: "ast-test-subscription-or-purchase",
            title: "Subscriber Gate Fixture",
            description: "A fixture film for subscription entitlement playback.",
            slug: "subscriber-gate-fixture",
            access_policy: "subscription_or_purchase",
            access_tier_id: Some("tier-test-studio-pass"),
            price_cents: Some(1599),
            currency: Some("USD"),
            rental_window_hours: Some(48),
            status: "published",
            visibility: "public",
            job_status: "published",
            asset_status: "published",
        },
        TestPlaybackFixture {
            upload_id: "upl-test-ready-publish",
            job_id: "upj-test-ready-publish",
            asset_id: "ast-test-ready-publish",
            title: "Ready Publish Fixture",
            description: "A ready fixture for publish and media job lifecycle tests.",
            slug: "ready-publish-fixture",
            access_policy: "free",
            access_tier_id: None,
            price_cents: None,
            currency: None,
            rental_window_hours: None,
            status: "draft",
            visibility: "private",
            job_status: "ready",
            asset_status: "ready",
        },
    ];

    for fixture in fixtures {
        seed_playback_fixture(pool, creator, &fixture, &now_rfc3339).await?;
    }

    Ok(())
}

async fn seed_public_catalog_test_fixtures(pool: &SqlitePool) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO categories (slug, name, cover_image, live_viewers, live_channels, tags_json)
        VALUES (
            'gaming', 'Gaming',
            'https://images.unsplash.com/photo-1511512578047-dfb367046420?auto=format&fit=crop&w=900&q=80',
            0, 0, '["esports","streaming"]'
        )
        ON CONFLICT(slug) DO UPDATE SET
            name = excluded.name,
            cover_image = excluded.cover_image,
            tags_json = excluded.tags_json
        "#,
    )
    .execute(pool)
    .await?;

    for (id, slug, title, score) in [
        (
            "ser-test-sci-fi-one",
            "test-sci-fi-one",
            "Test Sci-Fi One",
            91_i64,
        ),
        (
            "ser-test-sci-fi-two",
            "test-sci-fi-two",
            "Test Sci-Fi Two",
            89_i64,
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO series (
                id, slug, title, tagline, synopsis, year, rating, genres_json,
                images_json, credits_json, score, is_original, trending, hero_color,
                status, total_episodes
            ) VALUES (?, ?, ?, 'Fixture signal', 'A deterministic public catalog fixture.',
                2026, 'TV-14', '["Sci-Fi","Drama"]',
                '{"poster":"https://images.unsplash.com/photo-1446776811953-b23d57bd21aa?auto=format&fit=crop&w=900&q=80","backdrop":"https://images.unsplash.com/photo-1446776811953-b23d57bd21aa?auto=format&fit=crop&w=1600&q=80","thumbnail":"https://images.unsplash.com/photo-1446776811953-b23d57bd21aa?auto=format&fit=crop&w=900&q=80","logo":null}',
                '[]', ?, 1, 0, '#c7f5ff', 'ongoing', 0)
            ON CONFLICT(id) DO UPDATE SET
                slug = excluded.slug,
                title = excluded.title,
                genres_json = excluded.genres_json,
                images_json = excluded.images_json,
                credits_json = excluded.credits_json,
                score = excluded.score,
                is_original = excluded.is_original,
                trending = excluded.trending,
                status = excluded.status,
                total_episodes = excluded.total_episodes
            "#,
        )
        .bind(id)
        .bind(slug)
        .bind(title)
        .bind(score)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn seed_creator_identity_fixture(
    pool: &SqlitePool,
    creator_id: &str,
    user_id: &str,
    handle: &str,
    display_name: &str,
    stream_key: &str,
    now: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO creator_profiles (
            id, user_id, handle, display_name, avatar, banner, tagline, bio,
            partner_status, joined_at, stream_key, rtmp_url, default_category,
            default_tags_json, followers, subscribers, monthly_viewers,
            total_watch_hours, live_status, current_broadcast_id
        ) VALUES (?, ?, ?, ?, ?, ?, 'Fixture collaborator', 'Collaboration test creator',
            'affiliate', ?, ?, 'rtmp://ingest.vanta.local/live', 'Gaming',
            '["co-stream"]', 0, 0, 0, 0, 'offline', NULL)
        ON CONFLICT(id) DO UPDATE SET
            user_id = excluded.user_id,
            handle = excluded.handle,
            display_name = excluded.display_name,
            avatar = excluded.avatar,
            banner = excluded.banner,
            tagline = excluded.tagline,
            bio = excluded.bio,
            partner_status = excluded.partner_status,
            stream_key = excluded.stream_key,
            rtmp_url = excluded.rtmp_url,
            default_category = excluded.default_category,
            default_tags_json = excluded.default_tags_json
        "#,
    )
    .bind(creator_id)
    .bind(user_id)
    .bind(handle)
    .bind(display_name)
    .bind(format!("https://cdn.vanta.local/avatar/{handle}.jpg"))
    .bind(format!("https://cdn.vanta.local/banner/{handle}.jpg"))
    .bind(now)
    .bind(stream_key)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO streamers (
            id, handle, display_name, avatar, bio, followers, is_partner, is_live
        ) VALUES (?, ?, ?, ?, 'Collaboration test creator', 0, 1, 0)
        ON CONFLICT(handle) DO UPDATE SET
            display_name = excluded.display_name,
            avatar = excluded.avatar,
            bio = excluded.bio,
            followers = excluded.followers,
            is_partner = excluded.is_partner,
            is_live = excluded.is_live
        "#,
    )
    .bind(format!("str-{handle}"))
    .bind(handle)
    .bind(display_name)
    .bind(format!("https://cdn.vanta.local/avatar/{handle}.jpg"))
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO creator_operational_state (
            creator_id, legal_name, support_email, business_type, payout_country,
            payout_provider, onboarding_status, identity_status, tax_status,
            payout_status, hold_reasons_json, created_at, updated_at, last_reviewed_at
        ) VALUES (?, ?, ?, 'company', 'US', 'whop', 'approved', 'verified',
            'verified', 'active', '[]', ?, ?, ?)
        ON CONFLICT(creator_id) DO UPDATE SET
            legal_name = excluded.legal_name,
            support_email = excluded.support_email,
            business_type = excluded.business_type,
            payout_country = excluded.payout_country,
            payout_provider = excluded.payout_provider,
            onboarding_status = excluded.onboarding_status,
            identity_status = excluded.identity_status,
            tax_status = excluded.tax_status,
            payout_status = excluded.payout_status,
            hold_reasons_json = excluded.hold_reasons_json,
            updated_at = excluded.updated_at,
            last_reviewed_at = excluded.last_reviewed_at
        "#,
    )
    .bind(creator_id)
    .bind(format!("{display_name} QA LLC"))
    .bind(format!("support+{handle}@streamvanta.test"))
    .bind(now)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO creator_live_settings (
            creator_id, subscriber_only, slow_mode_seconds, auto_mod_level,
            notify_followers_default, active_scene_id, scenes_json, bitrate_kbps,
            cpu_percent, dropped_frames, free_disk_gb, delivery_class
        ) VALUES (?, 0, 0, 'standard', 1, 'scene-main', '[]', 0, 0, 0, 100.0, 'standard_hls')
        ON CONFLICT(creator_id) DO UPDATE SET
            subscriber_only = excluded.subscriber_only,
            slow_mode_seconds = excluded.slow_mode_seconds,
            auto_mod_level = excluded.auto_mod_level,
            notify_followers_default = excluded.notify_followers_default,
            active_scene_id = excluded.active_scene_id,
            scenes_json = excluded.scenes_json,
            bitrate_kbps = excluded.bitrate_kbps,
            cpu_percent = excluded.cpu_percent,
            dropped_frames = excluded.dropped_frames,
            free_disk_gb = excluded.free_disk_gb,
            delivery_class = excluded.delivery_class
        "#,
    )
    .bind(creator_id)
    .execute(pool)
    .await?;

    Ok(())
}

struct TestPlaybackFixture {
    upload_id: &'static str,
    job_id: &'static str,
    asset_id: &'static str,
    title: &'static str,
    description: &'static str,
    slug: &'static str,
    access_policy: &'static str,
    access_tier_id: Option<&'static str>,
    price_cents: Option<i64>,
    currency: Option<&'static str>,
    rental_window_hours: Option<i64>,
    status: &'static str,
    visibility: &'static str,
    job_status: &'static str,
    asset_status: &'static str,
}

async fn seed_playback_fixture(
    pool: &SqlitePool,
    creator: &CreatorProfile,
    fixture: &TestPlaybackFixture,
    now: &str,
) -> AppResult<()> {
    let source_path = format!("test-sources/{}/source.mp4", fixture.upload_id);
    let poster_path = format!("test-playback/{}/poster.jpg", fixture.upload_id);
    let playback_path = format!("test-playback/{}/master.m3u8", fixture.upload_id);
    let thumbnail_path = format!("test-playback/{}/thumb.jpg", fixture.upload_id);

    sqlx::query(
        r#"
        INSERT INTO uploads (
            id, creator_id, slug, title, description, kind, duration_sec,
            uploaded_at, published_at, release_at, status, visibility, access_policy,
            access_tier_id, price_cents, currency, rental_window_hours, views, likes,
            comments, watch_hours, thumbnail, series_title, season_number,
            episode_number, size_bytes, resolution, transcode_progress, series_id
        ) VALUES (?, ?, ?, ?, ?, 'film', 6420, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 0, 0, 0, ?, NULL, NULL, NULL, 4566887, '1280x720', 1.0, NULL)
        ON CONFLICT(id) DO UPDATE SET
            creator_id = excluded.creator_id,
            slug = excluded.slug,
            title = excluded.title,
            description = excluded.description,
            kind = excluded.kind,
            duration_sec = excluded.duration_sec,
            published_at = excluded.published_at,
            release_at = excluded.release_at,
            status = excluded.status,
            visibility = excluded.visibility,
            access_policy = excluded.access_policy,
            access_tier_id = excluded.access_tier_id,
            price_cents = excluded.price_cents,
            currency = excluded.currency,
            rental_window_hours = excluded.rental_window_hours,
            thumbnail = excluded.thumbnail,
            size_bytes = excluded.size_bytes,
            resolution = excluded.resolution,
            transcode_progress = excluded.transcode_progress
        "#,
    )
    .bind(fixture.upload_id)
    .bind(&creator.id)
    .bind(fixture.slug)
    .bind(fixture.title)
    .bind(fixture.description)
    .bind(now)
    .bind((fixture.status == "published").then_some(now))
    .bind(now)
    .bind(fixture.status)
    .bind(fixture.visibility)
    .bind(fixture.access_policy)
    .bind(fixture.access_tier_id)
    .bind(fixture.price_cents)
    .bind(fixture.currency)
    .bind(fixture.rental_window_hours)
    .bind(format!("/api/v1/media/{thumbnail_path}"))
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO upload_jobs (
            id, creator_id, upload_id, series_id, kind, source_type, status, title,
            intended_visibility, bytes_expected, bytes_received, storage_key,
            created_at, updated_at, published_content_id, mime_type, checksum_sha256,
            completed_at, processing_attempt_count, last_processing_error, last_failed_at
        ) VALUES (?, ?, ?, NULL, 'film', 'fixture', ?, ?, ?, 4566887, 4566887, ?, ?, ?, ?, 'video/mp4', NULL, ?, 0, NULL, NULL)
        ON CONFLICT(id) DO UPDATE SET
            creator_id = excluded.creator_id,
            upload_id = excluded.upload_id,
            kind = excluded.kind,
            source_type = excluded.source_type,
            status = excluded.status,
            title = excluded.title,
            intended_visibility = excluded.intended_visibility,
            bytes_expected = excluded.bytes_expected,
            bytes_received = excluded.bytes_received,
            storage_key = excluded.storage_key,
            updated_at = excluded.updated_at,
            published_content_id = excluded.published_content_id,
            mime_type = excluded.mime_type,
            completed_at = excluded.completed_at,
            processing_attempt_count = excluded.processing_attempt_count,
            last_processing_error = NULL,
            last_failed_at = NULL
        "#,
    )
    .bind(fixture.job_id)
    .bind(&creator.id)
    .bind(fixture.upload_id)
    .bind(fixture.job_status)
    .bind(fixture.title)
    .bind(fixture.visibility)
    .bind(&source_path)
    .bind(now)
    .bind(now)
    .bind(fixture.upload_id)
    .bind(now)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO upload_job_ingest_sessions (
            job_id, creator_id, relative_path, upload_token_hash, status, mime_type,
            bytes_received, created_at, updated_at, completed_at
        ) VALUES (?, ?, ?, ?, 'completed', 'video/mp4', 4566887, ?, ?, ?)
        ON CONFLICT(job_id) DO UPDATE SET
            creator_id = excluded.creator_id,
            relative_path = excluded.relative_path,
            upload_token_hash = excluded.upload_token_hash,
            status = excluded.status,
            mime_type = excluded.mime_type,
            bytes_received = excluded.bytes_received,
            updated_at = excluded.updated_at,
            completed_at = excluded.completed_at
        "#,
    )
    .bind(fixture.job_id)
    .bind(&creator.id)
    .bind(&source_path)
    .bind(hash_token(&format!(
        "fixture-upload-token-{}",
        fixture.job_id
    )))
    .bind(now)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO media_assets (
            id, creator_id, upload_job_id, upload_id, series_id, kind, title, status,
            visibility, source_relative_path, poster_relative_path, playback_relative_path,
            mime_type, checksum_sha256, container_format, file_size_bytes, duration_sec,
            width, height, frame_rate, video_codec, audio_codec, has_video, has_audio,
            created_at, updated_at, processed_at, published_content_id
        ) VALUES (?, ?, ?, ?, NULL, 'film', ?, ?, ?, ?, ?, ?, 'application/vnd.apple.mpegurl',
            NULL, 'hls', 4566887, 6420.0, 1280, 720, 30.0, 'h264', 'aac', 1, 1, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            creator_id = excluded.creator_id,
            upload_job_id = excluded.upload_job_id,
            upload_id = excluded.upload_id,
            kind = excluded.kind,
            title = excluded.title,
            status = excluded.status,
            visibility = excluded.visibility,
            source_relative_path = excluded.source_relative_path,
            poster_relative_path = excluded.poster_relative_path,
            playback_relative_path = excluded.playback_relative_path,
            mime_type = excluded.mime_type,
            container_format = excluded.container_format,
            file_size_bytes = excluded.file_size_bytes,
            duration_sec = excluded.duration_sec,
            width = excluded.width,
            height = excluded.height,
            frame_rate = excluded.frame_rate,
            video_codec = excluded.video_codec,
            audio_codec = excluded.audio_codec,
            has_video = excluded.has_video,
            has_audio = excluded.has_audio,
            updated_at = excluded.updated_at,
            processed_at = excluded.processed_at,
            published_content_id = excluded.published_content_id
        "#,
    )
    .bind(fixture.asset_id)
    .bind(&creator.id)
    .bind(fixture.job_id)
    .bind(fixture.upload_id)
    .bind(fixture.title)
    .bind(fixture.asset_status)
    .bind(fixture.visibility)
    .bind(&source_path)
    .bind(&poster_path)
    .bind(&playback_path)
    .bind(now)
    .bind(now)
    .bind(now)
    .bind(fixture.upload_id)
    .execute(pool)
    .await?;

    for (variant_type, label, relative_path, mime_type, width, height, bitrate_bps, is_default) in [
        (
            "hls",
            "720p",
            playback_path.as_str(),
            "application/vnd.apple.mpegurl",
            Some(1280_i64),
            Some(720_i64),
            Some(3_000_000_i64),
            1_i64,
        ),
        (
            "thumbnail",
            "card_thumbnail",
            thumbnail_path.as_str(),
            "image/jpeg",
            Some(900_i64),
            Some(506_i64),
            None,
            1_i64,
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO media_asset_variants (
                id, asset_id, variant_type, label, relative_path, mime_type, width,
                height, bitrate_bps, file_size_bytes, is_default, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 2048, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                asset_id = excluded.asset_id,
                variant_type = excluded.variant_type,
                label = excluded.label,
                relative_path = excluded.relative_path,
                mime_type = excluded.mime_type,
                width = excluded.width,
                height = excluded.height,
                bitrate_bps = excluded.bitrate_bps,
                file_size_bytes = excluded.file_size_bytes,
                is_default = excluded.is_default
            "#,
        )
        .bind(format!("var-{}-{variant_type}-{label}", fixture.asset_id))
        .bind(fixture.asset_id)
        .bind(variant_type)
        .bind(label)
        .bind(relative_path)
        .bind(mime_type)
        .bind(width)
        .bind(height)
        .bind(bitrate_bps)
        .bind(is_default)
        .bind(now)
        .execute(pool)
        .await?;
    }

    Ok(())
}
