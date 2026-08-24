WITH seeded_episode_assets AS (
    SELECT *
    FROM (VALUES
        (
            'ser-northlight-s1e1',
            'Aurora Index',
            'Mara finds a signal hidden in a weather archive and loses contact with the first relay station.',
            3040,
            1,
            1,
            'https://images.unsplash.com/photo-1446776811953-b23d57bd21aa?auto=format&fit=crop&w=900&q=80',
            'qa/episodes/ser-northlight-s1e1/master.m3u8',
            'qa/episodes/ser-northlight-s1e1/source.mp4',
            'ast-qa-ser-northlight-s1e1',
            'upj-qa-ser-northlight-s1e1'
        ),
        (
            'ser-northlight-s1e2',
            'Relay Drift',
            'The team follows the interference north as the network starts predicting their route.',
            3180,
            1,
            2,
            'https://images.unsplash.com/photo-1473923377535-0002805f57e8?auto=format&fit=crop&w=900&q=80',
            'qa/episodes/ser-northlight-s1e2/master.m3u8',
            'qa/episodes/ser-northlight-s1e2/source.mp4',
            'ast-qa-ser-northlight-s1e2',
            'upj-qa-ser-northlight-s1e2'
        ),
        (
            'ser-northlight-s1e3',
            'Cold Boot',
            'An offline observatory powers itself on and reveals a map no one built.',
            3220,
            1,
            3,
            'https://images.unsplash.com/photo-1519681393784-d120267933ba?auto=format&fit=crop&w=900&q=80',
            'qa/episodes/ser-northlight-s1e3/master.m3u8',
            'qa/episodes/ser-northlight-s1e3/source.mp4',
            'ast-qa-ser-northlight-s1e3',
            'upj-qa-ser-northlight-s1e3'
        )
    ) AS asset(
        episode_id,
        title,
        synopsis,
        duration_sec,
        season_number,
        episode_number,
        thumbnail,
        playback_relative_path,
        source_relative_path,
        asset_id,
        upload_job_id
    )
),
seed_owner AS (
    SELECT id AS creator_id
    FROM creator_profiles
    WHERE id = 'cr-saint0xsaint'
    LIMIT 1
)
INSERT INTO uploads (
    id, title, description, kind, duration_sec, uploaded_at, published_at, status,
    visibility, views, likes, comments, watch_hours, thumbnail, series_title,
    season_number, episode_number, size_bytes, resolution, transcode_progress,
    creator_id, series_id, slug, release_at, access_policy, access_tier_id,
    price_cents, currency, rental_window_hours
)
SELECT
    asset.episode_id,
    asset.title,
    asset.synopsis,
    'episode',
    asset.duration_sec,
    '2026-08-24T00:00:00Z',
    '2026-08-24T00:00:00Z',
    'published',
    'public',
    0,
    0,
    0,
    0,
    asset.thumbnail,
    'Northlight',
    asset.season_number,
    asset.episode_number,
    4566887,
    '1280x720',
    1.0,
    seed_owner.creator_id,
    NULL,
    asset.episode_id,
    '2026-08-24T00:00:00Z',
    'free',
    NULL,
    NULL,
    NULL,
    NULL
FROM seeded_episode_assets asset
CROSS JOIN seed_owner
ON CONFLICT (id) DO UPDATE SET
    title = EXCLUDED.title,
    description = EXCLUDED.description,
    kind = EXCLUDED.kind,
    duration_sec = EXCLUDED.duration_sec,
    published_at = EXCLUDED.published_at,
    status = EXCLUDED.status,
    visibility = EXCLUDED.visibility,
    thumbnail = EXCLUDED.thumbnail,
    series_title = EXCLUDED.series_title,
    season_number = EXCLUDED.season_number,
    episode_number = EXCLUDED.episode_number,
    size_bytes = EXCLUDED.size_bytes,
    resolution = EXCLUDED.resolution,
    transcode_progress = EXCLUDED.transcode_progress,
    creator_id = EXCLUDED.creator_id,
    slug = EXCLUDED.slug,
    release_at = EXCLUDED.release_at,
    access_policy = EXCLUDED.access_policy,
    access_tier_id = EXCLUDED.access_tier_id,
    price_cents = EXCLUDED.price_cents,
    currency = EXCLUDED.currency,
    rental_window_hours = EXCLUDED.rental_window_hours;

