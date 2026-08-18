CREATE TABLE IF NOT EXISTS playback_sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT,
    creator_id TEXT,
    asset_id TEXT NOT NULL,
    content_id TEXT NOT NULL,
    content_kind TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    access_scope TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    last_used_at TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE SET NULL,
    FOREIGN KEY (asset_id) REFERENCES media_assets(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_playback_sessions_asset_id
ON playback_sessions(asset_id, expires_at DESC);

CREATE INDEX IF NOT EXISTS idx_playback_sessions_user_id
ON playback_sessions(user_id, expires_at DESC);
