CREATE TABLE IF NOT EXISTS landing_signups (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    audience TEXT NOT NULL,
    name TEXT NOT NULL,
    email TEXT NOT NULL,
    company TEXT,
    website TEXT,
    budget TEXT,
    message TEXT,
    source_path TEXT,
    status TEXT NOT NULL DEFAULT 'new',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_landing_signups_kind_status_created
    ON landing_signups(kind, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_landing_signups_email_created
    ON landing_signups(email, created_at DESC);
