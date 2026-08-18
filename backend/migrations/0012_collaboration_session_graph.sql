CREATE TABLE IF NOT EXISTS collaboration_sessions (
    id TEXT PRIMARY KEY,
    host_creator_id TEXT NOT NULL,
    source_broadcast_id TEXT NOT NULL,
    title TEXT NOT NULL,
    status TEXT NOT NULL,
    chat_mode TEXT NOT NULL,
    recording_policy TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    activated_at TEXT,
    ended_at TEXT,
    FOREIGN KEY (host_creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (source_broadcast_id) REFERENCES broadcasts(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS collaboration_invites (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    host_creator_id TEXT NOT NULL,
    invitee_user_id TEXT NOT NULL,
    invitee_creator_id TEXT,
    role TEXT NOT NULL,
    state TEXT NOT NULL,
    mirror_to_guest_channel INTEGER NOT NULL,
    message TEXT,
    created_at TEXT NOT NULL,
    responded_at TEXT,
    expires_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES collaboration_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (host_creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (invitee_user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (invitee_creator_id) REFERENCES creator_profiles(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS collaboration_participants (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    invite_id TEXT,
    user_id TEXT NOT NULL,
    creator_id TEXT,
    role TEXT NOT NULL,
    state TEXT NOT NULL,
    publish_to_host INTEGER NOT NULL,
    mirror_to_guest_channel INTEGER NOT NULL,
    can_speak_in_chat INTEGER NOT NULL,
    joined_at TEXT,
    left_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES collaboration_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (invite_id) REFERENCES collaboration_invites(id) ON DELETE SET NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE SET NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_collaboration_sessions_broadcast
ON collaboration_sessions(source_broadcast_id)
WHERE status IN ('pending', 'active');

CREATE UNIQUE INDEX IF NOT EXISTS idx_collaboration_participants_session_user
ON collaboration_participants(session_id, user_id);

CREATE INDEX IF NOT EXISTS idx_collaboration_sessions_host_status
ON collaboration_sessions(host_creator_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_collaboration_invites_invitee_state
ON collaboration_invites(invitee_user_id, state, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_collaboration_participants_session_state
ON collaboration_participants(session_id, state, created_at ASC);
