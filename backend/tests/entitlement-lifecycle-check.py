import datetime
import hashlib
import json
import os
import sqlite3
import urllib.error
import urllib.request

BASE = os.environ.get("VANTA_BASE_URL", "http://127.0.0.1:8080")
DB = "/Users/deepsaint/Desktop/vanta/backend/vanta.db"
CREATOR = "Bearer vanta-local-dev-token"
VIEWER = "Bearer vanta-viewer-token"
UPLOAD_ID = "upl-48e7a559a80f4fe6bcec7e29764768e8"


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
conn.commit()
conn.close()

set_free_baseline = req(
    f"/api/v1/creator/me/uploads/{UPLOAD_ID}",
    "PATCH",
    CREATOR,
    {
        "accessPolicy": "free",
        "accessTierId": None,
        "priceCents": None,
        "currency": "USD",
        "rentalWindowHours": None,
        "visibility": "public",
        "status": "published",
    },
)
assert set_free_baseline[0] == 200, set_free_baseline

set_subscription = req(
    f"/api/v1/creator/me/uploads/{UPLOAD_ID}",
    "PATCH",
    CREATOR,
    {
        "accessPolicy": "subscription",
        "accessTierId": "tier-2",
        "priceCents": None,
        "currency": None,
        "rentalWindowHours": None,
        "visibility": "public",
        "status": "published",
    },
)
assert set_subscription[0] == 200, set_subscription

subscribed = req("/api/v1/creator/subscriptions/crt-deepsaint/tiers/tier-2", "POST", VIEWER)
assert subscribed[0] == 200 and subscribed[1]["status"] == "active", subscribed

canceled = req("/api/v1/creator/subscriptions/crt-deepsaint", "DELETE", VIEWER)
assert canceled[0] == 204, canceled

entitlements_after_cancel = req("/api/v1/me/entitlements", token=VIEWER)
assert entitlements_after_cancel[0] == 200, entitlements_after_cancel
membership = next(
    item for item in entitlements_after_cancel[1]["memberships"] if item["creatorId"] == "crt-deepsaint"
)
assert membership["status"] == "canceling", membership

playback_during_cancel = req(f"/api/v1/playback/uploads/{UPLOAD_ID}/session", "POST", VIEWER)
assert (
    playback_during_cancel[0] == 200
    and playback_during_cancel[1]["session"]["accessScope"] == "subscription"
), playback_during_cancel

set_purchase = req(
    f"/api/v1/creator/me/uploads/{UPLOAD_ID}",
    "PATCH",
    CREATOR,
    {
        "accessPolicy": "purchase",
        "accessTierId": None,
        "priceCents": 1299,
        "currency": "USD",
        "rentalWindowHours": 48,
        "visibility": "public",
        "status": "published",
    },
)
assert set_purchase[0] == 200, set_purchase

first_purchase = req(f"/api/v1/uploads/{UPLOAD_ID}/purchase", "POST", VIEWER)
assert first_purchase[0] == 200 and first_purchase[1]["status"] == "active", first_purchase
second_purchase = req(f"/api/v1/uploads/{UPLOAD_ID}/purchase", "POST", VIEWER)
assert second_purchase[0] == 200 and second_purchase[1]["id"] == first_purchase[1]["id"], second_purchase

past = "2026-08-17T00:00:00Z"
conn = sqlite3.connect(DB)
conn.execute(
    """
    UPDATE creator_memberships
    SET status = 'canceling', renews_at = ?, ends_at = ?, canceled_at = ?
    WHERE user_id = ? AND creator_id = ?
    """,
    (past, past, past, "usr-viewer", "crt-deepsaint"),
)
conn.execute(
    """
    UPDATE content_purchases
    SET status = 'active', expires_at = ?
    WHERE id = ?
    """,
    (past, first_purchase[1]["id"]),
)
conn.commit()
conn.close()

membership_before_reconcile = req(
    "/api/v1/me/entitlements/memberships/crt-deepsaint",
    token=VIEWER,
)
assert membership_before_reconcile[0] == 200, membership_before_reconcile
assert membership_before_reconcile[1]["status"] == "canceling", membership_before_reconcile

purchase_before_reconcile = req(
    f"/api/v1/me/entitlements/purchases/{first_purchase[1]['id']}",
    token=VIEWER,
)
assert purchase_before_reconcile[0] == 200, purchase_before_reconcile
assert purchase_before_reconcile[1]["status"] == "active", purchase_before_reconcile

membership_reconciled = req(
    "/api/v1/me/entitlements/memberships/crt-deepsaint/reconcile",
    "POST",
    VIEWER,
)
assert membership_reconciled[0] == 200, membership_reconciled
assert membership_reconciled[1]["creatorId"] == "crt-deepsaint", membership_reconciled
assert membership_reconciled[1]["membership"]["status"] == "expired", membership_reconciled
assert len(membership_reconciled[1]["actions"]) == 1, membership_reconciled
assert membership_reconciled[1]["actions"][0]["actionType"] == "membership_expired", membership_reconciled
assert membership_reconciled[1]["actions"][0]["previousState"] == "canceling", membership_reconciled
assert membership_reconciled[1]["actions"][0]["nextState"] == "expired", membership_reconciled

purchase_reconciled = req(
    f"/api/v1/me/entitlements/purchases/{first_purchase[1]['id']}/reconcile",
    "POST",
    VIEWER,
)
assert purchase_reconciled[0] == 200, purchase_reconciled
assert purchase_reconciled[1]["purchaseId"] == first_purchase[1]["id"], purchase_reconciled
assert purchase_reconciled[1]["purchase"]["status"] == "expired", purchase_reconciled
assert len(purchase_reconciled[1]["actions"]) == 1, purchase_reconciled
assert purchase_reconciled[1]["actions"][0]["actionType"] == "purchase_expired", purchase_reconciled
assert purchase_reconciled[1]["actions"][0]["previousState"] == "active", purchase_reconciled
assert purchase_reconciled[1]["actions"][0]["nextState"] == "expired", purchase_reconciled

entitlements_after_expiry = req("/api/v1/me/entitlements", token=VIEWER)
assert entitlements_after_expiry[0] == 200, entitlements_after_expiry
membership_after_expiry = next(
    item for item in entitlements_after_expiry[1]["memberships"] if item["creatorId"] == "crt-deepsaint"
)
purchase_after_expiry = next(
    item for item in entitlements_after_expiry[1]["purchases"] if item["id"] == first_purchase[1]["id"]
)
assert membership_after_expiry["status"] == "expired", membership_after_expiry
assert purchase_after_expiry["status"] == "expired", purchase_after_expiry

playback_after_expiry = req(f"/api/v1/playback/uploads/{UPLOAD_ID}/session", "POST", VIEWER)
assert playback_after_expiry[0] == 402, playback_after_expiry

print("entitlements|inspect|reconcile|expired|deduped")
