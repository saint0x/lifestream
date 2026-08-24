ALTER TABLE live_ingest_sessions
ADD COLUMN source_validation_state TEXT NOT NULL DEFAULT 'awaiting_probe';

ALTER TABLE live_ingest_sessions
ADD COLUMN source_validation_issues_json TEXT NOT NULL DEFAULT '[]';
