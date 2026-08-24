CREATE TABLE IF NOT EXISTS creator_live_socket_sessions (
    id TEXT PRIMARY KEY,
    creator_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    session_token_hash TEXT NOT NULL UNIQUE,
    connected_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    disconnected_at TEXT,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_creator_live_socket_sessions_creator_presence
ON creator_live_socket_sessions(creator_id, disconnected_at, last_seen_at DESC);

CREATE INDEX IF NOT EXISTS idx_creator_live_socket_sessions_user_presence
ON creator_live_socket_sessions(user_id, disconnected_at, last_seen_at DESC);
