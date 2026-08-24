CREATE TABLE IF NOT EXISTS viewer_events (
    id TEXT PRIMARY KEY,
    visitor_id TEXT NOT NULL,
    user_id TEXT,
    event_type TEXT NOT NULL,
    content_id TEXT,
    content_kind TEXT,
    episode_id TEXT,
    stream_id TEXT,
    session_id TEXT,
    path TEXT,
    url TEXT,
    referrer_url TEXT,
    landing_url TEXT,
    initial_referrer_url TEXT,
    utm_source TEXT,
    utm_medium TEXT,
    utm_campaign TEXT,
    utm_term TEXT,
    utm_content TEXT,
    progress_sec BIGINT,
    duration_sec BIGINT,
    watch_time_ms BIGINT,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    occurred_at TEXT NOT NULL,
    received_at TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_viewer_events_visitor_time
ON viewer_events(visitor_id, occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_viewer_events_user_time
ON viewer_events(user_id, occurred_at DESC)
WHERE user_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_viewer_events_content_time
ON viewer_events(content_kind, content_id, occurred_at DESC)
WHERE content_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_viewer_events_type_time
ON viewer_events(event_type, occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_viewer_events_campaign_time
ON viewer_events(utm_source, utm_medium, utm_campaign, occurred_at DESC)
WHERE utm_source IS NOT NULL OR utm_campaign IS NOT NULL;
