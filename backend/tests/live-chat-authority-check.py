import json
import sqlite3
import time
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"
DB = "/Users/deepsaint/Desktop/lifestream/backend/lifestream.db"
HOST = "Bearer lifestream-local-dev-token"
VIEWER = "Bearer lifestream-viewer-token"
OUTSIDER = "Bearer lifestream-local-collaborator-token"


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
    request = urllib.request.Request(
        BASE + path,
        data=data,
        headers=headers,
        method=method,
    )
    try:
        with urllib.request.urlopen(request) as resp:
            raw = resp.read().decode()
            return resp.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode()
        return exc.code, json.loads(raw) if raw else None


streams = req("/api/v1/live/streams", "GET")
assert streams[0] == 200, streams
stream = next(
    (item for item in streams[1] if item["streamer"]["handle"] == "deepsaint"),
    None,
)
created_session = None
broadcast_id = None
if stream is None:
    live = req("/api/v1/creator/me/live", "GET", HOST)
    assert live[0] == 200, live
    current = live[1]["currentBroadcast"] or live[1]["pendingBroadcast"]
    if current is None:
        started = req(
            "/api/v1/creator/me/broadcasts/start",
            "POST",
            HOST,
            {
                "title": "chat authority validation",
                "category": "Tech",
                "tags": ["chat", "authority"],
                "isMature": False,
                "notifyFollowers": False,
            },
        )
        assert started[0] == 200, started
        broadcast_id = started[1]["id"]
    else:
        broadcast_id = current["id"]
    live = req("/api/v1/creator/me/live", "GET", HOST)
    assert live[0] == 200 and (
        live[1]["currentBroadcast"] is not None
        or live[1]["pendingBroadcast"] is not None
    ), live
    connected = req(
        "/api/v1/ingest/live/connect",
        "POST",
        None,
        {
            "streamKey": live[1]["profile"]["streamKey"],
            "protocol": "rtmp",
            "ingestServer": "rtmp-us-east-1",
            "broadcastId": broadcast_id,
        },
    )
    assert connected[0] == 200, connected
    created_session = connected[1]
    beat = req(
        f"/api/v1/ingest/live/{created_session['session']['id']}/heartbeat",
        "POST",
        None,
        {
            "bitrateKbps": 4200,
            "viewers": 125,
            "droppedFrames": 0,
            "cpuPercent": 31,
            "freeDiskGb": 512.0,
        },
        {"x-ingest-token": created_session["ingestToken"]},
    )
    assert beat[0] == 200, beat
    stream_id = created_session["liveStreamId"]
else:
    stream_id = stream["id"]

conn = sqlite3.connect(DB)
conn.execute(
    "DELETE FROM chat_messages WHERE stream_id = ? AND user_id IN (?, ?, ?)",
    (stream_id, "usr-viewer", "usr-1", "usr-2"),
)
conn.commit()
conn.close()

settings = req(
    "/api/v1/creator/me/live/settings",
    "PATCH",
    HOST,
    {
        "subscriberOnly": True,
        "slowModeSeconds": 3,
        "autoModLevel": "standard",
    },
)
assert (
    settings[0] == 200
    and settings[1]["subscriberOnly"] is True
    and settings[1]["slowModeSeconds"] == 3
    and settings[1]["autoModLevel"] == "standard"
), settings

outsider = req(
    f"/api/v1/live/streams/{stream_id}/chat/messages",
    "POST",
    OUTSIDER,
    {"body": "let me in"},
)
assert outsider[0] == 402, outsider

membership = req(
    "/api/v1/creator/subscriptions/crt-deepsaint/tiers/tier-2",
    "POST",
    VIEWER,
)
assert membership[0] == 200 and membership[1]["status"] == "active", membership

first = req(
    f"/api/v1/live/streams/{stream_id}/chat/messages",
    "POST",
    VIEWER,
    {"body": "subscriber hello"},
)
assert first[0] == 200 and first[1]["userHandle"] == "viewer_one", first

second = req(
    f"/api/v1/live/streams/{stream_id}/chat/messages",
    "POST",
    VIEWER,
    {"body": "too soon"},
)
assert second[0] == 400 and "slow mode" in second[1]["error"], second

time.sleep(4)

automod = req(
    f"/api/v1/live/streams/{stream_id}/chat/messages",
    "POST",
    VIEWER,
    {"body": "visit https://spam.example right now"},
)
assert automod[0] == 400 and "automod" in automod[1]["error"], automod

host_a = req(
    f"/api/v1/live/streams/{stream_id}/chat/messages",
    "POST",
    HOST,
    {"body": "https://owner.example should pass"},
)
host_b = req(
    f"/api/v1/live/streams/{stream_id}/chat/messages",
    "POST",
    HOST,
    {"body": "second owner message immediately"},
)
assert host_a[0] == 200 and host_b[0] == 200, (host_a, host_b)

if created_session is not None:
    ended = req(
        f"/api/v1/ingest/live/{created_session['session']['id']}/disconnect",
        "POST",
        None,
        None,
        {"x-ingest-token": created_session["ingestToken"]},
    )
    assert ended[0] == 200, ended

print("live-chat-authority-pass")
