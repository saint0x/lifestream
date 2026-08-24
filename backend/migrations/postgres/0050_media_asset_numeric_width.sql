ALTER TABLE media_assets
ALTER COLUMN file_size_bytes TYPE BIGINT;

ALTER TABLE media_asset_variants
ALTER COLUMN bitrate_bps TYPE BIGINT,
ALTER COLUMN file_size_bytes TYPE BIGINT;
