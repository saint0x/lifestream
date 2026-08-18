CREATE VIRTUAL TABLE IF NOT EXISTS search_documents USING fts5(
    entity_id UNINDEXED,
    kind UNINDEXED,
    slug UNINDEXED,
    title,
    body,
    tokenize = 'unicode61 remove_diacritics 2'
);

INSERT INTO search_documents (entity_id, kind, slug, title, body)
SELECT
    id,
    'series',
    slug,
    title,
    trim(coalesce(synopsis, '') || ' ' || replace(replace(replace(genres_json, '[', ' '), ']', ' '), '\"', ' '))
FROM series
WHERE id NOT IN (SELECT entity_id FROM search_documents WHERE kind = 'series');

INSERT INTO search_documents (entity_id, kind, slug, title, body)
SELECT
    id,
    'film',
    slug,
    title,
    trim(coalesce(synopsis, '') || ' ' || replace(replace(replace(genres_json, '[', ' '), ']', ' '), '\"', ' '))
FROM films
WHERE id NOT IN (SELECT entity_id FROM search_documents WHERE kind = 'film');

INSERT INTO search_documents (entity_id, kind, slug, title, body)
SELECT
    ls.id,
    'live',
    ls.slug,
    ls.title,
    trim(
        coalesce(ls.category, '') || ' ' ||
        replace(replace(replace(ls.tags_json, '[', ' '), ']', ' '), '\"', ' ') || ' ' ||
        coalesce(s.display_name, '') || ' ' ||
        coalesce(s.handle, '')
    )
FROM live_streams ls
JOIN streamers s ON s.id = ls.streamer_id
WHERE ls.id NOT IN (SELECT entity_id FROM search_documents WHERE kind = 'live');

CREATE TRIGGER IF NOT EXISTS search_documents_series_insert
AFTER INSERT ON series
BEGIN
    INSERT INTO search_documents (entity_id, kind, slug, title, body)
    VALUES (
        NEW.id,
        'series',
        NEW.slug,
        NEW.title,
        trim(coalesce(NEW.synopsis, '') || ' ' || replace(replace(replace(NEW.genres_json, '[', ' '), ']', ' '), '\"', ' '))
    );
END;

CREATE TRIGGER IF NOT EXISTS search_documents_series_update
AFTER UPDATE ON series
BEGIN
    DELETE FROM search_documents WHERE entity_id = OLD.id AND kind = 'series';
    INSERT INTO search_documents (entity_id, kind, slug, title, body)
    VALUES (
        NEW.id,
        'series',
        NEW.slug,
        NEW.title,
        trim(coalesce(NEW.synopsis, '') || ' ' || replace(replace(replace(NEW.genres_json, '[', ' '), ']', ' '), '\"', ' '))
    );
END;

CREATE TRIGGER IF NOT EXISTS search_documents_series_delete
AFTER DELETE ON series
BEGIN
    DELETE FROM search_documents WHERE entity_id = OLD.id AND kind = 'series';
END;

CREATE TRIGGER IF NOT EXISTS search_documents_films_insert
AFTER INSERT ON films
BEGIN
    INSERT INTO search_documents (entity_id, kind, slug, title, body)
    VALUES (
        NEW.id,
        'film',
        NEW.slug,
        NEW.title,
        trim(coalesce(NEW.synopsis, '') || ' ' || replace(replace(replace(NEW.genres_json, '[', ' '), ']', ' '), '\"', ' '))
    );
END;

CREATE TRIGGER IF NOT EXISTS search_documents_films_update
AFTER UPDATE ON films
BEGIN
    DELETE FROM search_documents WHERE entity_id = OLD.id AND kind = 'film';
    INSERT INTO search_documents (entity_id, kind, slug, title, body)
    VALUES (
        NEW.id,
        'film',
        NEW.slug,
        NEW.title,
        trim(coalesce(NEW.synopsis, '') || ' ' || replace(replace(replace(NEW.genres_json, '[', ' '), ']', ' '), '\"', ' '))
    );
END;

CREATE TRIGGER IF NOT EXISTS search_documents_films_delete
AFTER DELETE ON films
BEGIN
    DELETE FROM search_documents WHERE entity_id = OLD.id AND kind = 'film';
END;

CREATE TRIGGER IF NOT EXISTS search_documents_live_streams_insert
AFTER INSERT ON live_streams
BEGIN
    INSERT INTO search_documents (entity_id, kind, slug, title, body)
    SELECT
        NEW.id,
        'live',
        NEW.slug,
        NEW.title,
        trim(
            coalesce(NEW.category, '') || ' ' ||
            replace(replace(replace(NEW.tags_json, '[', ' '), ']', ' '), '\"', ' ') || ' ' ||
            coalesce(s.display_name, '') || ' ' ||
            coalesce(s.handle, '')
        )
    FROM streamers s
    WHERE s.id = NEW.streamer_id;
END;

CREATE TRIGGER IF NOT EXISTS search_documents_live_streams_update
AFTER UPDATE ON live_streams
BEGIN
    DELETE FROM search_documents WHERE entity_id = OLD.id AND kind = 'live';
    INSERT INTO search_documents (entity_id, kind, slug, title, body)
    SELECT
        NEW.id,
        'live',
        NEW.slug,
        NEW.title,
        trim(
            coalesce(NEW.category, '') || ' ' ||
            replace(replace(replace(NEW.tags_json, '[', ' '), ']', ' '), '\"', ' ') || ' ' ||
            coalesce(s.display_name, '') || ' ' ||
            coalesce(s.handle, '')
        )
    FROM streamers s
    WHERE s.id = NEW.streamer_id;
END;

CREATE TRIGGER IF NOT EXISTS search_documents_live_streams_delete
AFTER DELETE ON live_streams
BEGIN
    DELETE FROM search_documents WHERE entity_id = OLD.id AND kind = 'live';
END;

CREATE TRIGGER IF NOT EXISTS search_documents_streamers_update_live
AFTER UPDATE OF handle, display_name ON streamers
BEGIN
    DELETE FROM search_documents
    WHERE kind = 'live'
      AND entity_id IN (SELECT id FROM live_streams WHERE streamer_id = NEW.id);

    INSERT INTO search_documents (entity_id, kind, slug, title, body)
    SELECT
        ls.id,
        'live',
        ls.slug,
        ls.title,
        trim(
            coalesce(ls.category, '') || ' ' ||
            replace(replace(replace(ls.tags_json, '[', ' '), ']', ' '), '\"', ' ') || ' ' ||
            coalesce(NEW.display_name, '') || ' ' ||
            coalesce(NEW.handle, '')
        )
    FROM live_streams ls
    WHERE ls.streamer_id = NEW.id;
END;
