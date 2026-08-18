CREATE TABLE IF NOT EXISTS creator_series_projects (
    id TEXT PRIMARY KEY,
    creator_id TEXT NOT NULL,
    slug TEXT NOT NULL,
    title TEXT NOT NULL,
    synopsis TEXT NOT NULL,
    rating TEXT NOT NULL,
    genres_json TEXT NOT NULL,
    hero_color TEXT NOT NULL,
    poster_url TEXT NOT NULL,
    backdrop_url TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (creator_id, slug),
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE
);

ALTER TABLE uploads ADD COLUMN series_id TEXT REFERENCES creator_series_projects(id);

CREATE TABLE IF NOT EXISTS upload_jobs (
    id TEXT PRIMARY KEY,
    creator_id TEXT NOT NULL,
    upload_id TEXT,
    series_id TEXT,
    kind TEXT NOT NULL,
    source_type TEXT NOT NULL,
    status TEXT NOT NULL,
    title TEXT NOT NULL,
    intended_visibility TEXT NOT NULL,
    bytes_expected INTEGER NOT NULL,
    bytes_received INTEGER NOT NULL,
    storage_key TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    published_content_id TEXT,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (upload_id) REFERENCES uploads(id) ON DELETE SET NULL,
    FOREIGN KEY (series_id) REFERENCES creator_series_projects(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_creator_series_projects_creator_id ON creator_series_projects(creator_id);
CREATE INDEX IF NOT EXISTS idx_upload_jobs_creator_id_created_at ON upload_jobs(creator_id, created_at DESC);
