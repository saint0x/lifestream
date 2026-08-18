CREATE TABLE IF NOT EXISTS user_profiles (
    user_id TEXT PRIMARY KEY,
    email TEXT NOT NULL,
    email_verified INTEGER NOT NULL,
    mature_content_allowed INTEGER NOT NULL,
    default_audio TEXT NOT NULL,
    subtitle_preset TEXT NOT NULL,
    autoplay_trailers INTEGER NOT NULL,
    live_chat_filter TEXT NOT NULL,
    hours_watched INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS user_playback_settings (
    user_id TEXT PRIMARY KEY,
    default_quality TEXT NOT NULL,
    audio_language TEXT NOT NULL,
    subtitle_language TEXT NOT NULL,
    subtitle_style TEXT NOT NULL,
    autoplay_next_episode INTEGER NOT NULL,
    autoplay_trailers INTEGER NOT NULL,
    reduced_motion INTEGER NOT NULL,
    prefer_dubbed INTEGER NOT NULL,
    playback_speed TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS user_notification_settings (
    user_id TEXT PRIMARY KEY,
    series_push INTEGER NOT NULL,
    series_email INTEGER NOT NULL,
    live_push INTEGER NOT NULL,
    live_email INTEGER NOT NULL,
    originals_push INTEGER NOT NULL,
    originals_email INTEGER NOT NULL,
    watchlist_push INTEGER NOT NULL,
    watchlist_email INTEGER NOT NULL,
    creator_push INTEGER NOT NULL,
    creator_email INTEGER NOT NULL,
    security_push INTEGER NOT NULL,
    security_email INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS user_privacy_settings (
    user_id TEXT PRIMARY KEY,
    show_friend_activity INTEGER NOT NULL,
    improve_recommendations INTEGER NOT NULL,
    personalized_ads INTEGER NOT NULL,
    ab_tests INTEGER NOT NULL,
    data_export_size_mb REAL NOT NULL,
    delete_cooldown_days INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS user_parental_controls (
    user_id TEXT PRIMARY KEY,
    max_rating TEXT NOT NULL,
    require_pin_for_mature INTEGER NOT NULL,
    hide_live_chat_for_kids INTEGER NOT NULL,
    block_mature_live_streams INTEGER NOT NULL,
    pin_set INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS user_download_settings (
    user_id TEXT PRIMARY KEY,
    video_quality TEXT NOT NULL,
    wifi_only INTEGER NOT NULL,
    smart_downloads INTEGER NOT NULL,
    storage_used_gb REAL NOT NULL,
    storage_limit_gb REAL NOT NULL,
    device_limit INTEGER NOT NULL,
    active_devices INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS user_language_settings (
    user_id TEXT PRIMARY KEY,
    interface_language TEXT NOT NULL,
    subtitle_language TEXT NOT NULL,
    catalog_region TEXT NOT NULL,
    date_format TEXT NOT NULL,
    clock_format TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS billing_profiles (
    user_id TEXT PRIMARY KEY,
    plan_name TEXT NOT NULL,
    monthly_price REAL NOT NULL,
    next_renewal_date TEXT NOT NULL,
    payment_brand TEXT NOT NULL,
    payment_last4 TEXT NOT NULL,
    billing_city TEXT NOT NULL,
    billing_region TEXT NOT NULL,
    billing_country TEXT NOT NULL,
    invoices_count INTEGER NOT NULL,
    screens INTEGER NOT NULL,
    features_json TEXT NOT NULL,
    average_revenue_per_user REAL NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS connected_accounts (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    display_name TEXT NOT NULL,
    connected_at TEXT NOT NULL,
    UNIQUE (user_id, provider),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS creator_live_settings (
    creator_id TEXT PRIMARY KEY,
    subscriber_only INTEGER NOT NULL,
    slow_mode_seconds INTEGER NOT NULL,
    auto_mod_level TEXT NOT NULL,
    notify_followers_default INTEGER NOT NULL,
    active_scene_id TEXT NOT NULL,
    scenes_json TEXT NOT NULL,
    bitrate_kbps INTEGER NOT NULL,
    cpu_percent INTEGER NOT NULL,
    dropped_frames INTEGER NOT NULL,
    free_disk_gb REAL NOT NULL,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS creator_stream_health_samples (
    id TEXT PRIMARY KEY,
    creator_id TEXT NOT NULL,
    collected_at TEXT NOT NULL,
    bitrate_kbps INTEGER NOT NULL,
    viewers INTEGER NOT NULL,
    cpu_percent INTEGER NOT NULL,
    dropped_frames INTEGER NOT NULL,
    free_disk_gb REAL NOT NULL,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS creator_subscriber_tiers (
    id TEXT PRIMARY KEY,
    creator_id TEXT NOT NULL,
    tier_name TEXT NOT NULL,
    monthly_price REAL NOT NULL,
    subscriber_count INTEGER NOT NULL,
    accent_color TEXT NOT NULL,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS live_stream_notification_preferences (
    user_id TEXT NOT NULL,
    streamer_id TEXT NOT NULL,
    enabled INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (user_id, streamer_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (streamer_id) REFERENCES streamers(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS live_stream_clip_requests (
    id TEXT PRIMARY KEY,
    stream_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (stream_id) REFERENCES live_streams(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS live_stream_reports (
    id TEXT PRIMARY KEY,
    stream_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    details TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (stream_id) REFERENCES live_streams(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_connected_accounts_user_id ON connected_accounts(user_id);
CREATE INDEX IF NOT EXISTS idx_creator_stream_health_creator_time ON creator_stream_health_samples(creator_id, collected_at DESC);
CREATE INDEX IF NOT EXISTS idx_creator_subscriber_tiers_creator_id ON creator_subscriber_tiers(creator_id);
CREATE INDEX IF NOT EXISTS idx_live_stream_clip_requests_stream_id ON live_stream_clip_requests(stream_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_live_stream_reports_stream_id ON live_stream_reports(stream_id, created_at DESC);
