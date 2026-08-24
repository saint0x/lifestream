ALTER TABLE playback_sessions
ADD COLUMN device_id TEXT;

ALTER TABLE playback_sessions
ADD COLUMN device_name TEXT;

ALTER TABLE playback_sessions
ADD COLUMN player_version TEXT;

ALTER TABLE playback_sessions
ADD COLUMN capabilities_json TEXT;

CREATE INDEX IF NOT EXISTS idx_playback_sessions_live_auth_lookup
ON playback_sessions(content_kind, content_id, auth_session_id, expires_at DESC);

CREATE INDEX IF NOT EXISTS idx_playback_sessions_live_device_lookup
ON playback_sessions(content_kind, content_id, device_id, expires_at DESC);
