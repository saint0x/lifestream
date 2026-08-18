ALTER TABLE collaboration_sessions
ADD COLUMN last_event_seq INTEGER NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS collaboration_events (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    actor_user_id TEXT,
    participant_id TEXT,
    event_type TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES collaboration_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (actor_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (participant_id) REFERENCES collaboration_participants(id) ON DELETE SET NULL,
    UNIQUE (session_id, sequence)
);

CREATE INDEX IF NOT EXISTS idx_collaboration_events_session_sequence
ON collaboration_events(session_id, sequence ASC);

CREATE TABLE IF NOT EXISTS collaboration_mirror_grants (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    participant_id TEXT NOT NULL,
    host_creator_id TEXT NOT NULL,
    guest_creator_id TEXT NOT NULL,
    scope TEXT NOT NULL,
    state TEXT NOT NULL,
    publish_to_host INTEGER NOT NULL,
    mirror_to_guest_channel INTEGER NOT NULL,
    token_hash TEXT NOT NULL,
    issued_at TEXT NOT NULL,
    activated_at TEXT,
    revoked_at TEXT,
    expires_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES collaboration_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (participant_id) REFERENCES collaboration_participants(id) ON DELETE CASCADE,
    FOREIGN KEY (host_creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (guest_creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE,
    UNIQUE (token_hash)
);

CREATE INDEX IF NOT EXISTS idx_collaboration_mirror_grants_participant_state
ON collaboration_mirror_grants(participant_id, state, issued_at DESC);

CREATE INDEX IF NOT EXISTS idx_collaboration_mirror_grants_guest_state
ON collaboration_mirror_grants(guest_creator_id, state, issued_at DESC);
