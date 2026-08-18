CREATE TABLE IF NOT EXISTS media_timeline_previews (
    id TEXT PRIMARY KEY,
    asset_id TEXT NOT NULL,
    label TEXT NOT NULL,
    image_relative_path TEXT NOT NULL,
    vtt_relative_path TEXT NOT NULL,
    tile_width INTEGER NOT NULL,
    tile_height INTEGER NOT NULL,
    columns_count INTEGER NOT NULL,
    rows_count INTEGER NOT NULL,
    interval_sec REAL NOT NULL,
    frame_count INTEGER NOT NULL,
    is_default INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (asset_id) REFERENCES media_assets(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_media_timeline_previews_asset_label
ON media_timeline_previews(asset_id, label);

CREATE INDEX IF NOT EXISTS idx_media_timeline_previews_asset_created_at
ON media_timeline_previews(asset_id, created_at ASC);
