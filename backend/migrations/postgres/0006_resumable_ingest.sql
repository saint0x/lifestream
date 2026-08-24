ALTER TABLE upload_jobs ADD COLUMN mime_type TEXT NOT NULL DEFAULT 'application/octet-stream';
ALTER TABLE upload_jobs ADD COLUMN checksum_sha256 TEXT;
ALTER TABLE upload_jobs ADD COLUMN completed_at TEXT;

CREATE TABLE IF NOT EXISTS upload_job_ingest_sessions (
    job_id TEXT PRIMARY KEY,
    creator_id TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    upload_token_hash TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    bytes_received INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    FOREIGN KEY (job_id) REFERENCES upload_jobs(id) ON DELETE CASCADE,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_upload_job_ingest_sessions_creator_id
ON upload_job_ingest_sessions(creator_id, created_at DESC);
