CREATE TABLE IF NOT EXISTS live_ingest_events (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    creator_id TEXT NOT NULL,
    broadcast_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES live_ingest_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (broadcast_id) REFERENCES broadcasts(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_live_ingest_events_creator_created_at
ON live_ingest_events(creator_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_live_ingest_events_session_created_at
ON live_ingest_events(session_id, created_at DESC);
