CREATE TABLE IF NOT EXISTS notification_events (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    body TEXT NOT NULL,
    actor_user_id TEXT,
    actor_label TEXT,
    creator_id TEXT,
    stream_id TEXT,
    amount REAL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (actor_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE SET NULL,
    FOREIGN KEY (stream_id) REFERENCES live_streams(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS notification_deliveries (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,
    recipient_user_id TEXT,
    recipient_creator_id TEXT,
    channel TEXT NOT NULL,
    state TEXT NOT NULL,
    sent_at TEXT NOT NULL,
    read_at TEXT,
    failed_at TEXT,
    last_error TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (event_id) REFERENCES notification_events(id) ON DELETE CASCADE,
    FOREIGN KEY (recipient_user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (recipient_creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE,
    CHECK (recipient_user_id IS NOT NULL OR recipient_creator_id IS NOT NULL)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_notification_deliveries_event_user_channel
ON notification_deliveries(event_id, recipient_user_id, channel)
WHERE recipient_user_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_notification_deliveries_event_creator_channel
ON notification_deliveries(event_id, recipient_creator_id, channel)
WHERE recipient_creator_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_notification_deliveries_user_sent
ON notification_deliveries(recipient_user_id, sent_at DESC);

CREATE INDEX IF NOT EXISTS idx_notification_deliveries_creator_sent
ON notification_deliveries(recipient_creator_id, sent_at DESC);

CREATE INDEX IF NOT EXISTS idx_notification_events_kind_created
ON notification_events(kind, created_at DESC);
