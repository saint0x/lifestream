CREATE TABLE IF NOT EXISTS search_documents (
    entity_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    slug TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (entity_id, kind)
);

CREATE INDEX IF NOT EXISTS idx_search_documents_text
ON search_documents
USING GIN (to_tsvector('simple', title || ' ' || body || ' ' || slug));

INSERT INTO search_documents (entity_id, kind, slug, title, body)
SELECT
    id,
    'series',
    slug,
    title,
    trim(coalesce(synopsis, '') || ' ' || replace(replace(replace(genres_json, '[', ' '), ']', ' '), '"', ' '))
FROM series
ON CONFLICT (entity_id, kind) DO NOTHING;

INSERT INTO search_documents (entity_id, kind, slug, title, body)
SELECT
    id,
    'film',
    slug,
    title,
    trim(coalesce(synopsis, '') || ' ' || replace(replace(replace(genres_json, '[', ' '), ']', ' '), '"', ' '))
FROM films
ON CONFLICT (entity_id, kind) DO NOTHING;

INSERT INTO search_documents (entity_id, kind, slug, title, body)
SELECT
    ls.id,
    'live',
    ls.slug,
    ls.title,
    trim(
        coalesce(ls.category, '') || ' ' ||
        replace(replace(replace(ls.tags_json, '[', ' '), ']', ' '), '"', ' ') || ' ' ||
        coalesce(s.display_name, '') || ' ' ||
        coalesce(s.handle, '')
    )
FROM live_streams ls
JOIN streamers s ON s.id = ls.streamer_id
ON CONFLICT (entity_id, kind) DO NOTHING;

CREATE OR REPLACE FUNCTION refresh_series_search_document()
RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        DELETE FROM search_documents WHERE entity_id = OLD.id AND kind = 'series';
        RETURN OLD;
    END IF;

    INSERT INTO search_documents (entity_id, kind, slug, title, body)
    VALUES (
        NEW.id,
        'series',
        NEW.slug,
        NEW.title,
        trim(coalesce(NEW.synopsis, '') || ' ' || replace(replace(replace(NEW.genres_json, '[', ' '), ']', ' '), '"', ' '))
    )
    ON CONFLICT (entity_id, kind) DO UPDATE
    SET slug = EXCLUDED.slug,
        title = EXCLUDED.title,
        body = EXCLUDED.body;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION refresh_film_search_document()
RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        DELETE FROM search_documents WHERE entity_id = OLD.id AND kind = 'film';
        RETURN OLD;
    END IF;

    INSERT INTO search_documents (entity_id, kind, slug, title, body)
    VALUES (
        NEW.id,
        'film',
        NEW.slug,
        NEW.title,
        trim(coalesce(NEW.synopsis, '') || ' ' || replace(replace(replace(NEW.genres_json, '[', ' '), ']', ' '), '"', ' '))
    )
    ON CONFLICT (entity_id, kind) DO UPDATE
    SET slug = EXCLUDED.slug,
        title = EXCLUDED.title,
        body = EXCLUDED.body;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION refresh_live_search_document()
RETURNS trigger AS $$
DECLARE
    streamer_display_name TEXT;
    streamer_handle TEXT;
BEGIN
    IF TG_OP = 'DELETE' THEN
        DELETE FROM search_documents WHERE entity_id = OLD.id AND kind = 'live';
        RETURN OLD;
    END IF;

    SELECT display_name, handle
    INTO streamer_display_name, streamer_handle
    FROM streamers
    WHERE id = NEW.streamer_id;

    INSERT INTO search_documents (entity_id, kind, slug, title, body)
    VALUES (
        NEW.id,
        'live',
        NEW.slug,
        NEW.title,
        trim(
            coalesce(NEW.category, '') || ' ' ||
            replace(replace(replace(NEW.tags_json, '[', ' '), ']', ' '), '"', ' ') || ' ' ||
            coalesce(streamer_display_name, '') || ' ' ||
            coalesce(streamer_handle, '')
        )
    )
    ON CONFLICT (entity_id, kind) DO UPDATE
    SET slug = EXCLUDED.slug,
        title = EXCLUDED.title,
        body = EXCLUDED.body;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER search_documents_series_insert
AFTER INSERT ON series
FOR EACH ROW EXECUTE FUNCTION refresh_series_search_document();

CREATE TRIGGER search_documents_series_update
AFTER UPDATE ON series
FOR EACH ROW EXECUTE FUNCTION refresh_series_search_document();

CREATE TRIGGER search_documents_series_delete
AFTER DELETE ON series
FOR EACH ROW EXECUTE FUNCTION refresh_series_search_document();

CREATE TRIGGER search_documents_films_insert
AFTER INSERT ON films
FOR EACH ROW EXECUTE FUNCTION refresh_film_search_document();

CREATE TRIGGER search_documents_films_update
AFTER UPDATE ON films
FOR EACH ROW EXECUTE FUNCTION refresh_film_search_document();

CREATE TRIGGER search_documents_films_delete
AFTER DELETE ON films
FOR EACH ROW EXECUTE FUNCTION refresh_film_search_document();

CREATE TRIGGER search_documents_live_streams_insert
AFTER INSERT ON live_streams
FOR EACH ROW EXECUTE FUNCTION refresh_live_search_document();

CREATE TRIGGER search_documents_live_streams_update
AFTER UPDATE ON live_streams
FOR EACH ROW EXECUTE FUNCTION refresh_live_search_document();

CREATE TRIGGER search_documents_live_streams_delete
AFTER DELETE ON live_streams
FOR EACH ROW EXECUTE FUNCTION refresh_live_search_document();
