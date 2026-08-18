CREATE TABLE IF NOT EXISTS collaboration_mirror_pickups (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    participant_id TEXT NOT NULL,
    grant_id TEXT NOT NULL,
    host_creator_id TEXT NOT NULL,
    guest_creator_id TEXT NOT NULL,
    source_broadcast_id TEXT NOT NULL,
    guest_broadcast_id TEXT NOT NULL,
    state TEXT NOT NULL,
    activated_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    ended_at TEXT,
    FOREIGN KEY (session_id) REFERENCES collaboration_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (participant_id) REFERENCES collaboration_participants(id) ON DELETE CASCADE,
    FOREIGN KEY (grant_id) REFERENCES collaboration_mirror_grants(id) ON DELETE CASCADE,
    FOREIGN KEY (host_creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (guest_creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (source_broadcast_id) REFERENCES broadcasts(id) ON DELETE CASCADE,
    FOREIGN KEY (guest_broadcast_id) REFERENCES broadcasts(id) ON DELETE CASCADE,
    UNIQUE (grant_id),
    UNIQUE (guest_broadcast_id)
);

CREATE INDEX IF NOT EXISTS idx_collaboration_mirror_pickups_session_state
ON collaboration_mirror_pickups(session_id, state, activated_at DESC);

CREATE INDEX IF NOT EXISTS idx_collaboration_mirror_pickups_participant_state
ON collaboration_mirror_pickups(participant_id, state, activated_at DESC);

CREATE INDEX IF NOT EXISTS idx_collaboration_mirror_pickups_guest_state
ON collaboration_mirror_pickups(guest_creator_id, state, activated_at DESC);
