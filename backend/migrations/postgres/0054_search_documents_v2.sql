DROP TRIGGER IF EXISTS search_documents_series_insert ON series;
DROP TRIGGER IF EXISTS search_documents_series_update ON series;
DROP TRIGGER IF EXISTS search_documents_series_delete ON series;
DROP TRIGGER IF EXISTS search_documents_films_insert ON films;
DROP TRIGGER IF EXISTS search_documents_films_update ON films;
DROP TRIGGER IF EXISTS search_documents_films_delete ON films;
DROP TRIGGER IF EXISTS search_documents_live_streams_insert ON live_streams;
DROP TRIGGER IF EXISTS search_documents_live_streams_update ON live_streams;
DROP TRIGGER IF EXISTS search_documents_live_streams_delete ON live_streams;
DROP TRIGGER IF EXISTS search_documents_streamers_update_live ON streamers;

DROP TRIGGER IF EXISTS search_documents_episodes_write ON episodes;
DROP TRIGGER IF EXISTS search_documents_categories_write ON categories;
DROP TRIGGER IF EXISTS search_documents_streamers_write ON streamers;
DROP TRIGGER IF EXISTS search_documents_creator_profiles_write ON creator_profiles;
DROP TRIGGER IF EXISTS search_documents_person_profiles_write ON person_profiles;
DROP TRIGGER IF EXISTS search_documents_person_profile_links_write ON person_profile_links;
DROP TRIGGER IF EXISTS search_documents_content_credits_write ON content_credits;

DROP FUNCTION IF EXISTS refresh_series_search_document();
DROP FUNCTION IF EXISTS refresh_film_search_document();
DROP FUNCTION IF EXISTS refresh_live_search_document();
DROP FUNCTION IF EXISTS refresh_series_search_document_v2();
DROP FUNCTION IF EXISTS refresh_film_search_document_v2();
DROP FUNCTION IF EXISTS refresh_live_search_document_v2();
DROP FUNCTION IF EXISTS refresh_episode_search_document_v2();
DROP FUNCTION IF EXISTS refresh_category_search_document_v2();
DROP FUNCTION IF EXISTS refresh_streamer_search_document_v2();
DROP FUNCTION IF EXISTS refresh_creator_profile_search_document_v2();
DROP FUNCTION IF EXISTS refresh_person_profile_search_document_v2();
DROP FUNCTION IF EXISTS refresh_person_profile_link_search_document_v2();
DROP FUNCTION IF EXISTS refresh_content_credit_search_documents_v2();
DROP FUNCTION IF EXISTS refresh_series_search_document_for_id(TEXT);
DROP FUNCTION IF EXISTS refresh_film_search_document_for_id(TEXT);
DROP FUNCTION IF EXISTS refresh_live_search_document_for_id(TEXT);
DROP FUNCTION IF EXISTS refresh_episode_search_document_for_id(TEXT);
DROP FUNCTION IF EXISTS refresh_category_search_document_for_slug(TEXT);
DROP FUNCTION IF EXISTS refresh_streamer_search_document_for_id(TEXT);
DROP FUNCTION IF EXISTS refresh_creator_profile_search_document_for_id(TEXT);
DROP FUNCTION IF EXISTS refresh_person_profile_search_document_for_id(TEXT);
DROP FUNCTION IF EXISTS search_document_vector(TEXT, TEXT, TEXT, TEXT);

DROP TABLE IF EXISTS search_documents;

CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE TABLE search_documents (
    entity_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    slug TEXT NOT NULL,
    title TEXT NOT NULL,
    subtitle TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL DEFAULT '',
    image TEXT,
    href TEXT NOT NULL,
    metadata_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    rank_boost DOUBLE PRECISION NOT NULL DEFAULT 0,
    popularity DOUBLE PRECISION NOT NULL DEFAULT 0,
    search_vector TSVECTOR NOT NULL,
    updated_at TEXT NOT NULL DEFAULT now()::TEXT,
    PRIMARY KEY (entity_id, kind)
);

