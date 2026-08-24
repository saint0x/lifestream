import json
import os
import sqlite3
import urllib.error
import urllib.request
import uuid

BASE = os.environ.get("VANTA_BASE_URL", "http://127.0.0.1:8080")
DB = "/Users/deepsaint/Desktop/vanta/backend/vanta.db"
OWNER = "Bearer vanta-local-dev-token"
USER = "Bearer vanta-local-collaborator-token"


def req(path, method="GET", token=None, body=None, extra_headers=None):
    headers = {}
    if token:
        headers["Authorization"] = token
    if extra_headers:
        headers.update(extra_headers)
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


def lookup_creator(handle):
    conn = sqlite3.connect(DB)
    row = conn.execute(
        "SELECT id, user_id FROM creator_profiles WHERE handle = ?",
        (handle,),
    ).fetchone()
    conn.close()
    assert row is not None, handle
    return row[0], row[1]


def insert_expired_moderation_action(stream_id, creator_id, actor_user_id):
    conn = sqlite3.connect(DB)
    action_id = f"lma-expired-{uuid.uuid4().hex}"
    conn.execute(
        """
        INSERT INTO live_moderation_actions (
            id, stream_id, creator_id, subject_user_id, actor_user_id, action_type, reason,
            state, expires_at, created_at, revoked_at
        ) VALUES (?, ?, ?, ?, ?, 'mute', ?, 'active', datetime('now', '-10 minutes'), datetime('now', '-2 hours'), NULL)
        """,
        (
            action_id,
            stream_id,
            creator_id,
            "usr-2",
            actor_user_id,
            "expired moderation inspection",
        ),
    )
    conn.commit()
    conn.close()
    return action_id


def ensure_live_for_owner():
    streams = req("/api/v1/live/streams")
    assert streams[0] == 200, streams
    existing = next(
        (item for item in streams[1] if item["streamer"]["handle"] == "deepsaint"),
        None,
    )
    if existing is not None:
        return existing["id"], None

    live = req("/api/v1/creator/me/live", token=OWNER)
    assert live[0] == 200, live
    current = live[1]["currentBroadcast"] or live[1]["pendingBroadcast"]
    broadcast_id = current["id"] if current is not None else None
    if broadcast_id is None:
        started = req(
            "/api/v1/creator/me/broadcasts/start",
            "POST",
            OWNER,
            {
                "title": "live moderation control validation",
                "category": "Systems",
                "tags": ["moderation", "control"],
                "thumbnail": None,
                "isMature": False,
                "notifyFollowers": False,
            },
        )
        assert started[0] == 200, started
        broadcast_id = started[1]["id"]

    live_after = req("/api/v1/creator/me/live", token=OWNER)
    assert live_after[0] == 200, live_after
    connected = req(
        "/api/v1/ingest/live/connect",
        "POST",
        None,
        {
            "streamKey": live_after[1]["profile"]["streamKey"],
            "protocol": "rtmp",
            "ingestServer": "rtmp-us-east-1",
            "broadcastId": broadcast_id,
        },
    )
    assert connected[0] == 200, connected
    heartbeat = req(
        f"/api/v1/ingest/live/{connected[1]['session']['id']}/heartbeat",
        "POST",
        None,
        {
            "bitrateKbps": 4200,
            "viewers": 77,
            "droppedFrames": 0,
            "cpuPercent": 19,
            "freeDiskGb": 512.0,
        },
        {"x-ingest-token": connected[1]["ingestToken"]},
    )
    assert heartbeat[0] == 200, heartbeat
    return connected[1]["liveStreamId"], connected[1]


creator_id, owner_user_id = lookup_creator("deepsaint")
stream_id, created_session = ensure_live_for_owner()
settings = req(
    "/api/v1/creator/me/live/settings",
    "PATCH",
    OWNER,
    {"subscriberOnly": False, "slowModeSeconds": 0, "autoModLevel": "off"},
)
assert settings[0] == 200, settings

reported = req(
    f"/api/v1/live/streams/{stream_id}/report",
    "POST",
    USER,
    {"reason": "harassment", "details": "deterministic moderation flow"},
)
assert reported[0] == 202, reported

reports = req(f"/api/v1/live/streams/{stream_id}/moderation/reports", token=OWNER)
assert reports[0] == 200 and len(reports[1]) >= 1, reports
report_id = reports[1][0]["id"]

removed_existing = req(
    f"/api/v1/live/streams/{stream_id}/moderation/moderators/usr-1",
    "DELETE",
    OWNER,
)
assert removed_existing[0] in (204, 404), removed_existing

