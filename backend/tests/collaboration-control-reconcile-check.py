import datetime
import hashlib
import json
import os
import sqlite3
import time
import urllib.error
import urllib.request

BASE = os.environ.get("VANTA_BASE_URL", "http://127.0.0.1:8080")
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


live_status, live = req("/api/v1/creator/me/live", token=HOST)
assert live_status == 200, (live_status, live)
broadcast = live["currentBroadcast"] or live["pendingBroadcast"]
if broadcast is None:
    started = req(
        "/api/v1/creator/me/broadcasts/start",
        "POST",
        HOST,
        {
            "title": f"Collab reconcile validation {SUFFIX}",
            "category": "Tech",
            "tags": ["collaboration", "reconcile", "control"],
            "isMature": False,
            "notifyFollowers": False,
        },
    )
    assert started[0] == 200, started
    broadcast = started[1]

collabs = req("/api/v1/creator/me/live/collabs", token=HOST)
assert collabs[0] == 200, collabs
for item in collabs[1]:
    if item["sourceBroadcastId"] == broadcast["id"] and item["status"] in ("pending", "active"):
        ended = req(
            f"/api/v1/creator/me/live/collabs/sessions/{item['id']}/end",
            "POST",
            HOST,
        )
        assert ended[0] == 200, ended

created = req(
    "/api/v1/creator/me/live/collabs/sessions",
    "POST",
    HOST,
    {
        "broadcastId": broadcast["id"],
        "title": f"Collab control {SUFFIX}",
        "chatMode": "shared",
        "recordingPolicy": "host_archive",
    },
)
assert created[0] == 200, created
session_id = created[1]["id"]

invite = req(
    f"/api/v1/creator/me/live/collabs/sessions/{session_id}/invites",
    "POST",
    HOST,
    {
        "inviteeUserId": "usr-2",
        "role": "co_streamer",
        "mirrorToGuestChannel": True,
        "message": "reconcile me",
        "expiresInMinutes": 30,
    },
)
assert invite[0] == 200, invite
invite_id = invite[1]["id"]

accepted = req(f"/api/v1/live/collabs/invites/{invite_id}/accept", "POST", COLLAB)
assert accepted[0] == 200, accepted
participant_id = accepted[1]["id"]

to_live = req(
    f"/api/v1/creator/me/live/collabs/sessions/{session_id}/participants/{participant_id}",
    "PATCH",
    HOST,
    {
        "state": "live",
        "mirrorToGuestChannel": True,
        "publishToHost": True,
        "canSpeakInChat": True,
    },
)
assert to_live[0] == 200, to_live

grant = req(
    f"/api/v1/creator/me/live/collabs/sessions/{session_id}/participants/{participant_id}/grants/mirror",
    "POST",
    HOST,
)
assert grant[0] == 200 and grant[1]["state"] == "issued", grant
grant_id = grant[1]["id"]

control_before = req(
    f"/api/v1/creator/me/live/collabs/sessions/{session_id}/control",
    token=HOST,
)
assert control_before[0] == 200, control_before
assert control_before[1]["pendingInviteCount"] == 0, control_before
assert control_before[1]["issuedGrantCount"] == 1, control_before

past = "2026-08-17T00:00:00Z"
cutoff = (
    datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(seconds=120)
).replace(microsecond=0).isoformat().replace("+00:00", "Z")

conn = sqlite3.connect(DB)
conn.execute(
    "UPDATE collaboration_invites SET state = 'pending', responded_at = NULL, expires_at = ? WHERE id = ?",
    (past, invite_id),
)
conn.execute(
    "UPDATE collaboration_mirror_grants SET state = 'issued', revoked_at = NULL, expires_at = ? WHERE id = ?",
    (past, grant_id),
)
conn.execute(
    """
    INSERT INTO collaboration_socket_sessions (
        id, collaboration_session_id, user_id, creator_id, participant_id,
        session_token_hash, connected_at, last_seen_at, disconnected_at
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL)
    """,
    (
        f"css-test-{SUFFIX}",
        session_id,
        "usr-2",
        "crt-atlas",
        participant_id,
        hashlib.sha256(f"stale-collab-{SUFFIX}".encode()).hexdigest(),
        cutoff,
        cutoff,
    ),
)
conn.commit()
conn.close()

socket_inspected = req(
    f"/api/v1/creator/me/live/collabs/sessions/{session_id}/socket-sessions/css-test-{SUFFIX}",
    token=HOST,
)
assert socket_inspected[0] == 200, socket_inspected
assert socket_inspected[1]["id"] == f"css-test-{SUFFIX}", socket_inspected
assert socket_inspected[1]["isStale"] is True, socket_inspected
assert socket_inspected[1]["disconnectedAt"] is None, socket_inspected

socket_reconciled = req(
    f"/api/v1/creator/me/live/collabs/sessions/{session_id}/socket-sessions/css-test-{SUFFIX}/reconcile",
    "POST",
    HOST,
)
assert socket_reconciled[0] == 200, socket_reconciled
assert socket_reconciled[1]["sessionId"] == session_id, socket_reconciled
assert socket_reconciled[1]["socketSessionId"] == f"css-test-{SUFFIX}", socket_reconciled
assert socket_reconciled[1]["socketSession"]["id"] == f"css-test-{SUFFIX}", socket_reconciled
assert socket_reconciled[1]["socketSession"]["disconnectedAt"] is not None, socket_reconciled
assert len(socket_reconciled[1]["actions"]) == 1, socket_reconciled
assert socket_reconciled[1]["actions"][0]["actionType"] == "socket_disconnected", socket_reconciled
assert socket_reconciled[1]["actions"][0]["previousState"] == "connected", socket_reconciled
assert socket_reconciled[1]["actions"][0]["nextState"] == "disconnected", socket_reconciled

control_dirty = req(
    f"/api/v1/creator/me/live/collabs/sessions/{session_id}/control",
    token=HOST,
)
assert control_dirty[0] == 200, control_dirty
assert control_dirty[1]["pendingInviteCount"] == 0, control_dirty
assert control_dirty[1]["issuedGrantCount"] == 0, control_dirty
assert control_dirty[1]["staleSocketCount"] == 0, control_dirty
assert any(
    socket["id"] == f"css-test-{SUFFIX}" and socket["disconnectedAt"] is not None
    for socket in control_dirty[1]["socketSessions"]
), control_dirty

reconciled = req(
    f"/api/v1/creator/me/live/collabs/sessions/{session_id}/reconcile",
    "POST",
    HOST,
)
assert reconciled[0] == 200, reconciled
report = reconciled[1]
action_types = [action["actionType"] for action in report["actions"]]
assert report["control"]["pendingInviteCount"] == 0, report
assert report["control"]["issuedGrantCount"] == 0, report
assert report["control"]["staleSocketCount"] == 0, report
assert any(
    socket["id"] == f"css-test-{SUFFIX}" and socket["disconnectedAt"] is not None
    for socket in report["control"]["socketSessions"]
), report

events = req(
    f"/api/v1/creator/me/live/collabs/sessions/{session_id}/events",
    token=HOST,
)
assert events[0] == 200, events
assert any(event["eventType"] == "invite_expired" for event in events[1]), events
assert any(event["eventType"] == "mirror_grant_expired" for event in events[1]), events

ended = req(f"/api/v1/creator/me/live/collabs/sessions/{session_id}/end", "POST", HOST)
assert ended[0] == 200 and ended[1]["status"] == "ended", ended

print("collab-control|socket-inspect|read-heal|reconcile-clean")
