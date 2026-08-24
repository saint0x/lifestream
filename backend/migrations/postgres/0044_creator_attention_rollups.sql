CREATE TABLE IF NOT EXISTS creator_attention_daily (
    creator_id TEXT NOT NULL,
    day TEXT NOT NULL,
    algorithm_version TEXT NOT NULL,
    qualified_viewers INTEGER NOT NULL,
    verified_viewer_score REAL NOT NULL,
    creator_attention_value REAL NOT NULL,
    average_watch_minutes REAL NOT NULL,
    attention_multiplier REAL NOT NULL,
    engagement_multiplier REAL NOT NULL,
    retention_multiplier REAL NOT NULL,
    audience_quality_multiplier REAL NOT NULL,
    data_confidence_multiplier REAL NOT NULL,
    qualified_viewer_rate REAL NOT NULL,
    returning_viewer_rate REAL NOT NULL,
    measured_sessions INTEGER NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (creator_id, day, algorithm_version),
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_creator_attention_daily_creator_day
ON creator_attention_daily(creator_id, day DESC);
