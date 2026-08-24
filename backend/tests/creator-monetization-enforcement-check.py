import datetime
import hashlib
import json
import sqlite3
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"
DB = "/Users/deepsaint/Desktop/vanta/backend/vanta.db"
CREATOR = "Bearer vanta-local-dev-token"
VIEWER = "Bearer vanta-viewer-token"


def req(path, method="GET", token=None, body=None):
    headers = {}
    if token:
        headers["Authorization"] = token
    data = None
    if body is not None:
        data = json.dumps(body).encode()
        headers["Content-Type"] = "application/json"
    request = urllib.request.Request(BASE + path, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(request) as response:
            raw = response.read().decode()
            return response.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode()
        return exc.code, json.loads(raw) if raw else None


conn = sqlite3.connect(DB)
now = datetime.datetime.now(datetime.timezone.utc).isoformat()
conn.execute(
    """
    INSERT OR IGNORE INTO users (id, handle, display_name, avatar, tier, joined_at)
    VALUES (?, ?, ?, ?, ?, ?)
    """,
    (
        "usr-viewer",
        "viewer_one",
        "Viewer One",
        "https://cdn.vanta.local/avatar/viewer-one.jpg",
        "free",
        now,
    ),
)
conn.execute(
    """
    INSERT OR REPLACE INTO auth_sessions (
        id, user_id, label, token_hash, scopes_json, created_at, expires_at, revoked_at, last_used_at
    ) VALUES (?, ?, ?, ?, ?, ?, NULL, NULL, NULL)
    """,
    (
        "sess-viewer-local",
        "usr-viewer",
        "local-viewer",
        hashlib.sha256("vanta-viewer-token".encode()).hexdigest(),
        json.dumps(["user"]),
        now,
    ),
)
conn.execute("DELETE FROM content_purchases WHERE user_id = ?", ("usr-viewer",))
conn.execute("DELETE FROM creator_memberships WHERE user_id = ?", ("usr-viewer",))
conn.execute(
    """
    UPDATE creator_operational_state
    SET onboarding_status = 'approved',
        identity_status = 'verified',
        tax_status = 'verified',
        payout_status = 'active',
        hold_reasons_json = '[]',
        updated_at = ?,
        last_reviewed_at = ?
    WHERE creator_id = 'crt-deepsaint'
    """,
    (now, now),
)
conn.commit()
conn.close()

uploads = req("/api/v1/creator/me/uploads", token=CREATOR)
assert uploads[0] == 200, uploads
upload_id = next(item["id"] for item in uploads[1] if item["status"] == "published")

jobs = req("/api/v1/creator/me/upload-jobs", token=CREATOR)
assert jobs[0] == 200, jobs
job_id = next(item["id"] for item in jobs[1] if item["status"] == "published")

ready_upload = req(
    f"/api/v1/creator/me/uploads/{upload_id}",
    "PATCH",
    CREATOR,
    {
        "accessPolicy": "purchase",
        "priceCents": 1499,
        "currency": "USD",
        "rentalWindowHours": 48,
        "accessTierId": None,
        "visibility": "public",
    },
)
assert ready_upload[0] == 200 and ready_upload[1]["accessPolicy"] == "purchase", ready_upload

conn = sqlite3.connect(DB)
conn.execute(
    """
    UPDATE creator_operational_state
    SET identity_status = 'submitted',
        tax_status = 'pending',
        payout_status = 'pending',
        hold_reasons_json = '["compliance_review"]',
        updated_at = '2026-08-17T17:40:00Z'
    WHERE creator_id = 'crt-deepsaint'
    """
)
conn.commit()
conn.close()

blocked_upload = req(
    f"/api/v1/creator/me/uploads/{upload_id}",
    "PATCH",
    CREATOR,
    {
        "accessPolicy": "subscription_or_purchase",
        "accessTierId": "tier-2",
        "priceCents": 1499,
        "currency": "USD",
        "rentalWindowHours": 48,
        "visibility": "public",
    },
)
assert (
    blocked_upload[0] == 400
    and "creator is not cleared to publish paid content" in blocked_upload[1]["error"]
), blocked_upload

blocked_publish = req(
    f"/api/v1/creator/me/upload-jobs/{job_id}/publish",
    "POST",
    CREATOR,
    {
        "description": "blocked paid publish",
        "visibility": "public",
        "accessPolicy": "purchase",
        "priceCents": 1599,
        "currency": "USD",
        "rentalWindowHours": 48,
    },
)
assert (
    blocked_publish[0] == 400
    and "creator is not cleared to publish paid content" in blocked_publish[1]["error"]
), blocked_publish

blocked_purchase = req(f"/api/v1/uploads/{upload_id}/purchase", "POST", VIEWER)
assert (
    blocked_purchase[0] == 400
    and "creator is not cleared to accept paid transactions" in blocked_purchase[1]["error"]
), blocked_purchase

blocked_subscription = req("/api/v1/creator/subscriptions/crt-deepsaint/tiers/tier-2", "POST", VIEWER)
assert (
    blocked_subscription[0] == 400
    and "creator is not cleared to accept paid transactions" in blocked_subscription[1]["error"]
), blocked_subscription

conn = sqlite3.connect(DB)
conn.execute(
    """
    UPDATE creator_operational_state
    SET onboarding_status = 'approved',
        identity_status = 'verified',
        tax_status = 'verified',
        payout_status = 'active',
        hold_reasons_json = '[]',
        updated_at = '2026-08-17T17:41:00Z',
        last_reviewed_at = '2026-08-17T17:41:00Z'
    WHERE creator_id = 'crt-deepsaint'
    """
)
conn.commit()
conn.close()

restored_publish = req(
    f"/api/v1/creator/me/upload-jobs/{job_id}/publish",
    "POST",
    CREATOR,
    {
        "description": "restored paid publish",
        "visibility": "public",
        "accessPolicy": "subscription_or_purchase",
        "accessTierId": "tier-2",
        "priceCents": 1599,
        "currency": "USD",
        "rentalWindowHours": 48,
    },
)
assert restored_publish[0] == 200, restored_publish

restored_purchase = req(f"/api/v1/uploads/{upload_id}/purchase", "POST", VIEWER)
assert restored_purchase[0] == 200 and restored_purchase[1]["status"] == "active", restored_purchase

restored_subscription = req("/api/v1/creator/subscriptions/crt-deepsaint/tiers/tier-2", "POST", VIEWER)
assert restored_subscription[0] == 200 and restored_subscription[1]["status"] == "active", restored_subscription

print("creator-monetization|blocked|restored")
