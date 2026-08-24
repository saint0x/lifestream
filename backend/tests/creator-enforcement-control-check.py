import json
import sqlite3
import urllib.error
import urllib.request
import uuid

BASE = "http://127.0.0.1:8080"
DB = "/Users/deepsaint/Desktop/vanta/backend/vanta.db"
ADMIN = "Bearer vanta-local-dev-token"
ATLAS = "Bearer vanta-local-collaborator-token"


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
        with urllib.request.urlopen(request) as resp:
            raw = resp.read().decode()
            return resp.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode()
        return exc.code, json.loads(raw) if raw else None


def db_setup():
    conn = sqlite3.connect(DB)
    conn.execute(
        "UPDATE auth_sessions SET scopes_json = ? WHERE id = ?",
        (json.dumps(["user", "creator", "creator:write", "admin"]), "sess-local-admin"),
    )
    conn.execute(
        "DELETE FROM creator_enforcement_actions WHERE creator_id IN (?, ?)",
        ("crt-atlas", "crt-deepsaint"),
    )
    conn.execute(
        "DELETE FROM creator_subscriber_tiers WHERE id LIKE ?",
        ("tier-enforcement-%",),
    )
    conn.commit()
    conn.close()


def insert_expired_action(creator_id, scope, reason):
    conn = sqlite3.connect(DB)
    action_id = f"cea-expired-{uuid.uuid4().hex}"
    creator_row = conn.execute(
        "SELECT user_id FROM creators WHERE id = ?",
        (creator_id,),
    ).fetchone()
    assert creator_row is not None, creator_id
    conn.execute(
        """
        INSERT INTO creator_enforcement_actions (
            id, creator_id, scope, state, reason, resolution_note, created_by_user_id,
            released_by_user_id, created_at, released_at, expires_at
        ) VALUES (?, ?, ?, 'active', ?, NULL, ?, NULL, datetime('now', '-2 hours'), NULL, datetime('now', '-10 minutes'))
        """,
        (action_id, creator_id, scope, reason, creator_row[0]),
    )
    conn.commit()
    conn.close()
    return action_id


def cleanup_broadcast(token):
    status, live = req("/api/v1/creator/me/live", token=token)
    assert status == 200, (status, live)
    pending = live.get("pendingBroadcast")
    current = live.get("currentBroadcast")
    target = current or pending
    if target:
        end_status, ended = req(
            f"/api/v1/creator/me/broadcasts/{target['id']}/end",
            "POST",
            token,
        )
        assert end_status == 200, (end_status, ended)


db_setup()
cleanup_broadcast(ATLAS)

start_status, started = req(
    "/api/v1/creator/me/broadcasts/start",
    "POST",
    ATLAS,
    {
        "title": "Enforcement baseline live",
        "category": "Systems",
        "tags": ["enforcement", "baseline"],
        "thumbnail": None,
        "isMature": False,
        "notifyFollowers": False,
    },
)
assert start_status == 200, (start_status, started)
cleanup_broadcast(ATLAS)

live_action_status, live_action = req(
    "/api/v1/admin/creators/crt-atlas/enforcement/actions",
    "POST",
    ADMIN,
    {
        "scope": "live_streaming",
        "reason": "incident review freeze",
    },
)
assert live_action_status == 200 and live_action["scope"] == "live_streaming", (
    live_action_status,
    live_action,
)
live_action_id = live_action["id"]

admin_live_state = req("/api/v1/admin/creators/crt-atlas/enforcement", token=ADMIN)
assert admin_live_state[0] == 200, admin_live_state
assert admin_live_state[1]["liveStreamingEnabled"] is False, admin_live_state
assert any(
    item["id"] == live_action_id for item in admin_live_state[1]["activeActions"]
), admin_live_state

atlas_ops = req("/api/v1/creator/me/operations", token=ATLAS)
assert atlas_ops[0] == 200, atlas_ops
assert atlas_ops[1]["liveStreamingEnabled"] is False, atlas_ops

blocked_live = req(
    "/api/v1/creator/me/broadcasts/start",
    "POST",
    ATLAS,
    {
        "title": "Blocked live",
        "category": "Systems",
        "tags": ["blocked"],
        "thumbnail": None,
        "isMature": False,
        "notifyFollowers": False,
    },
)
assert blocked_live[0] == 400 and "not currently allowed" in blocked_live[1]["error"], blocked_live

released_live = req(
    f"/api/v1/admin/creators/crt-atlas/enforcement/actions/{live_action_id}/release",
    "POST",
    ADMIN,
    {"resolutionNote": "incident cleared"},
)
assert released_live[0] == 200 and released_live[1]["state"] == "released", released_live

