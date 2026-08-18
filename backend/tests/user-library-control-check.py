import json
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"
HOST = "Bearer lifestream-local-dev-token"


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


series_id = "ser-northlight"
episode_id = "ser-northlight-s2e3"
film_catalog = req("/api/v1/catalog/films", token=HOST)
assert film_catalog[0] == 200 and len(film_catalog[1]) > 0, film_catalog
film_id = film_catalog[1][0]["id"]

assert req(f"/api/v1/me/progress/{series_id}", "DELETE", HOST)[0] == 200
assert req(f"/api/v1/me/progress/{film_id}", "DELETE", HOST)[0] == 200
assert req(f"/api/v1/me/history/{series_id}", "DELETE", HOST)[0] == 200
assert req(f"/api/v1/me/history/{film_id}", "DELETE", HOST)[0] == 200

partial_series = req(
    "/api/v1/me/progress",
    "PUT",
    HOST,
    {
        "contentId": series_id,
        "kind": "series",
        "episodeId": episode_id,
        "progressSec": 1280,
        "durationSec": 999999,
    },
)
assert partial_series[0] == 200, partial_series

completed_film = req(
    "/api/v1/me/progress",
    "PUT",
    HOST,
    {
        "contentId": film_id,
        "kind": "film",
        "progressSec": 999999,
        "durationSec": 999999,
    },
)
assert completed_film[0] == 200, completed_film

library = req("/api/v1/me/library", token=HOST)
assert library[0] == 200, library
assert "memberships" in library[1] and "purchases" in library[1], library

series_continue = next(
    item for item in library[1]["continueWatching"] if item["contentId"] == series_id
)
assert (
    series_continue["episodeId"] == episode_id
    and series_continue["progressSec"] == 1280
), series_continue

series_history = next(item for item in library[1]["history"] if item["contentId"] == series_id)
assert (
    series_history["episodeId"] == episode_id
    and series_history["completed"] is False
    and series_history["completedAt"] is None
), series_history

film_history = next(item for item in library[1]["history"] if item["contentId"] == film_id)
assert film_history["completed"] is True and film_history["completedAt"] is not None, film_history
assert all(
    item["contentId"] != film_id for item in library[1]["continueWatching"]
), library

removed = req(f"/api/v1/me/history/{film_id}", "DELETE", HOST)
assert removed[0] == 200, removed
assert all(item["contentId"] != film_id for item in removed[1]["history"]), removed

print("user-library|history|complete-retained")
