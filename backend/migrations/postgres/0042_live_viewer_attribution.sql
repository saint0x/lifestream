ALTER TABLE live_viewer_sessions
ADD COLUMN visitor_id TEXT;

ALTER TABLE live_viewer_sessions
ADD COLUMN landing_url TEXT;

ALTER TABLE live_viewer_sessions
ADD COLUMN initial_referrer_url TEXT;

ALTER TABLE live_viewer_sessions
ADD COLUMN current_url TEXT;

ALTER TABLE live_viewer_sessions
ADD COLUMN current_referrer_url TEXT;

ALTER TABLE live_viewer_sessions
ADD COLUMN utm_source TEXT;

ALTER TABLE live_viewer_sessions
ADD COLUMN utm_medium TEXT;

ALTER TABLE live_viewer_sessions
ADD COLUMN utm_campaign TEXT;

ALTER TABLE live_viewer_sessions
ADD COLUMN utm_term TEXT;

ALTER TABLE live_viewer_sessions
ADD COLUMN utm_content TEXT;

ALTER TABLE live_viewer_sessions
ADD COLUMN attribution_source TEXT;

ALTER TABLE live_viewer_sessions
ADD COLUMN attribution_medium TEXT;

ALTER TABLE live_viewer_sessions
ADD COLUMN attribution_campaign TEXT;

CREATE INDEX IF NOT EXISTS idx_live_viewer_sessions_stream_visitor_presence
ON live_viewer_sessions(stream_id, visitor_id, disconnected_at, last_seen_at DESC)
WHERE visitor_id IS NOT NULL;
