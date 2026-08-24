DROP TRIGGER IF EXISTS search_documents_series_insert;
DROP TRIGGER IF EXISTS search_documents_series_update;
DROP TRIGGER IF EXISTS search_documents_series_delete;
DROP TRIGGER IF EXISTS search_documents_films_insert;
DROP TRIGGER IF EXISTS search_documents_films_update;
DROP TRIGGER IF EXISTS search_documents_films_delete;
DROP TRIGGER IF EXISTS search_documents_live_streams_insert;
DROP TRIGGER IF EXISTS search_documents_live_streams_update;
DROP TRIGGER IF EXISTS search_documents_live_streams_delete;
DROP TRIGGER IF EXISTS search_documents_streamers_update_live;
DROP TRIGGER IF EXISTS search_documents_episodes_insert;
DROP TRIGGER IF EXISTS search_documents_episodes_update;
DROP TRIGGER IF EXISTS search_documents_episodes_delete;
DROP TRIGGER IF EXISTS search_documents_categories_insert;
DROP TRIGGER IF EXISTS search_documents_categories_update;
DROP TRIGGER IF EXISTS search_documents_categories_delete;
DROP TRIGGER IF EXISTS search_documents_streamers_insert;
DROP TRIGGER IF EXISTS search_documents_streamers_update;
DROP TRIGGER IF EXISTS search_documents_streamers_delete;
DROP TRIGGER IF EXISTS search_documents_person_profiles_insert;
DROP TRIGGER IF EXISTS search_documents_person_profiles_update;
DROP TRIGGER IF EXISTS search_documents_person_profiles_delete;
DROP TRIGGER IF EXISTS search_documents_creator_profiles_insert;
DROP TRIGGER IF EXISTS search_documents_creator_profiles_update;
DROP TRIGGER IF EXISTS search_documents_creator_profiles_delete;

DROP TABLE IF EXISTS search_documents;

CREATE VIRTUAL TABLE search_documents USING fts5(
    entity_id UNINDEXED,
    kind UNINDEXED,
    slug UNINDEXED,
    title,
    subtitle,
    body,
    image UNINDEXED,
    href UNINDEXED,
    metadata_json UNINDEXED,
    rank_boost UNINDEXED,
    popularity UNINDEXED,
    tokenize = 'unicode61 remove_diacritics 2'
);

INSERT INTO search_documents (entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity)
SELECT
    id,
    'series',
    slug,
    title,
    trim(CAST(year AS TEXT) || ' · ' || replace(replace(replace(genres_json, '[', ' '), ']', ' '), '"', ' ')),
    trim(coalesce(tagline, '') || ' ' || synopsis || ' ' || genres_json || ' ' || credits_json),
    coalesce(json_extract(images_json, '$.thumbnail'), json_extract(images_json, '$.poster'), ''),
    '/series/' || slug,
    '{}',
    45 + CASE WHEN is_original <> 0 THEN 12 ELSE 0 END + CASE WHEN trending <> 0 THEN 18 ELSE 0 END,
    score
FROM series;

INSERT INTO search_documents (entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity)
SELECT
    id,
    'film',
    slug,
    title,
    trim(CAST(year AS TEXT) || ' · ' || replace(replace(replace(genres_json, '[', ' '), ']', ' '), '"', ' ')),
    trim(coalesce(tagline, '') || ' ' || synopsis || ' ' || genres_json || ' ' || credits_json),
    coalesce(json_extract(images_json, '$.thumbnail'), json_extract(images_json, '$.poster'), ''),
    '/film/' || slug,
    '{}',
    38 + CASE WHEN is_original <> 0 THEN 10 ELSE 0 END + CASE WHEN trending <> 0 THEN 16 ELSE 0 END,
    score
FROM films;

INSERT INTO search_documents (entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity)
SELECT
    e.id,
    'episode',
    e.id,
    e.title,
    s.title || ' · S' || e.season_number || ' E' || e.episode_number,
    trim(e.synopsis || ' ' || s.title || ' ' || coalesce(s.tagline, '') || ' ' || s.synopsis || ' ' || s.genres_json || ' ' || s.credits_json),
    e.thumbnail,
    '/series/' || s.slug,
    '{}',
    16,
    s.score
FROM episodes e
JOIN series s ON s.id = e.series_id;

INSERT INTO search_documents (entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity)
SELECT
    ls.id,
    'live',
    ls.slug,
    ls.title,
    s.display_name || ' · Live · ' || ls.category,
    trim(ls.category || ' ' || ls.tags_json || ' ' || s.display_name || ' ' || s.handle || ' ' || s.bio || ' ' || ls.language),
    ls.thumbnail,
    '/live/' || ls.slug,
    '{}',
    70 + CASE WHEN s.is_partner <> 0 THEN 12 ELSE 0 END,
    ls.viewers
FROM live_streams ls
JOIN streamers s ON s.id = ls.streamer_id;

INSERT INTO search_documents (entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity)
SELECT
    id,
    'creator',
    handle,
    display_name,
    CASE WHEN is_live <> 0 THEN 'Live creator' ELSE 'Creator' END,
    trim(handle || ' ' || display_name || ' ' || bio),
    avatar,
    '/@' || handle,
    '{}',
    30 + CASE WHEN is_partner <> 0 THEN 12 ELSE 0 END + CASE WHEN is_live <> 0 THEN 20 ELSE 0 END,
    followers
FROM streamers;
DELETE FROM search_documents
WHERE kind = 'creator'
  AND entity_id IN (
      SELECT s.id
      FROM streamers s
      JOIN creator_profiles cp ON cp.handle = s.handle
  );

