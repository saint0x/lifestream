ALTER TABLE notification_deliveries
ADD COLUMN delivered_at TEXT;

ALTER TABLE notification_deliveries
ADD COLUMN last_attempted_at TEXT;

ALTER TABLE notification_deliveries
ADD COLUMN next_attempt_at TEXT;

CREATE INDEX IF NOT EXISTS idx_notification_deliveries_state_attempt
ON notification_deliveries(state, next_attempt_at, sent_at DESC);
