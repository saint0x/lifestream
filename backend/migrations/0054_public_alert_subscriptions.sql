CREATE TABLE IF NOT EXISTS public_alert_subscriptions (
    id TEXT PRIMARY KEY,
    target_kind TEXT NOT NULL,
    target_id TEXT NOT NULL,
    target_slug TEXT,
    target_title TEXT NOT NULL,
    visitor_id TEXT,
    user_id TEXT,
    contact_channel TEXT NOT NULL,
    contact_value TEXT NOT NULL,
    social_platform TEXT,
    alert_types_json TEXT NOT NULL,
    source_path TEXT,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_confirmed_at TEXT,
    UNIQUE(target_kind, target_id, contact_channel, contact_value)
);

CREATE INDEX IF NOT EXISTS idx_public_alert_subscriptions_target
ON public_alert_subscriptions(target_kind, target_id, status);

CREATE INDEX IF NOT EXISTS idx_public_alert_subscriptions_contact
ON public_alert_subscriptions(contact_channel, contact_value, status);

CREATE INDEX IF NOT EXISTS idx_public_alert_subscriptions_user
ON public_alert_subscriptions(user_id, updated_at DESC)
WHERE user_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_public_alert_subscriptions_visitor
ON public_alert_subscriptions(visitor_id, updated_at DESC)
WHERE visitor_id IS NOT NULL;
