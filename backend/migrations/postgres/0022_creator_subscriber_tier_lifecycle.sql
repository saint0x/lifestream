ALTER TABLE creator_subscriber_tiers ADD COLUMN status TEXT NOT NULL DEFAULT 'active';
ALTER TABLE creator_subscriber_tiers ADD COLUMN retired_at TEXT;

CREATE INDEX IF NOT EXISTS idx_creator_subscriber_tiers_creator_status_rank
ON creator_subscriber_tiers(creator_id, status, rank ASC);
