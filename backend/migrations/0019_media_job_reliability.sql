ALTER TABLE upload_jobs
ADD COLUMN processing_attempt_count INTEGER NOT NULL DEFAULT 0;

ALTER TABLE upload_jobs
ADD COLUMN last_processing_error TEXT;

ALTER TABLE upload_jobs
ADD COLUMN last_failed_at TEXT;

CREATE INDEX IF NOT EXISTS idx_upload_jobs_status_updated_at
ON upload_jobs(status, updated_at ASC);
