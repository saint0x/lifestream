import json
import sqlite3
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"
DB = "/Users/deepsaint/Desktop/vanta/backend/vanta.db"
AUTH = "Bearer vanta-local-dev-token"


def req(path, method="GET", body=None, headers=None):
    request_headers = {}
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
    live = req("/api/v1/creator/me/live", headers={"Authorization": AUTH})
    assert live[0] == 200, live
    for key in ("currentBroadcast", "pendingBroadcast"):
        broadcast = live[1].get(key)
        if broadcast is not None:
            ended = req(
                f"/api/v1/creator/me/broadcasts/{broadcast['id']}/end",
                "POST",
                headers={"Authorization": AUTH},
            )
            assert ended[0] == 200, ended


cleanup_active_broadcast()

started = req(
    "/api/v1/creator/me/broadcasts/start",
    "POST",
    {
        "title": "Stale live read consistency validation",
        "category": "Tech",
        "tags": ["stale", "live", "read"],
        "isMature": False,
        "notifyFollowers": False,
    },
    {"Authorization": AUTH},
)
assert started[0] == 200, started
broadcast_id = started[1]["id"]

live = req("/api/v1/creator/me/live", headers={"Authorization": AUTH})
assert live[0] == 200 and live[1]["pendingBroadcast"] is not None, live
stream_key = live[1]["profile"]["streamKey"]

connected = req(
    "/api/v1/ingest/live/connect",
    "POST",
    {
        "streamKey": stream_key,
        "protocol": "rtmp",
        "ingestServer": "rtmp-us-east-1-primary",
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
        "bitrateKbps": 5200,
        "viewers": 333,
        "droppedFrames": 1,
        "cpuPercent": 29,
        "freeDiskGb": 640.0,
    },
    {"x-ingest-token": ingest_token},
)
assert heartbeat[0] == 200, heartbeat

conn = sqlite3.connect(DB)
conn.execute(
    "UPDATE live_ingest_sessions SET last_heartbeat_at = ? WHERE id = ?",
    ("2026-08-17T00:00:00+00:00", session_id),
)
conn.commit()
conn.close()

public_stream = req("/api/v1/live/streams/deepsaint-live")
assert public_stream[0] == 404, public_stream

listing = req("/api/v1/live/streams")
assert listing[0] == 200, listing
assert all(item["slug"] != "deepsaint-live" for item in listing[1]), listing

playback = req("/api/v1/playback/live/lv-deepsaint-live/session", "POST")
assert playback[0] == 404, playback

chat = req("/api/v1/live/streams/lv-deepsaint-live/chat")
assert chat[0] == 404, chat

moderation = req(
    "/api/v1/live/streams/lv-deepsaint-live/moderation/actions",
    "POST",
    {
        "subjectUserId": "usr-2",
        "actionType": "mute",
        "reason": "stale stream should not accept moderation control",
        "durationMinutes": 5,
    },
    {"Authorization": AUTH},
)
assert moderation[0] == 404, moderation

creator_live = req("/api/v1/creator/me/live", headers={"Authorization": AUTH})
assert creator_live[0] == 200, creator_live
assert creator_live[1]["currentBroadcast"] is None, creator_live
assert creator_live[1]["pendingBroadcast"] is not None, creator_live
assert creator_live[1]["profile"]["liveStatus"] == "starting", creator_live
assert creator_live[1]["ingestSession"] is None, creator_live

ended = req(
    f"/api/v1/creator/me/broadcasts/{broadcast_id}/end",
    "POST",
    headers={"Authorization": AUTH},
)
assert ended[0] == 200, ended

print("stale-live|public-hidden|creator-healed")
