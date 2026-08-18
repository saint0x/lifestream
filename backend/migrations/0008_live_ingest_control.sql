CREATE TABLE IF NOT EXISTS live_ingest_sessions (
    id TEXT PRIMARY KEY,
    creator_id TEXT NOT NULL,
    broadcast_id TEXT NOT NULL,
    stream_key_hash TEXT NOT NULL,
    ingest_token_hash TEXT NOT NULL UNIQUE,
    protocol TEXT NOT NULL,
    ingest_server TEXT NOT NULL,
    status TEXT NOT NULL,
    bitrate_kbps INTEGER NOT NULL,
    viewers INTEGER NOT NULL,
    dropped_frames INTEGER NOT NULL,
    connected_at TEXT NOT NULL,
    last_heartbeat_at TEXT NOT NULL,
    disconnected_at TEXT,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (broadcast_id) REFERENCES broadcasts(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_live_ingest_sessions_creator_status
ON live_ingest_sessions(creator_id, status, connected_at DESC);

CREATE INDEX IF NOT EXISTS idx_live_ingest_sessions_broadcast_status
ON live_ingest_sessions(broadcast_id, status, connected_at DESC);
