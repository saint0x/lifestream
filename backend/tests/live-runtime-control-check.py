import json
import hashlib
import os
import sqlite3
import time
import urllib.error
import urllib.request

BASE = os.environ.get("LIFESTREAM_BASE_URL", "http://127.0.0.1:8080")
DB = "/Users/deepsaint/Desktop/lifestream/backend/lifestream.db"
AUTH = "Bearer lifestream-local-dev-token"
HEADERS = {"Authorization": AUTH, "Content-Type": "application/json"}
SUFFIX = str(int(time.time() * 1000))


def req(path, method="GET", body=None, headers=None):
    request_headers = dict(headers or {})
    data = None
    if body is not None:
        data = json.dumps(body).encode()
        request_headers.setdefault("Content-Type", "application/json")
    request = urllib.request.Request(BASE + path, data=data, headers=request_headers, method=method)
    try:
        with urllib.request.urlopen(request) as response:
            raw = response.read().decode()
            return response.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode()
        return exc.code, json.loads(raw) if raw else None


live_before = req("/api/v1/creator/me/live", headers={"Authorization": AUTH})
assert live_before[0] == 200, live_before
for broadcast_key in ("currentBroadcast", "pendingBroadcast"):
    broadcast = live_before[1].get(broadcast_key)
    if broadcast is not None:
        ended = req(
            f"/api/v1/creator/me/broadcasts/{broadcast['id']}/end",
            "POST",
            headers={"Authorization": AUTH},
        )
        assert ended[0] == 200, ended


start = req(
    "/api/v1/creator/me/broadcasts/start",
    "POST",
    {
        "title": f"Runtime authority validation {SUFFIX}",
        "category": "Tech",
        "tags": ["runtime", "control", "live"],
        "isMature": False,
        "notifyFollowers": False,
    },
    HEADERS,
)
assert start[0] == 200, start
broadcast = start[1]

live = req("/api/v1/creator/me/live", headers={"Authorization": AUTH})
assert live[0] == 200, live
stream_key = live[1]["profile"]["streamKey"]

connect = req(
    "/api/v1/ingest/live/connect",
    "POST",
    {
        "streamKey": stream_key,
        "protocol": "rtmp",
        "ingestServer": "rtmp-us-east-1-primary",
        "broadcastId": broadcast["id"],
    },
    {"Content-Type": "application/json"},
)
assert connect[0] == 200, connect
session = connect[1]["session"]
ingest_token = connect[1]["ingestToken"]

heartbeat = req(
    f"/api/v1/ingest/live/{session['id']}/heartbeat",
    "POST",
    {
        "bitrateKbps": 7200,
        "viewers": 1888,
        "droppedFrames": 4,
        "cpuPercent": 44,
        "freeDiskGb": 602.5,
    },
    {"Content-Type": "application/json", "x-ingest-token": ingest_token},
)
assert heartbeat[0] == 200, heartbeat

runtime = req("/api/v1/creator/me/live/runtime", headers={"Authorization": AUTH})
assert runtime[0] == 200, runtime
payload = runtime[1]
assert payload["activeSession"]["id"] == session["id"], payload
assert payload["snapshot"]["currentBroadcast"]["status"] == "live", payload
assert payload["health"]["samples"][-1]["viewers"] == 1888, payload
assert payload["collaboration"]["activeSession"] is None, payload
assert payload["collaboration"]["activeSessionCount"] == 0, payload
assert any(event["eventType"] == "connected" for event in payload["recentEvents"]), payload
assert any(
    event["eventType"] == "heartbeat_recorded" and event["payload"]["viewers"] == 1888
    for event in payload["recentEvents"]
), payload

created = req(
    "/api/v1/creator/me/live/collabs/sessions",
    "POST",
    {
        "broadcastId": broadcast["id"],
        "title": f"Runtime collab summary {SUFFIX}",
        "chatMode": "shared",
        "recordingPolicy": "host_archive",
    },
    HEADERS,
)
assert created[0] == 200, created
collab_session = created[1]

