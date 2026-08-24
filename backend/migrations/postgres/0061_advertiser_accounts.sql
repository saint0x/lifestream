CREATE TABLE IF NOT EXISTS advertiser_users (
    id TEXT PRIMARY KEY,
    auth_user_id TEXT UNIQUE,
    email TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS advertiser_memberships (
    advertiser_id TEXT NOT NULL REFERENCES ad_marketplace_advertisers(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES advertiser_users(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    permissions_json TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (advertiser_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_advertiser_memberships_user_status
    ON advertiser_memberships(user_id, status);

CREATE TABLE IF NOT EXISTS advertiser_invites (
    id TEXT PRIMARY KEY,
    advertiser_id TEXT NOT NULL REFERENCES ad_marketplace_advertisers(id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    role TEXT NOT NULL,
    permissions_json TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    invited_by_user_id TEXT NOT NULL REFERENCES advertiser_users(id) ON DELETE RESTRICT,
    token_hash TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    accepted_at TEXT,
    revoked_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_advertiser_invites_advertiser_status
    ON advertiser_invites(advertiser_id, status, created_at DESC);

CREATE TABLE IF NOT EXISTS advertiser_billing_profiles (
    advertiser_id TEXT PRIMARY KEY REFERENCES ad_marketplace_advertisers(id) ON DELETE CASCADE,
    billing_name TEXT NOT NULL,
    billing_email TEXT NOT NULL,
    payment_provider_key TEXT NOT NULL DEFAULT 'manual_invoice',
    external_customer_id TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

INSERT INTO advertiser_users (
    id, auth_user_id, email, name, status, created_at, updated_at
) VALUES (
    'adv-user-northstar-admin',
    'user-demo-advertiser-admin',
    'maya@northstarsupply.example',
    'Maya Chen',
    'active',
    '2026-01-01T00:00:00Z',
    '2026-01-01T00:00:00Z'
) ON CONFLICT (id) DO NOTHING;

INSERT INTO advertiser_memberships (
    advertiser_id, user_id, role, permissions_json, status, created_at, updated_at
) VALUES (
    'adv-vanta-seed-devtools',
    'adv-user-northstar-admin',
    'admin',
    '["manage_account","manage_team","manage_billing","buy_media","approve_work","view_reports"]',
    'active',
    '2026-01-01T00:00:00Z',
    '2026-01-01T00:00:00Z'
) ON CONFLICT (advertiser_id, user_id) DO NOTHING;

INSERT INTO advertiser_billing_profiles (
    advertiser_id, billing_name, billing_email, payment_provider_key, external_customer_id,
    status, created_at, updated_at
) VALUES (
    'adv-vanta-seed-devtools',
    'Northstar Supply Co.',
    'billing@northstarsupply.example',
    'manual_invoice',
    'cus_northstar_seed',
    'active',
    '2026-01-01T00:00:00Z',
    '2026-01-01T00:00:00Z'
) ON CONFLICT (advertiser_id) DO NOTHING;
