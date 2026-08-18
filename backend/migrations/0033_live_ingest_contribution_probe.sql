ALTER TABLE live_ingest_sessions
ADD COLUMN contribution_class TEXT NOT NULL DEFAULT 'rtmp_push';

ALTER TABLE live_ingest_sessions
ADD COLUMN contribution_state TEXT NOT NULL DEFAULT 'awaiting_probe';

ALTER TABLE live_ingest_sessions
ADD COLUMN ingest_latency_ms INTEGER;

ALTER TABLE live_ingest_sessions
ADD COLUMN source_container_format TEXT;

ALTER TABLE live_ingest_sessions
ADD COLUMN source_video_codec TEXT;

ALTER TABLE live_ingest_sessions
ADD COLUMN source_audio_codec TEXT;

ALTER TABLE live_ingest_sessions
ADD COLUMN source_width INTEGER;

ALTER TABLE live_ingest_sessions
ADD COLUMN source_height INTEGER;

ALTER TABLE live_ingest_sessions
ADD COLUMN source_frame_rate REAL;

ALTER TABLE live_ingest_sessions
ADD COLUMN source_audio_sample_rate_hz INTEGER;

ALTER TABLE live_ingest_sessions
ADD COLUMN source_audio_channels INTEGER;

ALTER TABLE live_ingest_sessions
ADD COLUMN last_source_probe_at TEXT;
