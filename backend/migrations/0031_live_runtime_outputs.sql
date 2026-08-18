CREATE TABLE IF NOT EXISTS live_runtime_outputs (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL UNIQUE,
    creator_id TEXT NOT NULL,
    broadcast_id TEXT NOT NULL,
    runtime_state TEXT NOT NULL,
    packaging_status TEXT NOT NULL,
    archive_status TEXT NOT NULL,
    manifest_relative_path TEXT,
    archive_relative_path TEXT,
    last_error TEXT,
    last_runtime_event_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES live_ingest_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (broadcast_id) REFERENCES broadcasts(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_live_runtime_outputs_creator_updated_at
ON live_runtime_outputs(creator_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_live_runtime_outputs_broadcast_updated_at
ON live_runtime_outputs(broadcast_id, updated_at DESC);
