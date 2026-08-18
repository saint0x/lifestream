import json
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"


def req(path, method="GET", token=None, body=None):
    headers = {}
    if token:
        headers["Authorization"] = token
    data = None
    if body is not None:
        headers["Content-Type"] = "application/json"
        data = json.dumps(body).encode()
    request = urllib.request.Request(BASE + path, headers=headers, data=data, method=method)
    try:
        with urllib.request.urlopen(request) as response:
            raw = response.read().decode()
            return response.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode()
        return exc.code, json.loads(raw) if raw else None


films = req("/api/v1/catalog/creator/films")
assert films[0] == 200 and len(films[1]) >= 1, films
film = films[1][0]
assert film["playbackReady"] is True, film
assert film["playbackSessionUrl"] == f"/api/v1/playback/content/{film['id']}/session", film

film_grant = req(film["playbackSessionUrl"], "POST")
assert film_grant[0] == 200, film_grant
assert film_grant[1]["session"]["contentId"] == film["id"], film_grant
assert film_grant[1]["manifestUrl"].startswith("/api/v1/playback/sessions/"), film_grant

film_manifest = urllib.request.urlopen(BASE + film_grant[1]["manifestUrl"])
film_body = film_manifest.read().decode()
film_segment = next(line for line in film_body.splitlines() if line and not line.startswith("#"))
assert "playbackToken=" in film_segment, film_segment
assert urllib.request.urlopen(BASE + film_segment).status == 200

series = req("/api/v1/catalog/creator/series/northlight-studio")
assert series[0] == 200 and len(series[1]["seasons"]) >= 1, series
episode = series[1]["seasons"][0]["episodes"][0]
assert episode["playbackReady"] is False, episode
assert episode["playbackSessionUrl"] is None, episode

print("creator-catalog-playback|film|episode")
