CREATE TABLE IF NOT EXISTS creator_operational_state (
    creator_id TEXT PRIMARY KEY,
    legal_name TEXT NOT NULL,
    support_email TEXT NOT NULL,
    business_type TEXT NOT NULL,
    payout_country TEXT NOT NULL,
    payout_provider TEXT NOT NULL,
    onboarding_status TEXT NOT NULL,
    identity_status TEXT NOT NULL,
    tax_status TEXT NOT NULL,
    payout_status TEXT NOT NULL,
    hold_reasons_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_reviewed_at TEXT,
    FOREIGN KEY (creator_id) REFERENCES creator_profiles(id) ON DELETE CASCADE
);
