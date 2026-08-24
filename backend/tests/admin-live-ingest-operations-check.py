import json
import sqlite3
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"
DB = "/Users/deepsaint/Desktop/vanta/backend/vanta.db"
AUTH = "Bearer vanta-local-dev-token"


def req(path, method="GET", body=None, headers=None):
    request_headers = {"Authorization": AUTH}
    if headers:
        request_headers.update(headers)
    data = None
    if body is not None:
        request_headers["Content-Type"] = "application/json"
        data = json.dumps(body).encode()
    request = urllib.request.Request(
        BASE + path,
        headers=request_headers,
        data=data,
        method=method,
    )
    try:
        with urllib.request.urlopen(request) as response:
            raw = response.read().decode()
            return response.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode()
        return exc.code, json.loads(raw) if raw else None


def cleanup_active_broadcast():
    live = req("/api/v1/creator/me/live")
    assert live[0] == 200, live
    current = live[1]["currentBroadcast"] or live[1]["pendingBroadcast"]
    if current is not None:
        ended = req(f"/api/v1/creator/me/broadcasts/{current['id']}/end", "POST")
        assert ended[0] == 200, ended
    conn = sqlite3.connect(DB)
    conn.execute(
        "UPDATE broadcasts SET status = 'ended', ended_at = COALESCE(ended_at, ?), duration_sec = COALESCE(duration_sec, 0) WHERE creator_id = ? AND status IN ('ready', 'live')",
        ("2026-08-21T13:00:00+00:00", "crt-deepsaint"),
    )
    conn.execute(
        "UPDATE creator_profiles SET live_status = 'offline', current_broadcast_id = NULL WHERE id = ?",
        ("crt-deepsaint",),
    )
    conn.commit()
    conn.close()


cleanup_active_broadcast()

started = req(
    "/api/v1/creator/me/broadcasts/start",
    "POST",
    {
        "title": "Admin ingest operator validation",
        "category": "Tech",
        "tags": ["admin", "ingest", "ops"],
        "isMature": False,
        "notifyFollowers": False,
    },
)
if started[0] == 400 and started[1] == {
    "error": "bad request: an active or pending broadcast already exists"
}:
    cleanup_active_broadcast()
    started = req(
        "/api/v1/creator/me/broadcasts/start",
        "POST",
        {
            "title": "Admin ingest operator validation",
            "category": "Tech",
            "tags": ["admin", "ingest", "ops"],
            "isMature": False,
            "notifyFollowers": False,
        },
    )
assert started[0] == 200, started
broadcast_id = started[1]["id"]

live = req("/api/v1/creator/me/live")
assert live[0] == 200 and live[1]["pendingBroadcast"] is not None, live
stream_key = live[1]["profile"]["streamKey"]

connected = req(
    "/api/v1/ingest/live/connect",
    "POST",
    {
        "streamKey": stream_key,
        "protocol": "rtmp",
        "ingestServer": "rtmp-us-east-1",
        "broadcastId": broadcast_id,
    },
)
assert connected[0] == 200, connected
session_id = connected[1]["session"]["id"]
ingest_token = connected[1]["ingestToken"]

heartbeat = req(
    f"/api/v1/ingest/live/{session_id}/heartbeat",
    "POST",
    {
        "bitrateKbps": 4800,
        "viewers": 222,
        "droppedFrames": 1,
        "cpuPercent": 33,
        "freeDiskGb": 512.0,
    },
    {"x-ingest-token": ingest_token},
)
assert heartbeat[0] == 200, heartbeat

record = req(f"/api/v1/admin/live/ingest/sessions/{session_id}")
assert record[0] == 200, record
assert record[1]["session"]["id"] == session_id, record
assert any(event["eventType"] == "heartbeat_recorded" for event in record[1]["recentEvents"]), record

conn = sqlite3.connect(DB)
conn.execute(
    "UPDATE live_ingest_sessions SET last_heartbeat_at = ? WHERE id = ?",
    ("2026-08-17T00:00:00+00:00", session_id),
)
conn.commit()
conn.close()

reconciled = req(
    f"/api/v1/admin/live/ingest/sessions/{session_id}/reconcile",
    "POST",
)
assert reconciled[0] == 200, reconciled
assert reconciled[1]["sessionId"] == session_id, reconciled
assert any(
    action["actionType"] == "session_marked_stale"
    and action["previousStatus"] == "connected"
    and action["nextStatus"] == "stale"
    for action in reconciled[1]["actions"]
), reconciled
assert reconciled[1]["record"]["session"]["status"] == "stale", reconciled
assert any(
    event["eventType"] == "stale_reconciled"
    for event in reconciled[1]["record"]["recentEvents"]
), reconciled

creator_live = req("/api/v1/creator/me/live")
assert creator_live[0] == 200, creator_live
assert creator_live[1]["profile"]["liveStatus"] == "starting", creator_live
assert creator_live[1]["currentBroadcast"] is None, creator_live
assert creator_live[1]["pendingBroadcast"] is not None, creator_live

reconnected = req(
    "/api/v1/ingest/live/connect",
    "POST",
    {
        "streamKey": stream_key,
        "protocol": "rtmp",
        "ingestServer": "rtmp-us-east-2",
        "broadcastId": broadcast_id,
    },
)
assert reconnected[0] == 200, reconnected
reconnected_session_id = reconnected[1]["session"]["id"]

terminated = req(
    f"/api/v1/admin/live/ingest/sessions/{reconnected_session_id}/terminate",
    "POST",
    {"reason": "operator cleanup validation"},
)
assert terminated[0] == 200, terminated
assert terminated[1]["session"]["status"] == "terminated", terminated
assert any(event["eventType"] == "admin_terminated" for event in terminated[1]["recentEvents"]), terminated

final_live = req("/api/v1/creator/me/live")
assert final_live[0] == 200, final_live
assert final_live[1]["profile"]["liveStatus"] == "offline", final_live
assert final_live[1]["currentBroadcast"] is None, final_live

print("admin-live-ingest|inspect|stale|terminate")
