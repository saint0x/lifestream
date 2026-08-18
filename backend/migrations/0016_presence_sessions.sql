CREATE TABLE IF NOT EXISTS live_viewer_sessions (
    id TEXT PRIMARY KEY,
    stream_id TEXT NOT NULL,
    user_id TEXT,
    session_token_hash TEXT NOT NULL UNIQUE,
    connected_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    disconnected_at TEXT,
    FOREIGN KEY (stream_id) REFERENCES live_streams(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_live_viewer_sessions_stream_presence
ON live_viewer_sessions(stream_id, disconnected_at, last_seen_at DESC);

CREATE INDEX IF NOT EXISTS idx_live_viewer_sessions_user_presence
ON live_viewer_sessions(user_id, disconnected_at, last_seen_at DESC)
WHERE user_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS collaboration_socket_sessions (
    id TEXT PRIMARY KEY,
    collaboration_session_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    creator_id TEXT,
    participant_id TEXT,
    session_token_hash TEXT NOT NULL UNIQUE,
    connected_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    disconnected_at TEXT,
    FOREIGN KEY (collaboration_session_id) REFERENCES collaboration_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE SET NULL,
    FOREIGN KEY (participant_id) REFERENCES collaboration_participants(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_collaboration_socket_sessions_session_presence
ON collaboration_socket_sessions(collaboration_session_id, disconnected_at, last_seen_at DESC);

CREATE INDEX IF NOT EXISTS idx_collaboration_socket_sessions_user_presence
ON collaboration_socket_sessions(user_id, disconnected_at, last_seen_at DESC);
