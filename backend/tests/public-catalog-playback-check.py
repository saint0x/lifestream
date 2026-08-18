import json
import urllib.request

BASE = "http://127.0.0.1:8080"
AUTH = "Bearer lifestream-local-dev-token"


def req(path, method="GET"):
    request = urllib.request.Request(
        BASE + path,
        headers={"Authorization": AUTH, "Accept": "application/json"},
        method=method,
    )
    with urllib.request.urlopen(request) as response:
        raw = response.read().decode()
        return response.status, json.loads(raw) if raw else None


films = req("/api/v1/catalog/films")
assert films[0] == 200 and len(films[1]) >= 1, films
film = films[1][0]
assert film["playbackReady"] is True, film
assert film["playbackSessionUrl"] == f"/api/v1/playback/content/{film['id']}/session", film

film_grant = req(film["playbackSessionUrl"], "POST")
assert film_grant[0] == 200, film_grant
assert film_grant[1]["session"]["contentId"] == film["id"], film_grant
film_manifest = urllib.request.urlopen(BASE + film_grant[1]["manifestUrl"])
film_body = film_manifest.read().decode()
film_segment = next(line for line in film_body.splitlines() if line and not line.startswith("#"))
assert "playbackToken=" in film_segment, film_segment
assert urllib.request.urlopen(BASE + film_segment).status == 200

series = req("/api/v1/catalog/series/northlight")
assert series[0] == 200 and len(series[1]["seasons"]) >= 1, series
episode = series[1]["seasons"][0]["episodes"][0]
assert episode["playbackReady"] is True, episode
assert episode["playbackSessionUrl"] == f"/api/v1/playback/content/{episode['id']}/session", episode

episode_grant = req(episode["playbackSessionUrl"], "POST")
assert episode_grant[0] == 200, episode_grant
assert episode_grant[1]["session"]["contentId"] == episode["id"], episode_grant
episode_manifest = urllib.request.urlopen(BASE + episode_grant[1]["manifestUrl"])
episode_body = episode_manifest.read().decode()
episode_segment = next(line for line in episode_body.splitlines() if line and not line.startswith("#"))
assert "playbackToken=" in episode_segment, episode_segment
assert urllib.request.urlopen(BASE + episode_segment).status == 200

print("public-catalog-playback|film|episode")
