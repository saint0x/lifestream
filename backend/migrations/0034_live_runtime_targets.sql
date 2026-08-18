CREATE TABLE IF NOT EXISTS live_runtime_targets (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    creator_id TEXT NOT NULL,
    broadcast_id TEXT NOT NULL,
    target_kind TEXT NOT NULL,
    target_key TEXT NOT NULL,
    target_label TEXT NOT NULL,
    route_state TEXT NOT NULL,
    target_creator_id TEXT,
    target_broadcast_id TEXT,
    playback_enabled INTEGER NOT NULL,
    recording_enabled INTEGER NOT NULL,
    mix_minus_required INTEGER NOT NULL,
    relative_path TEXT,
    source_participant_ids_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(session_id, target_kind, target_key),
    FOREIGN KEY (session_id) REFERENCES live_ingest_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (broadcast_id) REFERENCES broadcasts(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_live_runtime_targets_session_updated_at
ON live_runtime_targets(session_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_live_runtime_targets_creator_updated_at
ON live_runtime_targets(creator_id, updated_at DESC);
