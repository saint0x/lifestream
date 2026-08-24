import json
import os
import sqlite3
import hashlib
import urllib.error
import urllib.request

BASE = os.environ.get("VANTA_BASE_URL", "http://127.0.0.1:8080")
DB = "/Users/deepsaint/Desktop/vanta/backend/vanta.db"
OWNER = "Bearer vanta-local-dev-token"
VIEWER = "Bearer vanta-viewer-token"


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
        with urllib.request.urlopen(request) as response:
            raw = response.read().decode()
            return response.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode()
        return exc.code, json.loads(raw) if raw else None


def ensure_auth_sessions():
    conn = sqlite3.connect(DB)
    now = "2026-08-18T00:00:00Z"
    conn.execute(
        """
        INSERT OR REPLACE INTO auth_sessions (
            id, user_id, label, token_hash, scopes_json, created_at, expires_at, revoked_at, last_used_at
        ) VALUES (?, ?, ?, ?, ?, ?, NULL, NULL, NULL)
        """,
        (
            "sess-live-watch-owner",
            "usr-1",
            "live-watch-owner",
            hashlib.sha256("vanta-local-dev-token".encode()).hexdigest(),
            json.dumps(["user", "creator", "creator:write", "admin"]),
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
            "sess-live-watch-viewer",
            "usr-viewer",
            "live-watch-viewer",
            hashlib.sha256("vanta-viewer-token".encode()).hexdigest(),
            json.dumps(["user"]),
            now,
        ),
    )
    conn.commit()
    conn.close()


def ensure_live_stream():
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
    if current is None:
        started = req(
            "/api/v1/creator/me/broadcasts/start",
            "POST",
            OWNER,
            {
                "title": "live watch interaction validation",
                "category": "Tech",
                "tags": ["live", "watch", "validation"],
                "isMature": False,
                "notifyFollowers": False,
            },
        )
        assert started[0] == 200, started
        broadcast_id = started[1]["id"]
    else:
        broadcast_id = current["id"]

    refreshed = req("/api/v1/creator/me/live", token=OWNER)
    assert refreshed[0] == 200, refreshed
    connected = req(
        "/api/v1/ingest/live/connect",
        "POST",
        None,
        {
            "streamKey": refreshed[1]["profile"]["streamKey"],
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
            "bitrateKbps": 4300,
            "viewers": 188,
            "droppedFrames": 0,
            "cpuPercent": 24,
            "freeDiskGb": 512.0,
        },
        {"x-ingest-token": connected[1]["ingestToken"]},
    )
    assert heartbeat[0] == 200, heartbeat
    return connected[1]["liveStreamId"], connected[1]


ensure_auth_sessions()
stream_id, created_session = ensure_live_stream()

conn = sqlite3.connect(DB)
conn.execute(
    "DELETE FROM live_stream_clip_requests WHERE stream_id = ? AND user_id = ?",
    (stream_id, "usr-viewer"),
)
conn.commit()
conn.close()

preview = req(f"/api/v1/live/streams/{stream_id}/viewers")
assert preview[0] == 200 and preview[1]["totalViewers"] >= 1, preview

notify = req(f"/api/v1/live/streams/{stream_id}/notify", "POST", VIEWER)
assert notify[0] == 200 and notify[1]["enabled"] is True, notify

first_clip = req(f"/api/v1/live/streams/{stream_id}/clip", "POST", VIEWER)
second_clip = req(f"/api/v1/live/streams/{stream_id}/clip", "POST", VIEWER)
assert first_clip[0] == 202, first_clip
assert second_clip[0] == 202, second_clip

conn = sqlite3.connect(DB)
clip_count = conn.execute(
    "SELECT COUNT(*) FROM live_stream_clip_requests WHERE stream_id = ? AND user_id = ?",
    (stream_id, "usr-viewer"),
).fetchone()[0]
notify_enabled = conn.execute(
    """
    SELECT enabled
    FROM live_stream_notification_preferences
    WHERE user_id = ? AND streamer_id = (
        SELECT streamer_id FROM live_streams WHERE id = ?
    )
    """,
    ("usr-viewer", stream_id),
).fetchone()
conn.close()
assert clip_count == 1, clip_count
assert notify_enabled is not None and notify_enabled[0] == 1, notify_enabled

reported = req(
    f"/api/v1/live/streams/{stream_id}/report",
    "POST",
    VIEWER,
    {"reason": "spam", "details": "viewer live watch contract"},
)
assert reported[0] == 202, reported

notifications = req("/api/v1/creator/me/notifications", token=OWNER)
assert notifications[0] == 200 and any(
    item["kind"] == "live_report_received"
    and "live watch interaction validation" in item["body"]
    for item in notifications[1]
), notifications

if created_session is not None:
    ended = req(
        f"/api/v1/ingest/live/{created_session['session']['id']}/disconnect",
        "POST",
        None,
        None,
        {"x-ingest-token": created_session["ingestToken"]},
    )
    assert ended[0] == 200, ended

print("live-watch|viewers|notify|clip-dedupe|report")