mod_added = req(
    f"/api/v1/live/streams/{stream_id}/moderation/moderators",
    "POST",
    OWNER,
    {"userId": "usr-1", "role": "mod"},
)
assert mod_added[0] == 200 and mod_added[1]["userId"] == "usr-1", mod_added

mods = req(f"/api/v1/live/streams/{stream_id}/moderation/moderators", token=OWNER)
assert mods[0] == 200 and any(item["userId"] == "usr-1" for item in mods[1]), mods

resolved = req(
    f"/api/v1/live/streams/{stream_id}/moderation/reports/{report_id}",
    "PATCH",
    OWNER,
    {"status": "reviewing", "resolutionNote": "triaged by moderator"},
)
assert resolved[0] == 200 and resolved[1]["status"] == "reviewing", resolved

mute = req(
    f"/api/v1/live/streams/{stream_id}/moderation/actions",
    "POST",
    OWNER,
    {
        "subjectUserId": "usr-2",
        "actionType": "mute",
        "reason": "cooldown",
        "durationMinutes": 10,
    },
)
assert mute[0] == 200 and mute[1]["state"] == "active", mute

blocked = req(
    f"/api/v1/live/streams/{stream_id}/chat/messages",
    "POST",
    USER,
    {"body": "should be blocked"},
)
assert blocked[0] == 403, blocked

revoked = req(
    f"/api/v1/live/streams/{stream_id}/moderation/actions/{mute[1]['id']}/revoke",
    "POST",
    OWNER,
)
assert revoked[0] == 200 and revoked[1]["state"] == "revoked", revoked

shadow = req(
    f"/api/v1/live/streams/{stream_id}/moderation/actions",
    "POST",
    OWNER,
    {
        "subjectUserId": "usr-2",
        "actionType": "shadowban",
        "reason": "shadow test",
        "durationMinutes": 10,
    },
)
assert shadow[0] == 200 and shadow[1]["state"] == "active", shadow

sent = req(
    f"/api/v1/live/streams/{stream_id}/chat/messages",
    "POST",
    USER,
    {"body": "shadow hidden message"},
)
assert sent[0] == 200, sent

messages = req(f"/api/v1/live/streams/{stream_id}/chat")
assert messages[0] == 200 and all(item["id"] != sent[1]["id"] for item in messages[1]), messages

expired_action_id = insert_expired_moderation_action(stream_id, creator_id, owner_user_id)
inspected = req(
    f"/api/v1/live/streams/{stream_id}/moderation/actions/{expired_action_id}",
    token=OWNER,
)
assert inspected[0] == 200, inspected
assert inspected[1]["id"] == expired_action_id, inspected
assert inspected[1]["state"] == "active", inspected

reconciled = req(
    f"/api/v1/live/streams/{stream_id}/moderation/actions/{expired_action_id}/reconcile",
    "POST",
    OWNER,
)
assert reconciled[0] == 200, reconciled
assert reconciled[1]["actionId"] == expired_action_id, reconciled
assert reconciled[1]["action"]["id"] == expired_action_id, reconciled
assert reconciled[1]["action"]["state"] == "expired", reconciled
assert len(reconciled[1]["actions"]) == 1, reconciled
assert reconciled[1]["actions"][0]["actionType"] == "action_expired", reconciled
assert reconciled[1]["actions"][0]["previousState"] == "active", reconciled
assert reconciled[1]["actions"][0]["nextState"] == "expired", reconciled

actions = req(f"/api/v1/live/streams/{stream_id}/moderation/actions", token=OWNER)
assert actions[0] == 200 and any(item["actionType"] == "mute" for item in actions[1]) and any(
    item["actionType"] == "shadowban" for item in actions[1]
), actions
assert any(item["id"] == expired_action_id and item["state"] == "expired" for item in actions[1]), actions

audit = req(f"/api/v1/live/streams/{stream_id}/moderation/audit", token=OWNER)
assert audit[0] == 200 and any(item["eventType"] == "moderator_added" for item in audit[1]) and any(
    item["eventType"] == "report_resolved" for item in audit[1]
), audit
assert any(
    item["eventType"] == "moderation_action_expired"
    and item["payload"]["actionId"] == expired_action_id
    for item in audit[1]
), audit

if created_session is not None:
    ended = req(
        f"/api/v1/ingest/live/{created_session['session']['id']}/disconnect",
        "POST",
        None,
        None,
        {"x-ingest-token": created_session["ingestToken"]},
    )
    assert ended[0] == 200, ended

print("live-moderation|inspect|reconcile|revoke")
