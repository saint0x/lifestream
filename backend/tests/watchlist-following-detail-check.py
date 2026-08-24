import json
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"
HOST = "Bearer vanta-local-dev-token"


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
films = req("/api/v1/catalog/films", token=HOST)
assert films[0] == 200 and len(films[1]) > 0, films
film_id = films[1][0]["id"]

streamers = req("/api/v1/streamers")
assert streamers[0] == 200 and len(streamers[1]) > 0, streamers
streamer_id = streamers[1][0]["id"]

assert req(f"/api/v1/me/watchlist/{series_id}", "POST", HOST)[0] == 200
assert req(f"/api/v1/me/watchlist/{film_id}", "POST", HOST)[0] == 200
assert req(f"/api/v1/me/following/{streamer_id}", "POST", HOST)[0] == 200

watchlist = req("/api/v1/me/watchlist", token=HOST)
assert watchlist[0] == 200, watchlist
assert watchlist[1]["totalTitles"] == len(watchlist[1]["series"]) + len(watchlist[1]["films"]), watchlist
assert any(item["id"] == series_id for item in watchlist[1]["series"]), watchlist
assert any(item["id"] == film_id for item in watchlist[1]["films"]), watchlist

following = req("/api/v1/me/following", token=HOST)
assert following[0] == 200, following
assert following[1]["totalFollowedStreamers"] == len(following[1]["followedStreamers"]), following
assert following[1]["liveNowCount"] == len(following[1]["liveStreams"]), following
followed_ids = [item["id"] for item in following[1]["followedStreamers"]]
assert streamer_id in followed_ids, following
for stream in following[1]["liveStreams"]:
    assert stream["streamer"]["id"] in followed_ids, stream

print("watchlist-following|detail|consistent")