WITH seeded_episode_assets AS (
    SELECT *
    FROM (VALUES
        ('ser-northlight-s1e1', 'Aurora Index', 3040, 'qa/episodes/ser-northlight-s1e1/master.m3u8', 'qa/episodes/ser-northlight-s1e1/source.mp4', 'ast-qa-ser-northlight-s1e1', 'upj-qa-ser-northlight-s1e1'),
        ('ser-northlight-s1e2', 'Relay Drift', 3180, 'qa/episodes/ser-northlight-s1e2/master.m3u8', 'qa/episodes/ser-northlight-s1e2/source.mp4', 'ast-qa-ser-northlight-s1e2', 'upj-qa-ser-northlight-s1e2'),
        ('ser-northlight-s1e3', 'Cold Boot', 3220, 'qa/episodes/ser-northlight-s1e3/master.m3u8', 'qa/episodes/ser-northlight-s1e3/source.mp4', 'ast-qa-ser-northlight-s1e3', 'upj-qa-ser-northlight-s1e3')
    ) AS asset(episode_id, title, duration_sec, playback_relative_path, source_relative_path, asset_id, upload_job_id)
),
seed_owner AS (
    SELECT id AS creator_id
    FROM creator_profiles
    WHERE id = 'cr-saint0xsaint'
    LIMIT 1
)
INSERT INTO upload_jobs (
    id, creator_id, upload_id, series_id, kind, source_type, status, title,
    intended_visibility, bytes_expected, bytes_received, storage_key, created_at,
    updated_at, published_content_id, mime_type, checksum_sha256, completed_at,
    processing_attempt_count, last_processing_error, last_failed_at
)
SELECT
    asset.upload_job_id,
    seed_owner.creator_id,
    asset.episode_id,
    NULL,
    'episode',
    'seed',
    'published',
    asset.title,
    'public',
    4566887,
    4566887,
    asset.source_relative_path,
    '2026-08-24T00:00:00Z',
    '2026-08-24T00:00:00Z',
    asset.episode_id,
    'video/mp4',
    NULL,
    '2026-08-24T00:00:00Z',
    1,
    NULL,
    NULL
FROM seeded_episode_assets asset
CROSS JOIN seed_owner
ON CONFLICT (id) DO UPDATE SET
    creator_id = EXCLUDED.creator_id,
    upload_id = EXCLUDED.upload_id,
    kind = EXCLUDED.kind,
    source_type = EXCLUDED.source_type,
    status = EXCLUDED.status,
    title = EXCLUDED.title,
    intended_visibility = EXCLUDED.intended_visibility,
    bytes_expected = EXCLUDED.bytes_expected,
    bytes_received = EXCLUDED.bytes_received,
    storage_key = EXCLUDED.storage_key,
    updated_at = EXCLUDED.updated_at,
    published_content_id = EXCLUDED.published_content_id,
    mime_type = EXCLUDED.mime_type,
    completed_at = EXCLUDED.completed_at,
    processing_attempt_count = EXCLUDED.processing_attempt_count,
    last_processing_error = EXCLUDED.last_processing_error,
    last_failed_at = EXCLUDED.last_failed_at;

