ALTER TABLE broadcasts ADD COLUMN creator_id TEXT REFERENCES creator_profiles(id);
ALTER TABLE uploads ADD COLUMN creator_id TEXT REFERENCES creator_profiles(id);
ALTER TABLE analytics_points ADD COLUMN creator_id TEXT REFERENCES creator_profiles(id);
ALTER TABLE traffic_sources ADD COLUMN creator_id TEXT REFERENCES creator_profiles(id);
ALTER TABLE top_content ADD COLUMN creator_id TEXT REFERENCES creator_profiles(id);
ALTER TABLE revenue_entries ADD COLUMN creator_id TEXT REFERENCES creator_profiles(id);
ALTER TABLE creator_notifications ADD COLUMN creator_id TEXT REFERENCES creator_profiles(id);

UPDATE broadcasts SET creator_id = 'crt-deepsaint' WHERE creator_id IS NULL;
UPDATE uploads SET creator_id = 'crt-deepsaint' WHERE creator_id IS NULL;
UPDATE analytics_points SET creator_id = 'crt-deepsaint' WHERE creator_id IS NULL;
UPDATE traffic_sources SET creator_id = 'crt-deepsaint' WHERE creator_id IS NULL;
UPDATE top_content SET creator_id = 'crt-deepsaint' WHERE creator_id IS NULL;
UPDATE revenue_entries SET creator_id = 'crt-deepsaint' WHERE creator_id IS NULL;
UPDATE creator_notifications SET creator_id = 'crt-deepsaint' WHERE creator_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_broadcasts_creator_status_started_at
    ON broadcasts(creator_id, status, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_uploads_creator_status_uploaded_at
    ON uploads(creator_id, status, uploaded_at DESC);
CREATE INDEX IF NOT EXISTS idx_analytics_points_creator_date
    ON analytics_points(creator_id, date ASC);
CREATE INDEX IF NOT EXISTS idx_traffic_sources_creator_source
    ON traffic_sources(creator_id, source);
CREATE INDEX IF NOT EXISTS idx_top_content_creator_views
    ON top_content(creator_id, views DESC);
CREATE INDEX IF NOT EXISTS idx_revenue_entries_creator_date
    ON revenue_entries(creator_id, date DESC);
CREATE INDEX IF NOT EXISTS idx_creator_notifications_creator_sent_at
    ON creator_notifications(creator_id, sent_at DESC);
