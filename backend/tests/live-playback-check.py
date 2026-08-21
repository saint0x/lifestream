import json
import hashlib
import sqlite3
import urllib.request
import urllib.error

BASE = "http://127.0.0.1:8080"
DB = "/Users/deepsaint/Desktop/lifestream/backend/lifestream.db"
OWNER = "Bearer lifestream-local-dev-token"


def get_json(path, method="GET", token=None, body=None, extra_headers=None):
    headers = {"Accept": "application/json"}
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
        with urllib.request.urlopen(request) as response:
            raw = response.read().decode()
            return response.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode()
        return exc.code, json.loads(raw) if raw else None


def ensure_owner_session():
    conn = sqlite3.connect(DB)
    now = "2026-08-21T00:00:00Z"
    conn.execute(
        """
        INSERT OR REPLACE INTO auth_sessions (
            id, user_id, label, token_hash, scopes_json, created_at, expires_at, revoked_at, last_used_at
        ) VALUES (?, ?, ?, ?, ?, ?, NULL, NULL, NULL)
        """,
        (
            "sess-live-playback-owner",
            "usr-1",
            "live-playback-owner",
            hashlib.sha256("lifestream-local-dev-token".encode()).hexdigest(),
            json.dumps(["user", "creator", "creator:write", "admin"]),
            now,
        ),
    )
    conn.commit()
    conn.close()


def ensure_live_stream():
    streams = get_json("/api/v1/live/streams")
    assert streams[0] == 200, streams
    if streams[1]:
        return streams[1][0], None

    live = get_json("/api/v1/creator/me/live", token=OWNER)
    assert live[0] == 200, live
    current = live[1]["currentBroadcast"] or live[1]["pendingBroadcast"]
    if current is None:
        started = get_json(
            "/api/v1/creator/me/broadcasts/start",
            "POST",
            OWNER,
            {
                "title": "live playback validation",
                "category": "Tech",
                "tags": ["playback", "live", "validation"],
                "isMature": False,
                "notifyFollowers": False,
            },
        )
        assert started[0] == 200, started
        broadcast_id = started[1]["id"]
    else:
        broadcast_id = current["id"]

    refreshed = get_json("/api/v1/creator/me/live", token=OWNER)
    assert refreshed[0] == 200, refreshed
    connected = get_json(
        "/api/v1/ingest/live/connect",
        "POST",
        None,
        {
            "streamKey": refreshed[1]["profile"]["streamKey"],
            "protocol": "rtmp",
            "ingestServer": "rtmp-us-east-1-primary",
            "broadcastId": broadcast_id,
        },
    )
    assert connected[0] == 200, connected
    heartbeat = get_json(
        f"/api/v1/ingest/live/{connected[1]['session']['id']}/heartbeat",
        "POST",
        None,
        {
            "bitrateKbps": 5400,
            "viewers": 2115,
            "droppedFrames": 0,
            "cpuPercent": 31,
            "freeDiskGb": 512.0,
            "sourceProbe": {
                "containerFormat": "mpegts",
                "videoCodec": "h264",
                "audioCodec": "aac",
                "width": 1920,
                "height": 1080,
                "frameRate": 59.94,
                "audioSampleRateHz": 48000,
                "audioChannels": 2,
            },
        },
        {"x-ingest-token": connected[1]["ingestToken"]},
    )
    assert heartbeat[0] == 200, heartbeat
    runtime = get_json(
        f"/api/v1/ingest/live/{connected[1]['session']['id']}/runtime",
        "POST",
        None,
        {
            "runtimeState": "healthy",
            "packagingStatus": "ready",
            "archiveStatus": "not_started",
            "manifestRelativePath": (
                f"live/{refreshed[1]['profile']['id']}/{broadcast_id}/{connected[1]['session']['id']}/master.m3u8"
            ),
            "archiveRelativePath": None,
            "lastError": None,
        },
        {"x-ingest-token": connected[1]["ingestToken"]},
    )
    assert runtime[0] == 200, runtime

    streams = get_json("/api/v1/live/streams")
    assert streams[0] == 200 and streams[1], streams
    return streams[1][0], connected[1]


ensure_owner_session()
stream, created_session = ensure_live_stream()
assert stream["playbackReady"] is True, stream
assert stream["playbackSessionUrl"] == f"/api/v1/playback/live/{stream['id']}/session", stream

detail = get_json(f"/api/v1/live/streams/{stream['slug']}")
assert detail[0] == 200, detail
assert detail[1]["id"] == stream["id"], detail
assert detail[1]["playbackReady"] is True, detail
assert detail[1]["playbackSessionUrl"] == stream["playbackSessionUrl"], detail

grant = get_json(stream["playbackSessionUrl"], "POST")
assert grant[0] == 200, grant
assert grant[1]["session"]["contentId"] == stream["id"], grant
assert grant[1]["session"]["contentKind"] == "live", grant

with urllib.request.urlopen(BASE + grant[1]["manifestUrl"]) as manifest_response:
    manifest_body = manifest_response.read().decode()

playlist_url = next(
    line for line in manifest_body.splitlines() if line and not line.startswith("#")
)
assert "playbackToken=" in playlist_url, playlist_url

with urllib.request.urlopen(BASE + playlist_url) as playlist_response:
    playlist_body = playlist_response.read().decode()

segment_url = next(
    line for line in playlist_body.splitlines() if line and not line.startswith("#")
)
assert "playbackToken=" in segment_url, segment_url

with urllib.request.urlopen(BASE + segment_url) as segment_response:
    segment_body = segment_response.read()
    assert segment_response.status == 200, segment_response.status
    assert len(segment_body) > 0, len(segment_body)
    assert segment_response.headers["Content-Type"] == "video/mp2t"

if created_session is not None:
    ended = get_json(
        f"/api/v1/ingest/live/{created_session['session']['id']}/disconnect",
        "POST",
        None,
        None,
        {"x-ingest-token": created_session["ingestToken"]},
    )
    assert ended[0] == 200, ended

print("live-playback|stream|manifest|segment")
