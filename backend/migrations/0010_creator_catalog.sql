CREATE TABLE IF NOT EXISTS creator_series_seasons (
    id TEXT PRIMARY KEY,
    series_id TEXT NOT NULL,
    season_number INTEGER NOT NULL,
    title TEXT NOT NULL,
    synopsis TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (series_id, season_number),
    FOREIGN KEY (series_id) REFERENCES creator_series_projects(id) ON DELETE CASCADE
);

ALTER TABLE uploads ADD COLUMN slug TEXT;
ALTER TABLE uploads ADD COLUMN release_at TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_uploads_creator_slug
ON uploads(creator_id, slug)
WHERE slug IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_uploads_catalog_visibility_release
ON uploads(status, visibility, release_at DESC, published_at DESC);

CREATE INDEX IF NOT EXISTS idx_uploads_series_episode_order
ON uploads(series_id, season_number, episode_number, release_at ASC, published_at ASC);

CREATE INDEX IF NOT EXISTS idx_creator_series_seasons_series_order
ON creator_series_seasons(series_id, season_number ASC);