start_after_release = req(
    "/api/v1/creator/me/broadcasts/start",
    "POST",
    ATLAS,
    {
        "title": "Live restored",
        "category": "Systems",
        "tags": ["restored"],
        "thumbnail": None,
        "isMature": False,
        "notifyFollowers": False,
    },
)
assert start_after_release[0] == 200, start_after_release
cleanup_broadcast(ATLAS)

upload_action_status, upload_action = req(
    "/api/v1/admin/creators/crt-atlas/enforcement/actions",
    "POST",
    ADMIN,
    {
        "scope": "uploads",
        "reason": "upload abuse review",
    },
)
assert upload_action_status == 200, (upload_action_status, upload_action)
upload_action_id = upload_action["id"]
blocked_upload = req(
    "/api/v1/creator/me/upload-jobs",
    "POST",
    ATLAS,
    {
        "kind": "video",
        "sourceType": "resumable-upload",
        "title": "Blocked upload",
        "intendedVisibility": "private",
        "bytesExpected": 1024,
        "storageKey": f"uploads/creator/atlas/features/enforcement-{uuid.uuid4().hex}.mp4",
        "mimeType": "video/mp4",
    },
)
assert blocked_upload[0] == 400 and "not currently allowed" in blocked_upload[1]["error"], blocked_upload
released_upload = req(
    f"/api/v1/admin/creators/crt-atlas/enforcement/actions/{upload_action_id}/release",
    "POST",
    ADMIN,
    {"resolutionNote": "upload review cleared"},
)
assert released_upload[0] == 200 and released_upload[1]["state"] == "released", released_upload

expired_action_id = insert_expired_action(
    "crt-atlas",
    "uploads",
    "expired upload review freeze",
)
inspected_expired = req(
    f"/api/v1/admin/creators/crt-atlas/enforcement/actions/{expired_action_id}",
    token=ADMIN,
)
assert inspected_expired[0] == 200, inspected_expired
assert inspected_expired[1]["id"] == expired_action_id, inspected_expired
assert inspected_expired[1]["state"] == "active", inspected_expired

reconciled_expired = req(
    f"/api/v1/admin/creators/crt-atlas/enforcement/actions/{expired_action_id}/reconcile",
    "POST",
    ADMIN,
)
assert reconciled_expired[0] == 200, reconciled_expired
assert reconciled_expired[1]["creatorId"] == "crt-atlas", reconciled_expired
assert reconciled_expired[1]["action"]["id"] == expired_action_id, reconciled_expired
assert reconciled_expired[1]["action"]["state"] == "expired", reconciled_expired
assert len(reconciled_expired[1]["actions"]) == 1, reconciled_expired
assert reconciled_expired[1]["actions"][0]["actionType"] == "action_expired", reconciled_expired
assert reconciled_expired[1]["actions"][0]["previousState"] == "active", reconciled_expired
assert reconciled_expired[1]["actions"][0]["nextState"] == "expired", reconciled_expired

monetization_action_status, monetization_action = req(
    "/api/v1/admin/creators/crt-deepsaint/enforcement/actions",
    "POST",
    ADMIN,
    {
        "scope": "monetization",
        "reason": "manual payout hold review",
    },
)
assert monetization_action_status == 200, (
    monetization_action_status,
    monetization_action,
)
monetization_action_id = monetization_action["id"]

deepsaint_ops = req("/api/v1/creator/me/operations", token=ADMIN)
assert deepsaint_ops[0] == 200, deepsaint_ops
assert deepsaint_ops[1]["monetizationEnabled"] is False, deepsaint_ops
assert deepsaint_ops[1]["canMonetize"] is False, deepsaint_ops

tier_id = f"tier-enforcement-{uuid.uuid4().hex[:8]}"
blocked_tier = req(
    "/api/v1/creator/me/subscriber-tiers",
    "POST",
    ADMIN,
    {
        "tierName": tier_id,
        "monthlyPrice": 14.99,
        "accentColor": "#ff6600",
    },
)
assert blocked_tier[0] == 400 and "not cleared" in blocked_tier[1]["error"], blocked_tier

released_monetization = req(
    f"/api/v1/admin/creators/crt-deepsaint/enforcement/actions/{monetization_action_id}/release",
    "POST",
    ADMIN,
    {"resolutionNote": "hold released"},
)
assert released_monetization[0] == 200, released_monetization

created_tier = req(
    "/api/v1/creator/me/subscriber-tiers",
    "POST",
    ADMIN,
    {
        "tierName": tier_id,
        "monthlyPrice": 14.99,
        "accentColor": "#ff6600",
    },
)
assert created_tier[0] == 200, created_tier
retired_tier = req(
    f"/api/v1/creator/me/subscriber-tiers/{created_tier[1]['id']}/retire",
    "POST",
    ADMIN,
)
assert retired_tier[0] == 200 and retired_tier[1]["status"] == "retired", retired_tier

print("creator-enforcement|admin|gates|release|inspect|reconcile")
