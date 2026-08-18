CREATE TABLE IF NOT EXISTS media_assets (
    id TEXT PRIMARY KEY,
    creator_id TEXT NOT NULL,
    upload_job_id TEXT NOT NULL UNIQUE,
    upload_id TEXT,
    series_id TEXT,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    status TEXT NOT NULL,
    visibility TEXT NOT NULL,
    source_relative_path TEXT NOT NULL,
    poster_relative_path TEXT,
    playback_relative_path TEXT,
    mime_type TEXT NOT NULL,
    checksum_sha256 TEXT,
    container_format TEXT,
    file_size_bytes INTEGER NOT NULL,
    duration_sec REAL NOT NULL,
    width INTEGER,
    height INTEGER,
    frame_rate REAL,
    video_codec TEXT,
    audio_codec TEXT,
    has_video INTEGER NOT NULL,
    has_audio INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    processed_at TEXT,
    published_content_id TEXT,
    FOREIGN KEY (upload_job_id) REFERENCES upload_jobs(id) ON DELETE CASCADE,
    FOREIGN KEY (series_id) REFERENCES creator_series_projects(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_media_assets_creator_id_created_at
ON media_assets(creator_id, created_at DESC);

CREATE TABLE IF NOT EXISTS media_asset_variants (
    id TEXT PRIMARY KEY,
    asset_id TEXT NOT NULL,
    variant_type TEXT NOT NULL,
    label TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    width INTEGER,
    height INTEGER,
    bitrate_bps INTEGER,
    file_size_bytes INTEGER NOT NULL,
    is_default INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (asset_id) REFERENCES media_assets(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_media_asset_variants_asset_id
ON media_asset_variants(asset_id, created_at ASC);

CREATE TABLE IF NOT EXISTS media_processing_runs (
    id TEXT PRIMARY KEY,
    creator_id TEXT NOT NULL,
    upload_job_id TEXT NOT NULL,
    asset_id TEXT,
    stage TEXT NOT NULL,
    status TEXT NOT NULL,
    details_json TEXT NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    FOREIGN KEY (upload_job_id) REFERENCES upload_jobs(id) ON DELETE CASCADE,
    FOREIGN KEY (asset_id) REFERENCES media_assets(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_media_processing_runs_job_id
ON media_processing_runs(upload_job_id, started_at DESC);
