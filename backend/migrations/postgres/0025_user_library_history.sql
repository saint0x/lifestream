CREATE TABLE IF NOT EXISTS user_watch_history (
    user_id TEXT NOT NULL,
    content_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    episode_id TEXT,
    progress_sec INTEGER NOT NULL,
    duration_sec INTEGER NOT NULL,
    completed INTEGER NOT NULL DEFAULT 0,
    completed_at TEXT,
    last_watched_at TEXT NOT NULL,
    PRIMARY KEY (user_id, content_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_user_watch_history_recent
ON user_watch_history(user_id, last_watched_at DESC);

CREATE INDEX IF NOT EXISTS idx_user_watch_history_completed
ON user_watch_history(user_id, completed, completed_at DESC, last_watched_at DESC);
