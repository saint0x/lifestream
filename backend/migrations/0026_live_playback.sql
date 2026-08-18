ALTER TABLE live_streams ADD COLUMN playback_asset_id TEXT REFERENCES media_assets(id) ON DELETE SET NULL;
ALTER TABLE live_streams ADD COLUMN poster_relative_path TEXT;
ALTER TABLE live_streams ADD COLUMN playback_relative_path TEXT;
