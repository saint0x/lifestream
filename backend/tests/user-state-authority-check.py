import hashlib
import json
import sqlite3
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"
DB = "/Users/deepsaint/Desktop/vanta/backend/vanta.db"
HOST = "Bearer vanta-local-dev-token"


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
            "sess-user-state-authority-owner",
            "usr-1",
            "user-state-authority-owner",
            hashlib.sha256("vanta-local-dev-token".encode()).hexdigest(),
            json.dumps(["user", "creator", "creator:write", "admin"]),
            now,
        ),
    )
    conn.commit()
    conn.close()


def ensure_live_stream():
    streams = req("/api/v1/live/streams", token=HOST)
    assert streams[0] == 200, streams
    existing = next(
        (item for item in streams[1] if item["streamer"]["handle"] == "deepsaint"),
        None,
    )
    if existing is not None:
        return existing["id"], None

    live = req("/api/v1/creator/me/live", token=HOST)
    assert live[0] == 200, live
    current = live[1]["currentBroadcast"] or live[1]["pendingBroadcast"]
    if current is None:
        started = req(
            "/api/v1/creator/me/broadcasts/start",
            "POST",
            HOST,
            {
                "title": "user state authority validation",
                "category": "Tech",
                "tags": ["user-state", "authority", "validation"],
                "isMature": False,
                "notifyFollowers": False,
            },
        )
        assert started[0] == 200, started
        broadcast_id = started[1]["id"]
    else:
        broadcast_id = current["id"]

    refreshed = req("/api/v1/creator/me/live", token=HOST)
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
            "bitrateKbps": 4200,
            "viewers": 144,
            "droppedFrames": 0,
            "cpuPercent": 22,
            "freeDiskGb": 512.0,
        },
        {"x-ingest-token": connected[1]["ingestToken"]},
    )
    assert heartbeat[0] == 200, heartbeat
    return connected[1]["liveStreamId"], connected[1]


ensure_owner_session()
live_stream_id, created_session = ensure_live_stream()
cleanup_film = req("/api/v1/me/progress/film-afterglow", "DELETE", HOST)
assert cleanup_film[0] == 200, cleanup_film
cleanup_series = req("/api/v1/me/progress/ser-northlight", "DELETE", HOST)
assert cleanup_series[0] == 200, cleanup_series

streams = req("/api/v1/live/streams", token=HOST)
assert streams[0] == 200 and len(streams[1]) > 0, streams
assert any(item["id"] == live_stream_id for item in streams[1]), streams

series_catalog = req("/api/v1/catalog/series", token=HOST)
assert series_catalog[0] == 200 and len(series_catalog[1]) >= 2, series_catalog
northlight = next(item for item in series_catalog[1] if item["id"] == "ser-northlight")
northlight_episode = next(
    episode
    for season in northlight["seasons"]
    for episode in season["episodes"]
    if episode["id"] == "ser-northlight-s2e3"
)
foreign_series = next(item for item in series_catalog[1] if item["id"] != "ser-northlight")
foreign_episode_id = foreign_series["seasons"][0]["episodes"][0]["id"]
films = req("/api/v1/catalog/films", token=HOST)
assert films[0] == 200 and len(films[1]) > 0, films
film_id = films[1][0]["id"]

bad_watchlist = req(f"/api/v1/me/watchlist/{live_stream_id}", "POST", HOST)
assert (
    bad_watchlist[0] == 400 and "watchlist only supports series and films" in bad_watchlist[1]["error"]
), bad_watchlist

bad_following = req("/api/v1/me/following/streamer-does-not-exist", "POST", HOST)
assert bad_following[0] == 404, bad_following

film_with_episode = req(
    "/api/v1/me/progress",
    "PUT",
    HOST,
    {
        "contentId": film_id,
        "kind": "film",
        "episodeId": "ser-northlight-s2e3",
        "progressSec": 12,
        "durationSec": 1,
    },
)
assert (
    film_with_episode[0] == 400
    and "film progress cannot include an episodeId" in film_with_episode[1]["error"]
), film_with_episode

series_without_episode = req(
    "/api/v1/me/progress",
    "PUT",
    HOST,
    {
        "contentId": "ser-northlight",
        "kind": "series",
        "progressSec": 12,
        "durationSec": 1,
    },
)
assert (
    series_without_episode[0] == 400
    and "series progress requires an episodeId" in series_without_episode[1]["error"]
), series_without_episode

mismatched_episode = req(
    "/api/v1/me/progress",
    "PUT",
    HOST,
    {
        "contentId": "ser-northlight",
        "kind": "series",
        "episodeId": foreign_episode_id,
        "progressSec": 12,
        "durationSec": 1,
    },
)
assert (
    mismatched_episode[0] == 400
    and "episodeId does not belong to the requested series" in mismatched_episode[1]["error"]
), mismatched_episode

completed_film = req(
    "/api/v1/me/progress",
    "PUT",
    HOST,
    {
        "contentId": film_id,
        "kind": "film",
        "progressSec": 999999,
        "durationSec": 1,
    },
)
assert completed_film[0] == 200, completed_film
assert all(
    entry["contentId"] != film_id
    for entry in completed_film[1]["continueWatching"]
), completed_film

series_progress = req(
    "/api/v1/me/progress",
    "PUT",
    HOST,
    {
        "contentId": "ser-northlight",
        "kind": "series",
        "episodeId": "ser-northlight-s2e3",
        "progressSec": 1280,
        "durationSec": 1,
    },
)
assert series_progress[0] == 200, series_progress
series_entry = next(
    item for item in series_progress[1]["continueWatching"] if item["contentId"] == "ser-northlight"
)
assert (
    series_entry["episodeId"] == "ser-northlight-s2e3"
    and series_entry["progressSec"] == 1280
    and series_entry["durationSec"] == northlight_episode["durationSec"]
), series_entry

if created_session is not None:
    ended = req(
        f"/api/v1/ingest/live/{created_session['session']['id']}/disconnect",
        "POST",
        None,
        None,
        {"x-ingest-token": created_session["ingestToken"]},
    )
    assert ended[0] == 200, ended

print("user-state-authority-pass")
