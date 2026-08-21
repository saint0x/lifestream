PRAGMA foreign_keys = OFF;

CREATE INDEX IF NOT EXISTS idx_live_ingest_sessions_creator_connected_at
ON live_ingest_sessions(creator_id, connected_at DESC);

CREATE INDEX IF NOT EXISTS idx_live_ingest_sessions_creator_status_heartbeat
ON live_ingest_sessions(creator_id, status, last_heartbeat_at DESC, connected_at DESC);

CREATE INDEX IF NOT EXISTS idx_live_ingest_sessions_broadcast_status_heartbeat
ON live_ingest_sessions(broadcast_id, status, last_heartbeat_at DESC, connected_at DESC);

PRAGMA foreign_keys = ON;
