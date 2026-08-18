import json
import sqlite3
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"
DB = "/Users/deepsaint/Desktop/lifestream/backend/lifestream.db"
HOST = "Bearer lifestream-local-dev-token"
ATLAS = "Bearer lifestream-local-collaborator-token"


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
conn.execute(
    """
    UPDATE creator_operational_state
    SET onboarding_status = 'approved',
        identity_status = 'verified',
        tax_status = 'verified',
        payout_status = 'active',
        hold_reasons_json = '[]',
        updated_at = '2026-08-17T17:45:00Z'
    WHERE creator_id = 'crt-deepsaint'
    """
)
conn.execute(
    """
    UPDATE creator_operational_state
    SET onboarding_status = 'in_review',
        identity_status = 'submitted',
        tax_status = 'pending',
        payout_status = 'pending',
        hold_reasons_json = '["tax_profile_missing"]',
        updated_at = '2026-08-17T17:45:00Z'
    WHERE creator_id = 'crt-atlas'
    """
)
conn.commit()
conn.close()


atlas_blocked = req(
    "/api/v1/creator/me/subscriber-tiers",
    "POST",
    ATLAS,
    {
        "tierName": "Atlas Gold",
        "monthlyPrice": 6.99,
        "accentColor": "#ffaa00",
    },
)
assert (
    atlas_blocked[0] == 400
    and "creator is not cleared to manage subscription tiers" in atlas_blocked[1]["error"]
), atlas_blocked

created = req(
    "/api/v1/creator/me/subscriber-tiers",
    "POST",
    HOST,
    {
        "tierName": "Systems Gold",
        "monthlyPrice": 12.99,
        "accentColor": "#11aaee",
    },
)
assert created[0] == 200 and created[1]["status"] == "active", created
tier_id = created[1]["id"]

updated = req(
    f"/api/v1/creator/me/subscriber-tiers/{tier_id}",
    "PATCH",
    HOST,
    {
        "tierName": "Systems Ultra",
        "rank": 2,
        "monthlyPrice": 14.99,
        "accentColor": "#22bbff",
    },
)
assert updated[0] == 200, updated
assert updated[1]["tierName"] == "Systems Ultra", updated
assert updated[1]["monthlyPrice"] == 14.99, updated
assert updated[1]["status"] == "active", updated

tiers = req("/api/v1/creator/me/subscriber-tiers", token=HOST)
assert tiers[0] == 200 and any(item["id"] == tier_id for item in tiers[1]), tiers

uploads = req("/api/v1/creator/me/uploads", token=HOST)
assert uploads[0] == 200, uploads
upload_id = next(
    item["id"]
    for item in uploads[1]
    if item["status"] == "published" and item["accessPolicy"] == "free"
)

paid_upload = req(
    f"/api/v1/creator/me/uploads/{upload_id}",
    "PATCH",
    HOST,
    {
        "accessPolicy": "subscription",
        "accessTierId": tier_id,
        "priceCents": None,
        "currency": None,
        "rentalWindowHours": None,
        "visibility": "public",
    },
)
assert paid_upload[0] == 200 and paid_upload[1]["accessTierId"] == tier_id, paid_upload

retired = req(
    f"/api/v1/creator/me/subscriber-tiers/{tier_id}/retire",
    "POST",
    HOST,
)
assert retired[0] == 200 and retired[1]["status"] == "retired", retired

blocked_sub = req(
    f"/api/v1/creator/subscriptions/crt-deepsaint/tiers/{tier_id}",
    "POST",
    HOST,
)
assert (
    blocked_sub[0] == 400
    and "subscriber tier is not available for new subscriptions" in blocked_sub[1]["error"]
), blocked_sub

blocked_upload = req(
    f"/api/v1/creator/me/uploads/{upload_id}",
    "PATCH",
    HOST,
    {
        "accessPolicy": "subscription_or_purchase",
        "accessTierId": tier_id,
        "priceCents": 1499,
        "currency": "USD",
        "rentalWindowHours": 48,
        "visibility": "public",
    },
)
assert (
    blocked_upload[0] == 400
    and "subscription-based access requires an active subscriber tier" in blocked_upload[1]["error"]
), blocked_upload

print("subscriber-tiers|manage|retire|enforce")
