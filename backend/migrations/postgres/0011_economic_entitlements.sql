ALTER TABLE creator_subscriber_tiers ADD COLUMN rank INTEGER NOT NULL DEFAULT 1;

ALTER TABLE uploads ADD COLUMN access_policy TEXT NOT NULL DEFAULT 'free';
ALTER TABLE uploads ADD COLUMN access_tier_id TEXT REFERENCES creator_subscriber_tiers(id);
ALTER TABLE uploads ADD COLUMN price_cents INTEGER;
ALTER TABLE uploads ADD COLUMN currency TEXT;
ALTER TABLE uploads ADD COLUMN rental_window_hours INTEGER;

CREATE TABLE IF NOT EXISTS creator_memberships (
    user_id TEXT NOT NULL,
    creator_id TEXT NOT NULL,
    tier_id TEXT NOT NULL,
    status TEXT NOT NULL,
    started_at TEXT NOT NULL,
    renews_at TEXT,
    ends_at TEXT,
    canceled_at TEXT,
    PRIMARY KEY (user_id, creator_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (tier_id) REFERENCES creator_subscriber_tiers(id) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS content_purchases (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    creator_id TEXT NOT NULL,
    upload_id TEXT NOT NULL,
    access_policy TEXT NOT NULL,
    amount_cents INTEGER NOT NULL,
    currency TEXT NOT NULL,
    status TEXT NOT NULL,
    purchased_at TEXT NOT NULL,
    expires_at TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (upload_id) REFERENCES uploads(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_creator_subscriber_tiers_creator_rank
ON creator_subscriber_tiers(creator_id, rank ASC);

CREATE INDEX IF NOT EXISTS idx_uploads_creator_access_policy
ON uploads(creator_id, access_policy, access_tier_id, status, release_at DESC);

CREATE INDEX IF NOT EXISTS idx_creator_memberships_creator_status
ON creator_memberships(creator_id, status, renews_at DESC);

CREATE INDEX IF NOT EXISTS idx_creator_memberships_user_status
ON creator_memberships(user_id, status, renews_at DESC);

CREATE INDEX IF NOT EXISTS idx_content_purchases_user_upload_status
ON content_purchases(user_id, upload_id, status, expires_at DESC);

CREATE INDEX IF NOT EXISTS idx_content_purchases_creator_status
ON content_purchases(creator_id, status, purchased_at DESC);
