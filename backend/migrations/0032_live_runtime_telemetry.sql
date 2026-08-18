CREATE TABLE IF NOT EXISTS live_runtime_telemetry (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    creator_id TEXT NOT NULL,
    broadcast_id TEXT NOT NULL,
    sample_kind TEXT NOT NULL,
    runtime_state TEXT NOT NULL,
    packaging_status TEXT NOT NULL,
    archive_status TEXT NOT NULL,
    bitrate_kbps INTEGER NOT NULL,
    viewers INTEGER NOT NULL,
    dropped_frames INTEGER NOT NULL,
    cpu_percent INTEGER,
    free_disk_gb REAL,
    detail_json TEXT NOT NULL,
    collected_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES live_ingest_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (broadcast_id) REFERENCES broadcasts(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_live_runtime_telemetry_creator_collected_at
ON live_runtime_telemetry(creator_id, collected_at DESC);

CREATE INDEX IF NOT EXISTS idx_live_runtime_telemetry_session_collected_at
ON live_runtime_telemetry(session_id, collected_at DESC);