control = req("/api/v1/creator/me/live/control", headers={"Authorization": AUTH})
assert control[0] == 200, control
control_payload = control[1]
assert control_payload["collaboration"]["activeSession"]["id"] == collab_session["id"], control_payload
assert control_payload["collaboration"]["activeControl"]["runtime"]["session"]["id"] == collab_session["id"], control_payload
assert control_payload["collaboration"]["activeSessionCount"] >= 1, control_payload
assert control_payload["collaboration"]["totalSessions"] >= 1, control_payload

socket_id = f"cls-test-{SUFFIX}"
stale_seen_at = "2026-08-18T03:55:00Z"
conn = sqlite3.connect(DB)
conn.execute(
    """
    INSERT INTO creator_live_socket_sessions (
        id, creator_id, user_id, session_token_hash, connected_at, last_seen_at, disconnected_at
    ) VALUES (?, ?, ?, ?, ?, ?, NULL)
    """,
    (
        socket_id,
        live[1]["profile"]["id"],
        "usr-1",
        hashlib.sha256(f"creator-live-stale-{SUFFIX}".encode()).hexdigest(),
        stale_seen_at,
        stale_seen_at,
    ),
)
conn.commit()
conn.close()

socket_inspected = req(
    f"/api/v1/creator/me/live/socket-sessions/{socket_id}",
    headers={"Authorization": AUTH},
)
assert socket_inspected[0] == 200, socket_inspected
assert socket_inspected[1]["id"] == socket_id, socket_inspected
assert socket_inspected[1]["isStale"] is True, socket_inspected
assert socket_inspected[1]["disconnectedAt"] is None, socket_inspected

socket_reconciled = req(
    f"/api/v1/creator/me/live/socket-sessions/{socket_id}/reconcile",
    "POST",
    headers={"Authorization": AUTH},
)
assert socket_reconciled[0] == 200, socket_reconciled
assert socket_reconciled[1]["creatorId"] == live[1]["profile"]["id"], socket_reconciled
assert socket_reconciled[1]["socketSessionId"] == socket_id, socket_reconciled
assert socket_reconciled[1]["socketSession"]["id"] == socket_id, socket_reconciled
assert socket_reconciled[1]["socketSession"]["disconnectedAt"] is not None, socket_reconciled
assert len(socket_reconciled[1]["actions"]) == 1, socket_reconciled
assert socket_reconciled[1]["actions"][0]["actionType"] == "socket_disconnected", socket_reconciled
assert socket_reconciled[1]["actions"][0]["previousState"] == "connected", socket_reconciled
assert socket_reconciled[1]["actions"][0]["nextState"] == "disconnected", socket_reconciled

runtime_with_collab = req("/api/v1/creator/me/live/runtime", headers={"Authorization": AUTH})
assert runtime_with_collab[0] == 200, runtime_with_collab
runtime_collab_payload = runtime_with_collab[1]
assert runtime_collab_payload["collaboration"]["activeSession"]["id"] == collab_session["id"], runtime_collab_payload
assert runtime_collab_payload["collaboration"]["activeControl"]["runtime"]["topology"]["sharedChat"] is True, runtime_collab_payload
assert runtime_collab_payload["collaboration"]["recentSessions"][0]["id"] == collab_session["id"], runtime_collab_payload

ended_collab = req(
    f"/api/v1/creator/me/live/collabs/sessions/{collab_session['id']}/end",
    "POST",
    headers={"Authorization": AUTH},
)
assert ended_collab[0] == 200 and ended_collab[1]["status"] == "ended", ended_collab

terminated = req(
    f"/api/v1/creator/me/live/ingest/{session['id']}/terminate",
    "POST",
    {"reason": "operator cleanup validation"},
    HEADERS,
)
assert terminated[0] == 200, terminated
assert terminated[1]["status"] == "terminated", terminated

runtime_after = req("/api/v1/creator/me/live/runtime", headers={"Authorization": AUTH})
assert runtime_after[0] == 200, runtime_after
after = runtime_after[1]
assert after["activeSession"] is None, after
assert after["snapshot"]["profile"]["liveStatus"] == "offline", after
assert after["collaboration"]["activeSession"] is None, after
assert after["recentSessions"][0]["id"] == session["id"], after
assert after["recentSessions"][0]["status"] == "terminated", after
assert any(event["eventType"] == "creator_terminated" for event in after["recentEvents"]), after

print("runtime|socket-inspect|connected|terminated")
