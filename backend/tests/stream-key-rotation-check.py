import json
import time
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"
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


def cleanup_live():
    live = req("/api/v1/creator/me/live", headers={"Authorization": AUTH})
    assert live[0] == 200, live
    for broadcast_key in ("currentBroadcast", "pendingBroadcast"):
        broadcast = live[1].get(broadcast_key)
        if broadcast is not None:
            ended = req(
                f"/api/v1/creator/me/broadcasts/{broadcast['id']}/end",
                "POST",
                headers={"Authorization": AUTH},
            )
            assert ended[0] == 200, ended


def start_broadcast(title):
    started = req(
        "/api/v1/creator/me/broadcasts/start",
        "POST",
        {
            "title": title,
            "category": "Gaming",
            "tags": ["rotation", "ingest", "authority"],
            "isMature": False,
            "notifyFollowers": False,
        },
        HEADERS,
    )
    assert started[0] == 200, started
    return started[1]


cleanup_live()

before = req("/api/v1/creator/me/live", headers={"Authorization": AUTH})
assert before[0] == 200, before
old_key = before[1]["profile"]["streamKey"]

first_broadcast = start_broadcast(f"Rotation invalidation validation {SUFFIX}")

connected = req(
    "/api/v1/ingest/live/connect",
    "POST",
    {
        "streamKey": old_key,
        "protocol": "rtmp",
        "ingestServer": "rtmp-us-east-1-primary",
        "broadcastId": first_broadcast["id"],
    },
    {"Content-Type": "application/json"},
)
assert connected[0] == 200, connected
session_id = connected[1]["session"]["id"]
ingest_token = connected[1]["ingestToken"]

heartbeat = req(
    f"/api/v1/ingest/live/{session_id}/heartbeat",
    "POST",
    {
        "bitrateKbps": 6500,
        "viewers": 1444,
        "droppedFrames": 2,
        "cpuPercent": 41,
        "freeDiskGb": 480.0,
    },
    {"Content-Type": "application/json", "x-ingest-token": ingest_token},
)
assert heartbeat[0] == 200, heartbeat

rotated = req("/api/v1/creator/me/stream-key/rotate", "POST", headers={"Authorization": AUTH})
assert rotated[0] == 200, rotated
new_key = rotated[1]["streamKey"]
assert new_key != old_key, rotated

heartbeat_after = req(
    f"/api/v1/ingest/live/{session_id}/heartbeat",
    "POST",
    {
        "bitrateKbps": 6600,
        "viewers": 1555,
        "droppedFrames": 3,
        "cpuPercent": 43,
        "freeDiskGb": 470.0,
    },
    {"Content-Type": "application/json", "x-ingest-token": ingest_token},
)
assert heartbeat_after[0] == 401, heartbeat_after

runtime_after = req("/api/v1/creator/me/live/runtime", headers={"Authorization": AUTH})
assert runtime_after[0] == 200, runtime_after
assert runtime_after[1]["activeSession"] is None, runtime_after
assert runtime_after[1]["snapshot"]["profile"]["liveStatus"] == "offline", runtime_after
assert any(
    event["eventType"] == "stream_key_rotated" for event in runtime_after[1]["recentEvents"]
), runtime_after

second_broadcast = start_broadcast(f"Rotation reconnect validation {SUFFIX}")

old_key_connect = req(
    "/api/v1/ingest/live/connect",
    "POST",
    {
        "streamKey": old_key,
        "protocol": "rtmp",
        "ingestServer": "rtmp-us-east-1-primary",
        "broadcastId": second_broadcast["id"],
    },
    {"Content-Type": "application/json"},
)
assert old_key_connect[0] == 401, old_key_connect

new_key_connect = req(
    "/api/v1/ingest/live/connect",
    "POST",
    {
        "streamKey": new_key,
        "protocol": "rtmp",
        "ingestServer": "rtmp-us-east-1-primary",
        "broadcastId": second_broadcast["id"],
    },
    {"Content-Type": "application/json"},
)
assert new_key_connect[0] == 200, new_key_connect

terminated = req(
    f"/api/v1/creator/me/live/ingest/{new_key_connect[1]['session']['id']}/terminate",
    "POST",
    {"reason": "cleanup after stream key rotation validation"},
    HEADERS,
)
assert terminated[0] == 200, terminated

print("stream-key|rotate|invalidate|reconnect")
