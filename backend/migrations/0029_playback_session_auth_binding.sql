ALTER TABLE playback_sessions
ADD COLUMN auth_session_id TEXT REFERENCES auth_sessions(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_playback_sessions_auth_session_id
ON playback_sessions(auth_session_id, expires_at DESC);
