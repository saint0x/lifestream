CREATE TABLE IF NOT EXISTS ad_marketplace_advertisers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    industry TEXT NOT NULL,
    website_url TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS ad_marketplace_inventory_packages (
    id TEXT PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    placement_kind TEXT NOT NULL,
    spot_length_seconds INTEGER,
    deliverables_json TEXT NOT NULL,
    base_price_cents INTEGER NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS ad_marketplace_campaigns (
    id TEXT PRIMARY KEY,
    advertiser_id TEXT NOT NULL REFERENCES ad_marketplace_advertisers(id) ON DELETE RESTRICT,
    name TEXT NOT NULL,
    objective TEXT NOT NULL,
    starts_at TEXT,
    ends_at TEXT,
    budget_cents INTEGER NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    status TEXT NOT NULL DEFAULT 'planning',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS ad_marketplace_offers (
    id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL REFERENCES ad_marketplace_campaigns(id) ON DELETE CASCADE,
    package_id TEXT NOT NULL REFERENCES ad_marketplace_inventory_packages(id) ON DELETE RESTRICT,
    creator_id TEXT NOT NULL REFERENCES creator_profiles(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    brief TEXT NOT NULL,
    requirements_json TEXT NOT NULL,
    offer_amount_cents INTEGER NOT NULL,
    creator_payout_cents INTEGER NOT NULL,
    platform_fee_cents INTEGER NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    status TEXT NOT NULL DEFAULT 'pending',
    advertiser_review_status TEXT NOT NULL DEFAULT 'not_submitted',
    due_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    accepted_at TEXT,
    declined_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_ad_marketplace_offers_creator_status
    ON ad_marketplace_offers(creator_id, status, updated_at);

CREATE TABLE IF NOT EXISTS ad_marketplace_submissions (
    id TEXT PRIMARY KEY,
    offer_id TEXT NOT NULL REFERENCES ad_marketplace_offers(id) ON DELETE CASCADE,
    creator_id TEXT NOT NULL REFERENCES creator_profiles(id) ON DELETE CASCADE,
    submission_url TEXT NOT NULL,
    notes TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'review_pending',
    submitted_at TEXT NOT NULL,
    reviewed_at TEXT,
    advertiser_feedback TEXT,
    revision_due_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_ad_marketplace_submissions_offer_submitted
    ON ad_marketplace_submissions(offer_id, submitted_at DESC);

CREATE TABLE IF NOT EXISTS ad_marketplace_payment_providers (
    provider_key TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 0,
    mode TEXT NOT NULL DEFAULT 'test',
    status TEXT NOT NULL DEFAULT 'configured_pending',
    config_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

INSERT OR IGNORE INTO ad_marketplace_advertisers (
    id, name, industry, website_url, status, created_at, updated_at
) VALUES (
    'adv-vanta-seed-devtools',
    'Northstar DevTools',
    'Developer tools',
    'https://example.com',
    'active',
    '2026-01-01T00:00:00Z',
    '2026-01-01T00:00:00Z'
);

INSERT OR IGNORE INTO ad_marketplace_inventory_packages (
    id, code, title, description, placement_kind, spot_length_seconds, deliverables_json,
    base_price_cents, currency, status, created_at, updated_at
) VALUES
    (
        'pkg-creator-read-10',
        'creator_read_10',
        '10-second creator read',
        'Short creator-read placement inside a long-form episode or live segment.',
        'creator_read',
        10,
        '["Creator-read script", "In-episode placement", "Qualified-viewer report"]',
        125000,
        'USD',
        'active',
        '2026-01-01T00:00:00Z',
        '2026-01-01T00:00:00Z'
    ),
    (
        'pkg-creator-read-30',
        'creator_read_30',
        '30-second creator read',
        'Standard creator-read spot for premium episodic inventory.',
        'creator_read',
        30,
        '["Creator-read script", "Episode integration", "Post-campaign attention report"]',
        350000,
        'USD',
        'active',
        '2026-01-01T00:00:00Z',
        '2026-01-01T00:00:00Z'
    ),
    (
        'pkg-integrated-sponsor',
        'integrated_sponsor',
        'Integrated sponsorship',
        'Deeper creator integration with branded talking points and campaign recap.',
        'integrated_sponsorship',
        60,
        '["Creator integration", "Brand talking points", "Usage proof", "Campaign recap"]',
        950000,
        'USD',
        'active',
        '2026-01-01T00:00:00Z',
        '2026-01-01T00:00:00Z'
    ),
    (
        'pkg-site-display',
        'site_display',
        'VANTA display placement',
        'On-platform display inventory adjacent to premium creator content.',
        'site_display',
        NULL,
        '["Display creative", "Flight window", "Impression and attention reporting"]',
        250000,
        'USD',
        'active',
        '2026-01-01T00:00:00Z',
        '2026-01-01T00:00:00Z'
    );

INSERT OR IGNORE INTO ad_marketplace_campaigns (
    id, advertiser_id, name, objective, starts_at, ends_at, budget_cents, currency,
    status, created_at, updated_at
) VALUES (
    'camp-vanta-seed-devtools-launch',
    'adv-vanta-seed-devtools',
    'Developer creator launch',
    'Reach qualified developer audiences watching premium long-form builds.',
    '2026-09-01T00:00:00Z',
    '2026-10-01T00:00:00Z',
    2500000,
    'USD',
    'booking',
    '2026-01-01T00:00:00Z',
    '2026-01-01T00:00:00Z'
);

INSERT OR IGNORE INTO ad_marketplace_payment_providers (
    provider_key, display_name, enabled, mode, status, config_json, created_at, updated_at
) VALUES (
    'whop',
    'Whop',
    0,
    'test',
    'configured_pending',
    '{"role":"creator-ad-marketplace-payments","swappable":true}',
    '2026-01-01T00:00:00Z',
    '2026-01-01T00:00:00Z'
);

INSERT OR IGNORE INTO ad_marketplace_offers (
    id, campaign_id, package_id, creator_id, title, brief, requirements_json,
    offer_amount_cents, creator_payout_cents, platform_fee_cents, currency, status,
    advertiser_review_status, due_at, created_at, updated_at
)
SELECT
    'offer-seed-' || cp.id || '-devtools-30',
    'camp-vanta-seed-devtools-launch',
    'pkg-creator-read-30',
    cp.id,
    '30-second developer tools read',
    'Northstar DevTools wants a mid-roll creator read for a long-form build episode or live recap.',
    '["Mention local workflow speed", "Show product for at least 5 seconds", "Submit rough cut before publishing"]',
    350000,
    280000,
    70000,
    'USD',
    'pending',
    'not_submitted',
    '2026-09-15T00:00:00Z',
    '2026-01-01T00:00:00Z',
    '2026-01-01T00:00:00Z'
FROM creator_profiles cp
WHERE cp.user_id NOT LIKE 'guest-%'
LIMIT 8;
