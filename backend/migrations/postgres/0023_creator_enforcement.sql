CREATE TABLE IF NOT EXISTS creator_enforcement_actions (
    id TEXT PRIMARY KEY,
    creator_id TEXT NOT NULL,
    scope TEXT NOT NULL,
    state TEXT NOT NULL,
    reason TEXT NOT NULL,
    resolution_note TEXT,
    created_by_user_id TEXT NOT NULL,
    released_by_user_id TEXT,
    created_at TEXT NOT NULL,
    released_at TEXT,
    expires_at TEXT,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (released_by_user_id) REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_creator_enforcement_actions_creator_state
ON creator_enforcement_actions(creator_id, state, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_creator_enforcement_actions_scope_state
ON creator_enforcement_actions(scope, state, created_at DESC);
