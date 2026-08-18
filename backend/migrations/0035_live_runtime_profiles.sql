ALTER TABLE creator_live_settings
ADD COLUMN delivery_class TEXT NOT NULL DEFAULT 'standard_hls';

ALTER TABLE live_runtime_outputs
ADD COLUMN runtime_class TEXT NOT NULL DEFAULT 'standard_hls';

ALTER TABLE live_runtime_outputs
ADD COLUMN latency_profile TEXT NOT NULL DEFAULT 'standard';

ALTER TABLE live_runtime_outputs
ADD COLUMN segment_format TEXT NOT NULL DEFAULT 'mpegts';

ALTER TABLE live_runtime_outputs
ADD COLUMN partial_segments_enabled INTEGER NOT NULL DEFAULT 0;

ALTER TABLE live_runtime_outputs
ADD COLUMN blocking_reload_enabled INTEGER NOT NULL DEFAULT 0;

ALTER TABLE live_runtime_outputs
ADD COLUMN target_segment_duration_sec INTEGER NOT NULL DEFAULT 6;

ALTER TABLE live_runtime_outputs
ADD COLUMN hold_back_segments INTEGER NOT NULL DEFAULT 3;

ALTER TABLE live_runtime_outputs
ADD COLUMN discontinuity_sequence INTEGER NOT NULL DEFAULT 0;

ALTER TABLE live_runtime_outputs
ADD COLUMN ladder_policy TEXT NOT NULL DEFAULT 'awaiting_probe';

ALTER TABLE live_runtime_outputs
ADD COLUMN content_class TEXT NOT NULL DEFAULT 'unknown';
