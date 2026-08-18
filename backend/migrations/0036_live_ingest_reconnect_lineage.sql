ALTER TABLE live_ingest_sessions
ADD COLUMN previous_session_id TEXT REFERENCES live_ingest_sessions(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_live_ingest_sessions_broadcast_connected_at
ON live_ingest_sessions(broadcast_id, connected_at DESC);
