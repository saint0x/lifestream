ALTER TABLE person_profiles ADD COLUMN IF NOT EXISTS linkedin_url TEXT;
ALTER TABLE person_profiles ADD COLUMN IF NOT EXISTS facebook_url TEXT;

CREATE TABLE IF NOT EXISTS person_profile_links (
    id TEXT PRIMARY KEY,
    person_id TEXT NOT NULL REFERENCES person_profiles(id) ON DELETE CASCADE,
    platform TEXT NOT NULL,
    label TEXT NOT NULL,
    url TEXT NOT NULL,
    position BIGINT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(person_id, position)
);

CREATE INDEX IF NOT EXISTS idx_person_profile_links_person_position
ON person_profile_links(person_id, position ASC);
