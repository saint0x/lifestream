CREATE TABLE IF NOT EXISTS person_profiles (
    id TEXT PRIMARY KEY,
    user_id TEXT UNIQUE REFERENCES users(id) ON DELETE SET NULL,
    slug TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    avatar TEXT NOT NULL,
    hero_image TEXT NOT NULL,
    headline TEXT NOT NULL,
    location TEXT NOT NULL,
    about TEXT NOT NULL,
    known_for_json TEXT NOT NULL,
    website_url TEXT,
    instagram_url TEXT,
    x_url TEXT,
    imdb_url TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS content_credits (
    id TEXT PRIMARY KEY,
    person_id TEXT NOT NULL REFERENCES person_profiles(id) ON DELETE CASCADE,
    content_id TEXT NOT NULL,
    content_kind TEXT NOT NULL,
    role TEXT NOT NULL,
    character TEXT,
    credit_order INTEGER NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_content_credits_content
ON content_credits(content_kind, content_id, credit_order ASC);

CREATE INDEX IF NOT EXISTS idx_content_credits_person
ON content_credits(person_id, credit_order ASC);

INSERT INTO person_profiles (
    id, user_id, slug, display_name, avatar, hero_image, headline, location, about,
    known_for_json, website_url, instagram_url, x_url, imdb_url, created_at, updated_at
) VALUES
('per-mara-vale', NULL, 'mara-vale', 'Mara Vale', 'https://images.unsplash.com/photo-1494790108377-be9c29b29330?auto=format&fit=crop&w=320&q=80', 'https://images.unsplash.com/photo-1500534314209-a25ddb2bd429?auto=format&fit=crop&w=1600&q=80', 'Creator and systems storyteller', 'Reykjavik / Los Angeles', 'Mara develops cinematic technology stories about infrastructure, climate, and invisible networks.', '["Creator","Writer","Systems storytelling"]', NULL, NULL, NULL, NULL, '2026-08-24T00:00:00Z', '2026-08-24T00:00:00Z'),
('per-ilya-ren', NULL, 'ilya-ren', 'Ilya Ren', 'https://images.unsplash.com/photo-1500648767791-00dcc994a43e?auto=format&fit=crop&w=320&q=80', 'https://images.unsplash.com/photo-1519681393784-d120267933ba?auto=format&fit=crop&w=1600&q=80', 'Director of atmospheric science fiction', 'Toronto', 'Ilya directs grounded genre work with practical environments, quiet tension, and precise production design.', '["Director","Science Fiction","Visual tone"]', NULL, NULL, NULL, NULL, '2026-08-24T00:00:00Z', '2026-08-24T00:00:00Z'),
('per-noor-frame', NULL, 'noor-frame', 'Noor Frame', 'https://images.unsplash.com/photo-1534528741775-53994a69daeb?auto=format&fit=crop&w=320&q=80', 'https://images.unsplash.com/photo-1492691527719-9d1e07e534b4?auto=format&fit=crop&w=1600&q=80', 'Documentary director and field producer', 'Oakland', 'Noor builds documentaries around fieldwork, editorial transparency, and the moments that almost miss the final cut.', '["Director","Documentary","Field production"]', NULL, NULL, NULL, NULL, '2026-08-24T00:00:00Z', '2026-08-24T00:00:00Z'),
('per-kai-signal', NULL, 'kai-signal', 'Kai Signal', 'https://images.unsplash.com/photo-1517841905240-472988babdf9?auto=format&fit=crop&w=320&q=80', 'https://images.unsplash.com/photo-1511379938547-c1f69419868d?auto=format&fit=crop&w=1600&q=80', 'Composer, host, and live mix designer', 'New York', 'Kai scores live sessions and builds production workflows for music-led films, streams, and performance systems.', '["Composer","Host","Live production"]', NULL, NULL, NULL, NULL, '2026-08-24T00:00:00Z', '2026-08-24T00:00:00Z'),
('per-aria-labs', NULL, 'aria-labs', 'Aria Labs', 'https://images.unsplash.com/photo-1550745165-9bc0b252726f?auto=format&fit=crop&w=320&q=80', 'https://images.unsplash.com/photo-1518709268805-4e9042af2176?auto=format&fit=crop&w=1600&q=80', 'Production technology studio', 'Remote', 'Aria Labs produces realtime tools, cinematic streams, and experimental infrastructure for VANTA originals.', '["Studio","Realtime tools","Production"]', NULL, NULL, NULL, NULL, '2026-08-24T00:00:00Z', '2026-08-24T00:00:00Z')
ON CONFLICT (id) DO UPDATE
SET slug = EXCLUDED.slug,
    display_name = EXCLUDED.display_name,
    avatar = EXCLUDED.avatar,
    hero_image = EXCLUDED.hero_image,
    headline = EXCLUDED.headline,
    location = EXCLUDED.location,
    about = EXCLUDED.about,
    known_for_json = EXCLUDED.known_for_json,
    updated_at = EXCLUDED.updated_at;

INSERT INTO content_credits (id, person_id, content_id, content_kind, role, character, credit_order, created_at) VALUES
('cc-northlight-mara', 'per-mara-vale', 'ser-northlight', 'series', 'Creator', NULL, 1, '2026-08-24T00:00:00Z'),
('cc-northlight-ilya', 'per-ilya-ren', 'ser-northlight', 'series', 'Director', NULL, 2, '2026-08-24T00:00:00Z'),
('cc-cutline-noor', 'per-noor-frame', 'ser-cutline', 'series', 'Creator', NULL, 1, '2026-08-24T00:00:00Z'),
('cc-signal-kai', 'per-kai-signal', 'ser-signal-room', 'series', 'Host', NULL, 1, '2026-08-24T00:00:00Z'),
('cc-signal-aria', 'per-aria-labs', 'ser-signal-room', 'series', 'Production', NULL, 2, '2026-08-24T00:00:00Z'),
('cc-ghost-mara', 'per-mara-vale', 'film-ghost-standard', 'film', 'Writer', NULL, 1, '2026-08-24T00:00:00Z'),
('cc-ghost-aria', 'per-aria-labs', 'film-ghost-standard', 'film', 'Studio', NULL, 2, '2026-08-24T00:00:00Z'),
('cc-after-noor', 'per-noor-frame', 'film-after-the-cut', 'film', 'Director', NULL, 1, '2026-08-24T00:00:00Z'),
('cc-night-kai', 'per-kai-signal', 'film-night-mix', 'film', 'Composer', NULL, 1, '2026-08-24T00:00:00Z')
ON CONFLICT (id) DO UPDATE
SET person_id = EXCLUDED.person_id,
    content_id = EXCLUDED.content_id,
    content_kind = EXCLUDED.content_kind,
    role = EXCLUDED.role,
    character = EXCLUDED.character,
    credit_order = EXCLUDED.credit_order;

UPDATE series
SET credits_json = (
    SELECT json_agg(json_build_object(
        'id', cc.id,
        'personId', p.id,
        'personSlug', p.slug,
        'name', p.display_name,
        'role', cc.role,
        'character', cc.character,
        'avatar', p.avatar
    ) ORDER BY cc.credit_order)::TEXT
    FROM content_credits cc
    JOIN person_profiles p ON p.id = cc.person_id
    WHERE cc.content_kind = 'series' AND cc.content_id = series.id
)
WHERE EXISTS (
    SELECT 1 FROM content_credits cc WHERE cc.content_kind = 'series' AND cc.content_id = series.id
);

UPDATE films
SET credits_json = (
    SELECT json_agg(json_build_object(
        'id', cc.id,
        'personId', p.id,
        'personSlug', p.slug,
        'name', p.display_name,
        'role', cc.role,
        'character', cc.character,
        'avatar', p.avatar
    ) ORDER BY cc.credit_order)::TEXT
    FROM content_credits cc
    JOIN person_profiles p ON p.id = cc.person_id
    WHERE cc.content_kind = 'film' AND cc.content_id = films.id
)
WHERE EXISTS (
    SELECT 1 FROM content_credits cc WHERE cc.content_kind = 'film' AND cc.content_id = films.id
);
