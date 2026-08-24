CREATE INDEX IF NOT EXISTS idx_viewer_events_stream_time
ON viewer_events(stream_id, occurred_at DESC)
WHERE stream_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_viewer_events_episode_time
ON viewer_events(episode_id, occurred_at DESC)
WHERE episode_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_viewer_events_visitor_content_time
ON viewer_events(visitor_id, content_kind, content_id, occurred_at DESC)
WHERE content_id IS NOT NULL;