WITH seeded_episode_assets AS (
    SELECT *
    FROM (VALUES
        ('ser-northlight-s1e1', 'Aurora Index', 3040, 'qa/episodes/ser-northlight-s1e1/master.m3u8', 'qa/episodes/ser-northlight-s1e1/source.mp4', 'ast-qa-ser-northlight-s1e1', 'upj-qa-ser-northlight-s1e1'),
        ('ser-northlight-s1e2', 'Relay Drift', 3180, 'qa/episodes/ser-northlight-s1e2/master.m3u8', 'qa/episodes/ser-northlight-s1e2/source.mp4', 'ast-qa-ser-northlight-s1e2', 'upj-qa-ser-northlight-s1e2'),
        ('ser-northlight-s1e3', 'Cold Boot', 3220, 'qa/episodes/ser-northlight-s1e3/master.m3u8', 'qa/episodes/ser-northlight-s1e3/source.mp4', 'ast-qa-ser-northlight-s1e3', 'upj-qa-ser-northlight-s1e3')
    ) AS asset(episode_id, title, duration_sec, playback_relative_path, source_relative_path, asset_id, upload_job_id)
),
seed_owner AS (
    SELECT id AS creator_id
    FROM creator_profiles
    WHERE id = 'cr-saint0xsaint'
    LIMIT 1
)
INSERT INTO media_assets (
    id, creator_id, upload_job_id, upload_id, series_id, kind, title, status,
    visibility, source_relative_path, poster_relative_path, playback_relative_path,
    mime_type, checksum_sha256, container_format, file_size_bytes, duration_sec,
    width, height, frame_rate, video_codec, audio_codec, has_video, has_audio,
    created_at, updated_at, processed_at, published_content_id
)
SELECT
    asset.asset_id,
    seed_owner.creator_id,
    asset.upload_job_id,
    asset.episode_id,
    NULL,
    'episode',
    asset.title,
    'published',
    'public',
    asset.source_relative_path,
    NULL,
    asset.playback_relative_path,
    'application/vnd.apple.mpegurl',
    NULL,
    'hls',
    4566887,
    asset.duration_sec::DOUBLE PRECISION,
    1280,
    720,
    30.0,
    'h264',
    'aac',
    1,
    1,
    '2026-08-24T00:00:00Z',
    '2026-08-24T00:00:00Z',
    '2026-08-24T00:00:00Z',
    asset.episode_id
FROM seeded_episode_assets asset
CROSS JOIN seed_owner
ON CONFLICT (id) DO UPDATE SET
    creator_id = EXCLUDED.creator_id,
    upload_job_id = EXCLUDED.upload_job_id,
    upload_id = EXCLUDED.upload_id,
    kind = EXCLUDED.kind,
    title = EXCLUDED.title,
    status = EXCLUDED.status,
    visibility = EXCLUDED.visibility,
    source_relative_path = EXCLUDED.source_relative_path,
    poster_relative_path = EXCLUDED.poster_relative_path,
    playback_relative_path = EXCLUDED.playback_relative_path,
    mime_type = EXCLUDED.mime_type,
    container_format = EXCLUDED.container_format,
    file_size_bytes = EXCLUDED.file_size_bytes,
    duration_sec = EXCLUDED.duration_sec,
    width = EXCLUDED.width,
    height = EXCLUDED.height,
    frame_rate = EXCLUDED.frame_rate,
    video_codec = EXCLUDED.video_codec,
    audio_codec = EXCLUDED.audio_codec,
    has_video = EXCLUDED.has_video,
    has_audio = EXCLUDED.has_audio,
    updated_at = EXCLUDED.updated_at,
    processed_at = EXCLUDED.processed_at,
    published_content_id = EXCLUDED.published_content_id;

WITH seeded_episode_assets AS (
    SELECT *
    FROM (VALUES
        ('ast-qa-ser-northlight-s1e1', 'qa/episodes/ser-northlight-s1e1/master.m3u8'),
        ('ast-qa-ser-northlight-s1e2', 'qa/episodes/ser-northlight-s1e2/master.m3u8'),
        ('ast-qa-ser-northlight-s1e3', 'qa/episodes/ser-northlight-s1e3/master.m3u8')
    ) AS asset(asset_id, playback_relative_path)
)
INSERT INTO media_asset_variants (
    id, asset_id, variant_type, label, relative_path, mime_type, width, height,
    bitrate_bps, file_size_bytes, is_default, created_at
)
SELECT
    'var-qa-' || asset.asset_id || '-hls',
    asset.asset_id,
    'hls',
    '720p',
    asset.playback_relative_path,
    'application/vnd.apple.mpegurl',
    1280,
    720,
    3000000,
    4566887,
    1,
    '2026-08-24T00:00:00Z'
FROM seeded_episode_assets asset
ON CONFLICT (id) DO UPDATE SET
    variant_type = EXCLUDED.variant_type,
    label = EXCLUDED.label,
    relative_path = EXCLUDED.relative_path,
    mime_type = EXCLUDED.mime_type,
    width = EXCLUDED.width,
    height = EXCLUDED.height,
    bitrate_bps = EXCLUDED.bitrate_bps,
    file_size_bytes = EXCLUDED.file_size_bytes,
    is_default = EXCLUDED.is_default;
