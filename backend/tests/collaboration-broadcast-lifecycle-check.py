import json
import sqlite3
import time
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"
DB = "/Users/deepsaint/Desktop/vanta/backend/vanta.db"
HOST = "Bearer vanta-local-dev-token"
COLLAB = "Bearer vanta-local-collaborator-token"
SUFFIX = str(int(time.time() * 1000))


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


def cleanup_live():
    live = req("/api/v1/creator/me/live", token=HOST)
    assert live[0] == 200, live
    for broadcast_key in ("currentBroadcast", "pendingBroadcast"):
        broadcast = live[1].get(broadcast_key)
        if broadcast is not None:
            ended = req(
                f"/api/v1/creator/me/broadcasts/{broadcast['id']}/end",
                "POST",
                HOST,
            )
            assert ended[0] == 200, ended


def cleanup_collabs():
    collabs = req("/api/v1/creator/me/live/collabs", token=HOST)
    assert collabs[0] == 200, collabs
    for item in collabs[1]:
        if item["status"] in ("pending", "active"):
            ended = req(
                f"/api/v1/creator/me/live/collabs/sessions/{item['id']}/end",
                "POST",
                HOST,
            )
            assert ended[0] == 200, ended


def start_broadcast(title):
    started = req(
        "/api/v1/creator/me/broadcasts/start",
        "POST",
        HOST,
        {
            "title": title,
            "category": "Tech",
            "tags": ["collaboration", "lifecycle", "live"],
            "isMature": False,
            "notifyFollowers": False,
        },
    )
    assert started[0] == 200, started
    return started[1]


def create_session(broadcast_id, title):
    created = req(
        "/api/v1/creator/me/live/collabs/sessions",
        "POST",
        HOST,
        {
            "broadcastId": broadcast_id,
            "title": title,
            "chatMode": "shared",
            "recordingPolicy": "host_archive",
        },
    )
    assert created[0] == 200, created
    return created[1]


cleanup_live()
cleanup_collabs()

first_broadcast = start_broadcast(f"Collab broadcast end validation {SUFFIX}")
first_session = create_session(first_broadcast["id"], f"Collab lifecycle {SUFFIX}")

invite = req(
    f"/api/v1/creator/me/live/collabs/sessions/{first_session['id']}/invites",
    "POST",
    HOST,
    {
        "inviteeUserId": "usr-2",
        "role": "co_streamer",
        "mirrorToGuestChannel": True,
        "message": "join lifecycle validation",
        "expiresInMinutes": 30,
    },
)
assert invite[0] == 200, invite
accepted = req(f"/api/v1/live/collabs/invites/{invite[1]['id']}/accept", "POST", COLLAB)
assert accepted[0] == 200, accepted

ended_broadcast = req(
    f"/api/v1/creator/me/broadcasts/{first_broadcast['id']}/end",
    "POST",
    HOST,
)
assert ended_broadcast[0] == 200, ended_broadcast

session_after_broadcast_end = req(
    f"/api/v1/creator/me/live/collabs/sessions/{first_session['id']}",
    token=HOST,
)
assert session_after_broadcast_end[0] == 200, session_after_broadcast_end
assert session_after_broadcast_end[1]["status"] == "ended", session_after_broadcast_end

events_after_broadcast_end = req(
    f"/api/v1/creator/me/live/collabs/sessions/{first_session['id']}/events",
    token=HOST,
)
assert events_after_broadcast_end[0] == 200, events_after_broadcast_end
assert any(
    event["eventType"] == "session_ended"
    and event["payload"]["details"]["reason"] == "source broadcast ended"
    for event in events_after_broadcast_end[1]
), events_after_broadcast_end

second_broadcast = start_broadcast(f"Collab reconcile lifecycle {SUFFIX}")
second_session = create_session(second_broadcast["id"], f"Collab reconcile {SUFFIX}")

conn = sqlite3.connect(DB)
conn.execute(
    "UPDATE broadcasts SET status = 'ended', ended_at = ?, duration_sec = 1 WHERE id = ?",
    ("2026-08-17T00:00:00Z", second_broadcast["id"]),
)
conn.commit()
conn.close()

reconciled = req(
    f"/api/v1/creator/me/live/collabs/sessions/{second_session['id']}",
    HOST,
)
assert reconciled[0] == 200, reconciled
assert reconciled[1]["status"] == "ended", reconciled

events_after_reconcile = req(
    f"/api/v1/creator/me/live/collabs/sessions/{second_session['id']}/events",
    token=HOST,
)
assert events_after_reconcile[0] == 200, events_after_reconcile
assert any(
    event["eventType"] == "session_ended"
    and event["payload"]["details"]["reason"] == "source broadcast is no longer active"
    for event in events_after_reconcile[1]
), events_after_reconcile

print("collab-lifecycle|broadcast-end|read-heal-end")
