CREATE TABLE IF NOT EXISTS series (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    tagline TEXT,
    synopsis TEXT NOT NULL,
    year INTEGER NOT NULL,
    rating TEXT NOT NULL,
    genres_json TEXT NOT NULL,
    images_json TEXT NOT NULL,
    credits_json TEXT NOT NULL,
    score INTEGER NOT NULL,
    is_original INTEGER NOT NULL,
    trending INTEGER NOT NULL,
    hero_color TEXT NOT NULL,
    status TEXT NOT NULL,
    total_episodes INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS seasons (
    series_id TEXT NOT NULL,
    season_number INTEGER NOT NULL,
    title TEXT NOT NULL,
    PRIMARY KEY (series_id, season_number),
    FOREIGN KEY (series_id) REFERENCES series(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS episodes (
    id TEXT PRIMARY KEY,
    series_id TEXT NOT NULL,
    season_number INTEGER NOT NULL,
    episode_number INTEGER NOT NULL,
    title TEXT NOT NULL,
    synopsis TEXT NOT NULL,
    duration_sec INTEGER NOT NULL,
    aired_at TEXT NOT NULL,
    thumbnail TEXT NOT NULL,
    FOREIGN KEY (series_id) REFERENCES series(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS films (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    tagline TEXT,
    synopsis TEXT NOT NULL,
    year INTEGER NOT NULL,
    rating TEXT NOT NULL,
    genres_json TEXT NOT NULL,
    images_json TEXT NOT NULL,
    credits_json TEXT NOT NULL,
    score INTEGER NOT NULL,
    is_original INTEGER NOT NULL,
    trending INTEGER NOT NULL,
    hero_color TEXT NOT NULL,
    duration_sec INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS streamers (
    id TEXT PRIMARY KEY,
    handle TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    avatar TEXT NOT NULL,
    bio TEXT NOT NULL,
    followers INTEGER NOT NULL,
    is_partner INTEGER NOT NULL,
    is_live INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS live_streams (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    category TEXT NOT NULL,
    tags_json TEXT NOT NULL,
    streamer_id TEXT NOT NULL,
    viewers INTEGER NOT NULL,
    started_at TEXT NOT NULL,
    thumbnail TEXT NOT NULL,
    language TEXT NOT NULL,
    is_mature INTEGER NOT NULL,
    FOREIGN KEY (streamer_id) REFERENCES streamers(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS categories (
    slug TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    cover_image TEXT NOT NULL,
    live_viewers INTEGER NOT NULL,
    live_channels INTEGER NOT NULL,
    tags_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    handle TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    avatar TEXT NOT NULL,
    tier TEXT NOT NULL,
    joined_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS user_watchlist (
    user_id TEXT NOT NULL,
    content_id TEXT NOT NULL,
    PRIMARY KEY (user_id, content_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS user_following (
    user_id TEXT NOT NULL,
    streamer_id TEXT NOT NULL,
    PRIMARY KEY (user_id, streamer_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (streamer_id) REFERENCES streamers(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS continue_watching (
    user_id TEXT NOT NULL,
    content_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    episode_id TEXT,
    progress_sec INTEGER NOT NULL,
    duration_sec INTEGER NOT NULL,
    last_watched_at TEXT NOT NULL,
    PRIMARY KEY (user_id, content_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS creator_profiles (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL UNIQUE,
    handle TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    avatar TEXT NOT NULL,
    banner TEXT NOT NULL,
    tagline TEXT NOT NULL,
    bio TEXT NOT NULL,
    partner_status TEXT NOT NULL,
    joined_at TEXT NOT NULL,
    stream_key TEXT NOT NULL,
    rtmp_url TEXT NOT NULL,
    default_category TEXT NOT NULL,
    default_tags_json TEXT NOT NULL,
    followers INTEGER NOT NULL,
    subscribers INTEGER NOT NULL,
    monthly_viewers INTEGER NOT NULL,
    total_watch_hours INTEGER NOT NULL,
    live_status TEXT NOT NULL,
    current_broadcast_id TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS broadcasts (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    category TEXT NOT NULL,
    tags_json TEXT NOT NULL,
    status TEXT NOT NULL,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    duration_sec INTEGER,
    peak_viewers INTEGER NOT NULL,
    average_viewers INTEGER NOT NULL,
    chat_messages INTEGER NOT NULL,
    new_followers INTEGER NOT NULL,
    new_subscribers INTEGER NOT NULL,
    revenue REAL NOT NULL,
    thumbnail TEXT NOT NULL,
    is_mature INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS uploads (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    kind TEXT NOT NULL,
    duration_sec INTEGER NOT NULL,
    uploaded_at TEXT NOT NULL,
    published_at TEXT,
    status TEXT NOT NULL,
    visibility TEXT NOT NULL,
    views INTEGER NOT NULL,
    likes INTEGER NOT NULL,
    comments INTEGER NOT NULL,
    watch_hours INTEGER NOT NULL,
    thumbnail TEXT NOT NULL,
    series_title TEXT,
    season_number INTEGER,
    episode_number INTEGER,
    size_bytes INTEGER NOT NULL,
    resolution TEXT NOT NULL,
    transcode_progress REAL
);

CREATE TABLE IF NOT EXISTS analytics_points (
    id TEXT PRIMARY KEY,
    date TEXT NOT NULL,
    viewers INTEGER NOT NULL,
    watch_minutes INTEGER NOT NULL,
    revenue REAL NOT NULL,
    new_followers INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS traffic_sources (
    source TEXT PRIMARY KEY,
    sessions INTEGER NOT NULL,
    share REAL NOT NULL
);

CREATE TABLE IF NOT EXISTS top_content (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    kind TEXT NOT NULL,
    views INTEGER NOT NULL,
    watch_hours INTEGER NOT NULL,
    trend REAL NOT NULL,
    thumbnail TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS revenue_entries (
    id TEXT PRIMARY KEY,
    date TEXT NOT NULL,
    source TEXT NOT NULL,
    description TEXT NOT NULL,
    amount REAL NOT NULL
);

CREATE TABLE IF NOT EXISTS creator_notifications (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    body TEXT NOT NULL,
    sent_at TEXT NOT NULL,
    amount REAL,
    actor TEXT
);

CREATE TABLE IF NOT EXISTS chat_messages (
    id TEXT PRIMARY KEY,
    stream_id TEXT NOT NULL,
    user_handle TEXT NOT NULL,
    display_name TEXT NOT NULL,
    color TEXT NOT NULL,
    badges_json TEXT NOT NULL,
    body TEXT NOT NULL,
    sent_at TEXT NOT NULL,
    FOREIGN KEY (stream_id) REFERENCES live_streams(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS auth_sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    label TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    scopes_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT,
    revoked_at TEXT,
    last_used_at TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_series_slug ON series(slug);
CREATE INDEX IF NOT EXISTS idx_films_slug ON films(slug);
CREATE INDEX IF NOT EXISTS idx_live_streams_slug ON live_streams(slug);
CREATE INDEX IF NOT EXISTS idx_live_streams_streamer_id ON live_streams(streamer_id);
CREATE INDEX IF NOT EXISTS idx_chat_messages_stream_sent_at ON chat_messages(stream_id, sent_at DESC);
CREATE INDEX IF NOT EXISTS idx_continue_watching_user_time ON continue_watching(user_id, last_watched_at DESC);
CREATE INDEX IF NOT EXISTS idx_broadcasts_status_started_at ON broadcasts(status, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_uploads_status_uploaded_at ON uploads(status, uploaded_at DESC);
CREATE INDEX IF NOT EXISTS idx_auth_sessions_user_id ON auth_sessions(user_id);