CREATE OR REPLACE FUNCTION search_document_vector(
    search_title TEXT,
    search_subtitle TEXT,
    search_body TEXT,
    search_slug TEXT
) RETURNS TSVECTOR AS $$
    SELECT
        setweight(to_tsvector('english', coalesce(search_title, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(search_slug, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(search_subtitle, '')), 'B') ||
        setweight(to_tsvector('english', coalesce(search_body, '')), 'C');
$$ LANGUAGE SQL IMMUTABLE;

CREATE INDEX idx_search_documents_vector
ON search_documents USING GIN (search_vector);

CREATE INDEX idx_search_documents_title_trgm
ON search_documents USING GIN (title gin_trgm_ops);

CREATE INDEX idx_search_documents_slug_trgm
ON search_documents USING GIN (slug gin_trgm_ops);

CREATE INDEX idx_search_documents_kind_popularity
ON search_documents(kind, popularity DESC, rank_boost DESC);

CREATE OR REPLACE FUNCTION refresh_series_search_document_for_id(series_id TEXT)
RETURNS VOID AS $$
DECLARE
    doc RECORD;
    credits_text TEXT;
    body_text TEXT;
    subtitle_text TEXT;
BEGIN
    SELECT * INTO doc FROM series WHERE id = series_id;
    IF NOT FOUND THEN
        DELETE FROM search_documents WHERE entity_id = series_id AND kind = 'series';
        RETURN;
    END IF;

    SELECT string_agg(trim(p.display_name || ' ' || cc.role || ' ' || coalesce(cc.character, '')), ' ' ORDER BY cc.credit_order)
    INTO credits_text
    FROM content_credits cc
    JOIN person_profiles p ON p.id = cc.person_id
    WHERE cc.content_kind = 'series' AND cc.content_id = doc.id;

    subtitle_text := trim(doc.year::TEXT || ' · ' || replace(replace(replace(coalesce(doc.genres_json, ''), '[', ' '), ']', ' '), '"', ' '));
    body_text := trim(coalesce(doc.tagline, '') || ' ' || doc.synopsis || ' ' || coalesce(doc.genres_json, '') || ' ' || coalesce(doc.credits_json, '') || ' ' || coalesce(credits_text, ''));

    INSERT INTO search_documents (
        entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity, search_vector, updated_at
    ) VALUES (
        doc.id,
        'series',
        doc.slug,
        doc.title,
        subtitle_text,
        body_text,
        coalesce((doc.images_json::jsonb)->>'thumbnail', (doc.images_json::jsonb)->>'poster'),
        '/series/' || doc.slug,
        jsonb_build_object('year', doc.year, 'rating', doc.rating, 'genres', doc.genres_json::jsonb, 'status', doc.status, 'totalEpisodes', doc.total_episodes),
        45 + CASE WHEN doc.is_original <> 0 THEN 12 ELSE 0 END + CASE WHEN doc.trending <> 0 THEN 18 ELSE 0 END,
        doc.score,
        search_document_vector(doc.title, subtitle_text, body_text, doc.slug),
        now()::TEXT
    )
    ON CONFLICT (entity_id, kind) DO UPDATE
    SET slug = EXCLUDED.slug,
        title = EXCLUDED.title,
        subtitle = EXCLUDED.subtitle,
        body = EXCLUDED.body,
        image = EXCLUDED.image,
        href = EXCLUDED.href,
        metadata_json = EXCLUDED.metadata_json,
        rank_boost = EXCLUDED.rank_boost,
        popularity = EXCLUDED.popularity,
        search_vector = EXCLUDED.search_vector,
        updated_at = EXCLUDED.updated_at;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION refresh_film_search_document_for_id(film_id TEXT)
RETURNS VOID AS $$
DECLARE
    doc RECORD;
    credits_text TEXT;
    body_text TEXT;
    subtitle_text TEXT;
BEGIN
    SELECT * INTO doc FROM films WHERE id = film_id;
    IF NOT FOUND THEN
        DELETE FROM search_documents WHERE entity_id = film_id AND kind = 'film';
        RETURN;
    END IF;

    SELECT string_agg(trim(p.display_name || ' ' || cc.role || ' ' || coalesce(cc.character, '')), ' ' ORDER BY cc.credit_order)
    INTO credits_text
    FROM content_credits cc
    JOIN person_profiles p ON p.id = cc.person_id
    WHERE cc.content_kind = 'film' AND cc.content_id = doc.id;

    subtitle_text := trim(doc.year::TEXT || ' · ' || replace(replace(replace(coalesce(doc.genres_json, ''), '[', ' '), ']', ' '), '"', ' '));
    body_text := trim(coalesce(doc.tagline, '') || ' ' || doc.synopsis || ' ' || coalesce(doc.genres_json, '') || ' ' || coalesce(doc.credits_json, '') || ' ' || coalesce(credits_text, ''));

    INSERT INTO search_documents (
        entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity, search_vector, updated_at
    ) VALUES (
        doc.id,
        'film',
        doc.slug,
        doc.title,
        subtitle_text,
        body_text,
        coalesce((doc.images_json::jsonb)->>'thumbnail', (doc.images_json::jsonb)->>'poster'),
        '/film/' || doc.slug,
        jsonb_build_object('year', doc.year, 'rating', doc.rating, 'genres', doc.genres_json::jsonb, 'durationSec', doc.duration_sec),
        38 + CASE WHEN doc.is_original <> 0 THEN 10 ELSE 0 END + CASE WHEN doc.trending <> 0 THEN 16 ELSE 0 END,
        doc.score,
        search_document_vector(doc.title, subtitle_text, body_text, doc.slug),
        now()::TEXT
    )
    ON CONFLICT (entity_id, kind) DO UPDATE
    SET slug = EXCLUDED.slug,
        title = EXCLUDED.title,
        subtitle = EXCLUDED.subtitle,
        body = EXCLUDED.body,
        image = EXCLUDED.image,
        href = EXCLUDED.href,
        metadata_json = EXCLUDED.metadata_json,
        rank_boost = EXCLUDED.rank_boost,
        popularity = EXCLUDED.popularity,
        search_vector = EXCLUDED.search_vector,
        updated_at = EXCLUDED.updated_at;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION refresh_episode_search_document_for_id(episode_id TEXT)
RETURNS VOID AS $$
DECLARE
    doc RECORD;
    parent RECORD;
    body_text TEXT;
    subtitle_text TEXT;
BEGIN
    SELECT * INTO doc FROM episodes WHERE id = episode_id;
    IF NOT FOUND THEN
        DELETE FROM search_documents WHERE entity_id = episode_id AND kind = 'episode';
        RETURN;
    END IF;

    SELECT * INTO parent FROM series WHERE id = doc.series_id;
    IF NOT FOUND THEN
        DELETE FROM search_documents WHERE entity_id = episode_id AND kind = 'episode';
        RETURN;
    END IF;

    subtitle_text := parent.title || ' · S' || doc.season_number || ' E' || doc.episode_number;
    body_text := trim(doc.synopsis || ' ' || parent.title || ' ' || coalesce(parent.tagline, '') || ' ' || parent.synopsis || ' ' || parent.genres_json || ' ' || parent.credits_json);

    INSERT INTO search_documents (
        entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity, search_vector, updated_at
    ) VALUES (
        doc.id,
        'episode',
        doc.id,
        doc.title,
        subtitle_text,
        body_text,
        doc.thumbnail,
        '/series/' || parent.slug,
        jsonb_build_object('seriesId', parent.id, 'seriesSlug', parent.slug, 'seriesTitle', parent.title, 'seasonNumber', doc.season_number, 'episodeNumber', doc.episode_number, 'durationSec', doc.duration_sec),
        16,
        parent.score,
        search_document_vector(doc.title, subtitle_text, body_text, doc.id),
        now()::TEXT
    )
    ON CONFLICT (entity_id, kind) DO UPDATE
    SET slug = EXCLUDED.slug,
        title = EXCLUDED.title,
        subtitle = EXCLUDED.subtitle,
        body = EXCLUDED.body,
        image = EXCLUDED.image,
        href = EXCLUDED.href,
        metadata_json = EXCLUDED.metadata_json,
        rank_boost = EXCLUDED.rank_boost,
        popularity = EXCLUDED.popularity,
        search_vector = EXCLUDED.search_vector,
        updated_at = EXCLUDED.updated_at;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION refresh_live_search_document_for_id(live_id TEXT)
RETURNS VOID AS $$
DECLARE
    doc RECORD;
    streamer RECORD;
    body_text TEXT;
    subtitle_text TEXT;
BEGIN
    SELECT * INTO doc FROM live_streams WHERE id = live_id;
    IF NOT FOUND THEN
        DELETE FROM search_documents WHERE entity_id = live_id AND kind = 'live';
        RETURN;
    END IF;

    SELECT * INTO streamer FROM streamers WHERE id = doc.streamer_id;
    IF NOT FOUND THEN
        DELETE FROM search_documents WHERE entity_id = live_id AND kind = 'live';
        RETURN;
    END IF;

    subtitle_text := streamer.display_name || ' · Live · ' || doc.category;
    body_text := trim(doc.category || ' ' || doc.tags_json || ' ' || streamer.display_name || ' ' || streamer.handle || ' ' || streamer.bio || ' ' || doc.language);

    INSERT INTO search_documents (
        entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity, search_vector, updated_at
    ) VALUES (
        doc.id,
        'live',
        doc.slug,
        doc.title,
        subtitle_text,
        body_text,
        doc.thumbnail,
        '/live/' || doc.slug,
        jsonb_build_object('category', doc.category, 'tags', doc.tags_json::jsonb, 'streamerId', streamer.id, 'streamerHandle', streamer.handle, 'streamerDisplayName', streamer.display_name, 'viewers', doc.viewers, 'language', doc.language),
        70 + CASE WHEN streamer.is_partner <> 0 THEN 12 ELSE 0 END,
        doc.viewers,
        search_document_vector(doc.title, subtitle_text, body_text, doc.slug),
        now()::TEXT
    )
    ON CONFLICT (entity_id, kind) DO UPDATE
    SET slug = EXCLUDED.slug,
        title = EXCLUDED.title,
        subtitle = EXCLUDED.subtitle,
        body = EXCLUDED.body,
        image = EXCLUDED.image,
        href = EXCLUDED.href,
        metadata_json = EXCLUDED.metadata_json,
        rank_boost = EXCLUDED.rank_boost,
        popularity = EXCLUDED.popularity,
        search_vector = EXCLUDED.search_vector,
        updated_at = EXCLUDED.updated_at;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION refresh_streamer_search_document_for_id(streamer_id TEXT)
RETURNS VOID AS $$
DECLARE
    doc RECORD;
    body_text TEXT;
    subtitle_text TEXT;
BEGIN
    SELECT * INTO doc FROM streamers WHERE id = streamer_id;
    IF NOT FOUND THEN
        DELETE FROM search_documents WHERE entity_id = streamer_id AND kind = 'creator';
        RETURN;
    END IF;

    IF EXISTS (SELECT 1 FROM creator_profiles WHERE creator_profiles.handle = doc.handle) THEN
        DELETE FROM search_documents WHERE entity_id = streamer_id AND kind = 'creator';
        PERFORM refresh_live_search_document_for_id(id) FROM live_streams WHERE live_streams.streamer_id = doc.id;
        RETURN;
    END IF;

    subtitle_text := CASE WHEN doc.is_live <> 0 THEN 'Live creator' ELSE 'Creator' END;
    body_text := trim(doc.handle || ' ' || doc.display_name || ' ' || doc.bio);

    INSERT INTO search_documents (
        entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity, search_vector, updated_at
    ) VALUES (
        doc.id,
        'creator',
        doc.handle,
        doc.display_name,
        subtitle_text,
        body_text,
        doc.avatar,
        '/@' || doc.handle,
        jsonb_build_object('handle', doc.handle, 'followers', doc.followers, 'isPartner', doc.is_partner <> 0, 'isLive', doc.is_live <> 0),
        30 + CASE WHEN doc.is_partner <> 0 THEN 12 ELSE 0 END + CASE WHEN doc.is_live <> 0 THEN 20 ELSE 0 END,
        doc.followers,
        search_document_vector(doc.display_name, subtitle_text, body_text, doc.handle),
        now()::TEXT
    )
    ON CONFLICT (entity_id, kind) DO UPDATE
    SET slug = EXCLUDED.slug,
        title = EXCLUDED.title,
        subtitle = EXCLUDED.subtitle,
        body = EXCLUDED.body,
        image = EXCLUDED.image,
        href = EXCLUDED.href,
        metadata_json = EXCLUDED.metadata_json,
        rank_boost = EXCLUDED.rank_boost,
        popularity = EXCLUDED.popularity,
        search_vector = EXCLUDED.search_vector,
        updated_at = EXCLUDED.updated_at;

    PERFORM refresh_live_search_document_for_id(id) FROM live_streams WHERE live_streams.streamer_id = doc.id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION refresh_creator_profile_search_document_for_id(creator_id TEXT)
RETURNS VOID AS $$
DECLARE
    doc RECORD;
    body_text TEXT;
BEGIN
    SELECT * INTO doc FROM creator_profiles WHERE id = creator_id;
    IF NOT FOUND THEN
        DELETE FROM search_documents WHERE entity_id = creator_id AND kind = 'creator';
        RETURN;
    END IF;

    body_text := trim(doc.handle || ' ' || doc.display_name || ' ' || doc.tagline || ' ' || doc.bio || ' ' || doc.default_category || ' ' || doc.default_tags_json || ' ' || doc.partner_status);

    INSERT INTO search_documents (
        entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity, search_vector, updated_at
    ) VALUES (
        doc.id,
        'creator',
        doc.handle,
        doc.display_name,
        'Creator Studio',
        body_text,
        doc.avatar,
        '/@' || doc.handle,
        jsonb_build_object('handle', doc.handle, 'partnerStatus', doc.partner_status, 'defaultCategory', doc.default_category, 'tags', doc.default_tags_json::jsonb, 'followers', doc.followers, 'subscribers', doc.subscribers),
        34 + CASE WHEN doc.partner_status = 'partner' THEN 16 ELSE 0 END,
        doc.monthly_viewers + doc.followers + doc.subscribers,
        search_document_vector(doc.display_name, 'Creator Studio', body_text, doc.handle),
        now()::TEXT
    )
    ON CONFLICT (entity_id, kind) DO UPDATE
    SET slug = EXCLUDED.slug,
        title = EXCLUDED.title,
        subtitle = EXCLUDED.subtitle,
        body = EXCLUDED.body,
        image = EXCLUDED.image,
        href = EXCLUDED.href,
        metadata_json = EXCLUDED.metadata_json,
        rank_boost = EXCLUDED.rank_boost,
        popularity = EXCLUDED.popularity,
        search_vector = EXCLUDED.search_vector,
        updated_at = EXCLUDED.updated_at;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION refresh_person_profile_search_document_for_id(person_id TEXT)
RETURNS VOID AS $$
DECLARE
    doc RECORD;
    credits_text TEXT;
    links_text TEXT;
    body_text TEXT;
BEGIN
    SELECT * INTO doc FROM person_profiles WHERE id = person_id;
    IF NOT FOUND THEN
        DELETE FROM search_documents WHERE entity_id = person_id AND kind = 'profile';
        RETURN;
    END IF;

    SELECT string_agg(trim(cc.role || ' ' || coalesce(cc.character, '') || ' ' || coalesce(s.title, f.title, '')), ' ' ORDER BY cc.credit_order)
    INTO credits_text
    FROM content_credits cc
    LEFT JOIN series s ON cc.content_kind = 'series' AND s.id = cc.content_id
    LEFT JOIN films f ON cc.content_kind = 'film' AND f.id = cc.content_id
    WHERE cc.person_id = doc.id;

    SELECT string_agg(trim(label || ' ' || platform || ' ' || url), ' ' ORDER BY position)
    INTO links_text
    FROM person_profile_links
    WHERE person_profile_links.person_id = doc.id;

    body_text := trim(doc.slug || ' ' || doc.display_name || ' ' || doc.headline || ' ' || doc.location || ' ' || doc.about || ' ' || doc.known_for_json || ' ' || coalesce(doc.website_url, '') || ' ' || coalesce(doc.instagram_url, '') || ' ' || coalesce(doc.x_url, '') || ' ' || coalesce(doc.imdb_url, '') || ' ' || coalesce(doc.linkedin_url, '') || ' ' || coalesce(doc.facebook_url, '') || ' ' || coalesce(links_text, '') || ' ' || coalesce(credits_text, ''));

    INSERT INTO search_documents (
        entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity, search_vector, updated_at
    ) VALUES (
        doc.id,
        'profile',
        doc.slug,
        doc.display_name,
        doc.headline,
        body_text,
        doc.avatar,
        '/profile/' || doc.slug,
        jsonb_build_object('knownFor', doc.known_for_json::jsonb, 'location', doc.location),
        28,
        coalesce((SELECT count(*)::DOUBLE PRECISION * 15 FROM content_credits WHERE content_credits.person_id = doc.id), 0),
        search_document_vector(doc.display_name, doc.headline, body_text, doc.slug),
        now()::TEXT
    )
    ON CONFLICT (entity_id, kind) DO UPDATE
    SET slug = EXCLUDED.slug,
        title = EXCLUDED.title,
        subtitle = EXCLUDED.subtitle,
        body = EXCLUDED.body,
        image = EXCLUDED.image,
        href = EXCLUDED.href,
        metadata_json = EXCLUDED.metadata_json,
        rank_boost = EXCLUDED.rank_boost,
        popularity = EXCLUDED.popularity,
        search_vector = EXCLUDED.search_vector,
        updated_at = EXCLUDED.updated_at;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION refresh_category_search_document_for_slug(category_slug TEXT)
RETURNS VOID AS $$
DECLARE
    doc RECORD;
    body_text TEXT;
BEGIN
    SELECT * INTO doc FROM categories WHERE slug = category_slug;
    IF NOT FOUND THEN
        DELETE FROM search_documents WHERE entity_id = category_slug AND kind = 'category';
        RETURN;
    END IF;

    body_text := trim(doc.name || ' ' || doc.slug || ' ' || doc.tags_json);

    INSERT INTO search_documents (
        entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity, search_vector, updated_at
    ) VALUES (
        doc.slug,
        'category',
        doc.slug,
        doc.name,
        'Category',
        body_text,
        doc.cover_image,
        '/catalog?genre=' || replace(doc.name, ' ', '+'),
        jsonb_build_object('liveViewers', doc.live_viewers, 'liveChannels', doc.live_channels, 'tags', doc.tags_json::jsonb),
        18,
        doc.live_viewers,
        search_document_vector(doc.name, 'Category', body_text, doc.slug),
        now()::TEXT
    )
    ON CONFLICT (entity_id, kind) DO UPDATE
    SET slug = EXCLUDED.slug,
        title = EXCLUDED.title,
        subtitle = EXCLUDED.subtitle,
        body = EXCLUDED.body,
        image = EXCLUDED.image,
        href = EXCLUDED.href,
        metadata_json = EXCLUDED.metadata_json,
        rank_boost = EXCLUDED.rank_boost,
        popularity = EXCLUDED.popularity,
        search_vector = EXCLUDED.search_vector,
        updated_at = EXCLUDED.updated_at;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION refresh_series_search_document_v2()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM refresh_series_search_document_for_id(OLD.id);
        DELETE FROM search_documents WHERE kind = 'episode' AND metadata_json->>'seriesId' = OLD.id;
        RETURN OLD;
    END IF;
    PERFORM refresh_series_search_document_for_id(NEW.id);
    PERFORM refresh_episode_search_document_for_id(id) FROM episodes WHERE series_id = NEW.id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION refresh_film_search_document_v2()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM refresh_film_search_document_for_id(OLD.id);
        RETURN OLD;
    END IF;
    PERFORM refresh_film_search_document_for_id(NEW.id);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION refresh_episode_search_document_v2()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM refresh_episode_search_document_for_id(OLD.id);
        RETURN OLD;
    END IF;
    PERFORM refresh_episode_search_document_for_id(NEW.id);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION refresh_live_search_document_v2()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM refresh_live_search_document_for_id(OLD.id);
        RETURN OLD;
    END IF;
    PERFORM refresh_live_search_document_for_id(NEW.id);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION refresh_streamer_search_document_v2()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM refresh_streamer_search_document_for_id(OLD.id);
        DELETE FROM search_documents WHERE kind = 'live' AND metadata_json->>'streamerId' = OLD.id;
        RETURN OLD;
    END IF;
    PERFORM refresh_streamer_search_document_for_id(NEW.id);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION refresh_creator_profile_search_document_v2()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM refresh_creator_profile_search_document_for_id(OLD.id);
        RETURN OLD;
    END IF;
    PERFORM refresh_creator_profile_search_document_for_id(NEW.id);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION refresh_person_profile_search_document_v2()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM refresh_person_profile_search_document_for_id(OLD.id);
        RETURN OLD;
    END IF;
    PERFORM refresh_person_profile_search_document_for_id(NEW.id);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION refresh_person_profile_link_search_document_v2()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM refresh_person_profile_search_document_for_id(OLD.person_id);
        RETURN OLD;
    END IF;
    PERFORM refresh_person_profile_search_document_for_id(NEW.person_id);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION refresh_category_search_document_v2()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM refresh_category_search_document_for_slug(OLD.slug);
        RETURN OLD;
    END IF;
    PERFORM refresh_category_search_document_for_slug(NEW.slug);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION refresh_content_credit_search_documents_v2()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' OR TG_OP = 'UPDATE' THEN
        IF OLD.content_kind = 'series' THEN
            PERFORM refresh_series_search_document_for_id(OLD.content_id);
            PERFORM refresh_episode_search_document_for_id(id) FROM episodes WHERE series_id = OLD.content_id;
        ELSIF OLD.content_kind = 'film' THEN
            PERFORM refresh_film_search_document_for_id(OLD.content_id);
        END IF;
        PERFORM refresh_person_profile_search_document_for_id(OLD.person_id);
    END IF;

    IF TG_OP = 'INSERT' OR TG_OP = 'UPDATE' THEN
        IF NEW.content_kind = 'series' THEN
            PERFORM refresh_series_search_document_for_id(NEW.content_id);
            PERFORM refresh_episode_search_document_for_id(id) FROM episodes WHERE series_id = NEW.content_id;
        ELSIF NEW.content_kind = 'film' THEN
            PERFORM refresh_film_search_document_for_id(NEW.content_id);
        END IF;
        PERFORM refresh_person_profile_search_document_for_id(NEW.person_id);
        RETURN NEW;
    END IF;

    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER search_documents_series_insert
AFTER INSERT ON series
FOR EACH ROW EXECUTE FUNCTION refresh_series_search_document_v2();

CREATE TRIGGER search_documents_series_update
AFTER UPDATE ON series
FOR EACH ROW EXECUTE FUNCTION refresh_series_search_document_v2();

CREATE TRIGGER search_documents_series_delete
AFTER DELETE ON series
FOR EACH ROW EXECUTE FUNCTION refresh_series_search_document_v2();

CREATE TRIGGER search_documents_films_insert
AFTER INSERT ON films
FOR EACH ROW EXECUTE FUNCTION refresh_film_search_document_v2();

CREATE TRIGGER search_documents_films_update
AFTER UPDATE ON films
FOR EACH ROW EXECUTE FUNCTION refresh_film_search_document_v2();

CREATE TRIGGER search_documents_films_delete
AFTER DELETE ON films
FOR EACH ROW EXECUTE FUNCTION refresh_film_search_document_v2();

CREATE TRIGGER search_documents_episodes_write
AFTER INSERT OR UPDATE OR DELETE ON episodes
FOR EACH ROW EXECUTE FUNCTION refresh_episode_search_document_v2();

CREATE TRIGGER search_documents_live_streams_insert
AFTER INSERT ON live_streams
FOR EACH ROW EXECUTE FUNCTION refresh_live_search_document_v2();

CREATE TRIGGER search_documents_live_streams_update
AFTER UPDATE ON live_streams
FOR EACH ROW EXECUTE FUNCTION refresh_live_search_document_v2();

CREATE TRIGGER search_documents_live_streams_delete
AFTER DELETE ON live_streams
FOR EACH ROW EXECUTE FUNCTION refresh_live_search_document_v2();

CREATE TRIGGER search_documents_streamers_write
AFTER INSERT OR UPDATE OR DELETE ON streamers
FOR EACH ROW EXECUTE FUNCTION refresh_streamer_search_document_v2();

CREATE TRIGGER search_documents_creator_profiles_write
AFTER INSERT OR UPDATE OR DELETE ON creator_profiles
FOR EACH ROW EXECUTE FUNCTION refresh_creator_profile_search_document_v2();

CREATE TRIGGER search_documents_person_profiles_write
AFTER INSERT OR UPDATE OR DELETE ON person_profiles
FOR EACH ROW EXECUTE FUNCTION refresh_person_profile_search_document_v2();

CREATE TRIGGER search_documents_person_profile_links_write
AFTER INSERT OR UPDATE OR DELETE ON person_profile_links
FOR EACH ROW EXECUTE FUNCTION refresh_person_profile_link_search_document_v2();

CREATE TRIGGER search_documents_content_credits_write
AFTER INSERT OR UPDATE OR DELETE ON content_credits
FOR EACH ROW EXECUTE FUNCTION refresh_content_credit_search_documents_v2();

CREATE TRIGGER search_documents_categories_write
AFTER INSERT OR UPDATE OR DELETE ON categories
FOR EACH ROW EXECUTE FUNCTION refresh_category_search_document_v2();

SELECT refresh_series_search_document_for_id(id) FROM series;
SELECT refresh_film_search_document_for_id(id) FROM films;
SELECT refresh_episode_search_document_for_id(id) FROM episodes;
SELECT refresh_live_search_document_for_id(id) FROM live_streams;
SELECT refresh_streamer_search_document_for_id(id) FROM streamers;
SELECT refresh_creator_profile_search_document_for_id(id) FROM creator_profiles;
SELECT refresh_person_profile_search_document_for_id(id) FROM person_profiles;
SELECT refresh_category_search_document_for_slug(slug) FROM categories;
