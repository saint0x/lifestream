ALTER TABLE upload_jobs
ALTER COLUMN bytes_expected TYPE BIGINT,
ALTER COLUMN bytes_received TYPE BIGINT;

ALTER TABLE upload_job_ingest_sessions
ALTER COLUMN bytes_received TYPE BIGINT;