INSERT INTO search_documents (entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity)
SELECT
    id,
    'creator',
    handle,
    display_name,
    'Creator Studio',
    trim(handle || ' ' || display_name || ' ' || tagline || ' ' || bio || ' ' || default_category || ' ' || default_tags_json || ' ' || partner_status),
    avatar,
    '/@' || handle,
    '{}',
    34 + CASE WHEN partner_status = 'partner' THEN 16 ELSE 0 END,
    monthly_viewers + followers + subscribers
FROM creator_profiles;

INSERT INTO search_documents (entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity)
SELECT
    id,
    'profile',
    slug,
    display_name,
    headline,
    trim(slug || ' ' || display_name || ' ' || headline || ' ' || location || ' ' || about || ' ' || known_for_json || ' ' || coalesce(website_url, '') || ' ' || coalesce(instagram_url, '') || ' ' || coalesce(x_url, '') || ' ' || coalesce(imdb_url, '') || ' ' || coalesce(linkedin_url, '') || ' ' || coalesce(facebook_url, '')),
    avatar,
    '/profile/' || slug,
    '{}',
    28,
    coalesce((SELECT count(*) * 15 FROM content_credits WHERE content_credits.person_id = person_profiles.id), 0)
FROM person_profiles;

INSERT INTO search_documents (entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity)
SELECT
    slug,
    'category',
    slug,
    name,
    'Category',
    trim(name || ' ' || slug || ' ' || tags_json),
    cover_image,
    '/catalog?genre=' || replace(name, ' ', '+'),
    '{}',
    18,
    live_viewers
FROM categories;

CREATE TRIGGER search_documents_series_insert
AFTER INSERT ON series
BEGIN
    INSERT INTO search_documents (entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity)
    VALUES (NEW.id, 'series', NEW.slug, NEW.title, trim(CAST(NEW.year AS TEXT) || ' · ' || replace(replace(replace(NEW.genres_json, '[', ' '), ']', ' '), '"', ' ')), trim(coalesce(NEW.tagline, '') || ' ' || NEW.synopsis || ' ' || NEW.genres_json || ' ' || NEW.credits_json), coalesce(json_extract(NEW.images_json, '$.thumbnail'), json_extract(NEW.images_json, '$.poster'), ''), '/series/' || NEW.slug, '{}', 45 + CASE WHEN NEW.is_original <> 0 THEN 12 ELSE 0 END + CASE WHEN NEW.trending <> 0 THEN 18 ELSE 0 END, NEW.score);
END;

CREATE TRIGGER search_documents_series_update
AFTER UPDATE ON series
BEGIN
    DELETE FROM search_documents WHERE entity_id = OLD.id AND kind = 'series';
    INSERT INTO search_documents (entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity)
    VALUES (NEW.id, 'series', NEW.slug, NEW.title, trim(CAST(NEW.year AS TEXT) || ' · ' || replace(replace(replace(NEW.genres_json, '[', ' '), ']', ' '), '"', ' ')), trim(coalesce(NEW.tagline, '') || ' ' || NEW.synopsis || ' ' || NEW.genres_json || ' ' || NEW.credits_json), coalesce(json_extract(NEW.images_json, '$.thumbnail'), json_extract(NEW.images_json, '$.poster'), ''), '/series/' || NEW.slug, '{}', 45 + CASE WHEN NEW.is_original <> 0 THEN 12 ELSE 0 END + CASE WHEN NEW.trending <> 0 THEN 18 ELSE 0 END, NEW.score);
END;

CREATE TRIGGER search_documents_series_delete
AFTER DELETE ON series
BEGIN
    DELETE FROM search_documents WHERE entity_id = OLD.id AND kind = 'series';
    DELETE FROM search_documents WHERE kind = 'episode' AND href = '/series/' || OLD.slug;
END;

CREATE TRIGGER search_documents_films_insert
AFTER INSERT ON films
BEGIN
    INSERT INTO search_documents (entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity)
    VALUES (NEW.id, 'film', NEW.slug, NEW.title, trim(CAST(NEW.year AS TEXT) || ' · ' || replace(replace(replace(NEW.genres_json, '[', ' '), ']', ' '), '"', ' ')), trim(coalesce(NEW.tagline, '') || ' ' || NEW.synopsis || ' ' || NEW.genres_json || ' ' || NEW.credits_json), coalesce(json_extract(NEW.images_json, '$.thumbnail'), json_extract(NEW.images_json, '$.poster'), ''), '/film/' || NEW.slug, '{}', 38 + CASE WHEN NEW.is_original <> 0 THEN 10 ELSE 0 END + CASE WHEN NEW.trending <> 0 THEN 16 ELSE 0 END, NEW.score);
END;

CREATE TRIGGER search_documents_films_update
AFTER UPDATE ON films
BEGIN
    DELETE FROM search_documents WHERE entity_id = OLD.id AND kind = 'film';
    INSERT INTO search_documents (entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity)
    VALUES (NEW.id, 'film', NEW.slug, NEW.title, trim(CAST(NEW.year AS TEXT) || ' · ' || replace(replace(replace(NEW.genres_json, '[', ' '), ']', ' '), '"', ' ')), trim(coalesce(NEW.tagline, '') || ' ' || NEW.synopsis || ' ' || NEW.genres_json || ' ' || NEW.credits_json), coalesce(json_extract(NEW.images_json, '$.thumbnail'), json_extract(NEW.images_json, '$.poster'), ''), '/film/' || NEW.slug, '{}', 38 + CASE WHEN NEW.is_original <> 0 THEN 10 ELSE 0 END + CASE WHEN NEW.trending <> 0 THEN 16 ELSE 0 END, NEW.score);
END;

CREATE TRIGGER search_documents_films_delete
AFTER DELETE ON films
BEGIN
    DELETE FROM search_documents WHERE entity_id = OLD.id AND kind = 'film';
END;
