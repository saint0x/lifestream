ALTER TABLE chat_messages
ADD COLUMN hidden_by_moderation INTEGER NOT NULL DEFAULT 0;

ALTER TABLE live_stream_reports
ADD COLUMN status TEXT NOT NULL DEFAULT 'open';

ALTER TABLE live_stream_reports
ADD COLUMN resolved_by_user_id TEXT;

ALTER TABLE live_stream_reports
ADD COLUMN resolution_note TEXT;

ALTER TABLE live_stream_reports
ADD COLUMN resolved_at TEXT;

CREATE TABLE IF NOT EXISTS creator_moderators (
    creator_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    role TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (creator_id, user_id),
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS live_moderation_actions (
    id TEXT PRIMARY KEY,
    stream_id TEXT NOT NULL,
    creator_id TEXT NOT NULL,
    subject_user_id TEXT NOT NULL,
    actor_user_id TEXT NOT NULL,
    action_type TEXT NOT NULL,
    reason TEXT NOT NULL,
    state TEXT NOT NULL,
    expires_at TEXT,
    created_at TEXT NOT NULL,
    revoked_at TEXT,
    FOREIGN KEY (stream_id) REFERENCES live_streams(id) ON DELETE CASCADE,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (subject_user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (actor_user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_live_moderation_actions_stream_subject_state
ON live_moderation_actions(stream_id, subject_user_id, state, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_live_moderation_actions_creator_state
ON live_moderation_actions(creator_id, state, created_at DESC);

CREATE TABLE IF NOT EXISTS moderation_audit_log (
    id TEXT PRIMARY KEY,
    creator_id TEXT NOT NULL,
    stream_id TEXT,
    actor_user_id TEXT NOT NULL,
    subject_user_id TEXT,
    event_type TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (stream_id) REFERENCES live_streams(id) ON DELETE SET NULL,
    FOREIGN KEY (actor_user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (subject_user_id) REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_creator_moderators_creator_id
ON creator_moderators(creator_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_moderation_audit_log_creator_stream
ON moderation_audit_log(creator_id, stream_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_live_stream_reports_stream_status
ON live_stream_reports(stream_id, status, created_at DESC);
