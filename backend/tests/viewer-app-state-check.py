import json
import urllib.request

BASE = "http://127.0.0.1:8080"
HOST = "Bearer lifestream-local-dev-token"


def req(path, token=None):
    headers = {}
    if token:
        headers["Authorization"] = token
    request = urllib.request.Request(BASE + path, headers=headers)
    with urllib.request.urlopen(request) as response:
        raw = response.read().decode()
        return response.status, json.loads(raw) if raw else None


state = req("/api/v1/me/state", token=HOST)
assert state[0] == 200, state
assert state[1]["user"]["handle"] == "deepsaint", state
assert "library" in state[1] and "watchlist" in state[1] and "following" in state[1], state
assert "profile" in state[1] and "settings" in state[1] and "plan" in state[1], state
assert "notifications" in state[1] and "sessions" in state[1], state
assert state[1]["plan"]["planName"] == "LIFESTREAM Premium", state
assert state[1]["following"]["totalFollowedStreamers"] == len(state[1]["following"]["followedStreamers"]), state
assert state[1]["watchlist"]["totalTitles"] == len(state[1]["watchlist"]["series"]) + len(state[1]["watchlist"]["films"]), state
assert any(session["isCurrent"] for session in state[1]["sessions"]), state

bootstrap = req("/api/v1/bootstrap", token=HOST)
assert bootstrap[0] == 200, bootstrap
assert bootstrap[1]["me"]["handle"] == state[1]["user"]["handle"], bootstrap
assert bootstrap[1]["viewer"] is None, bootstrap

print("viewer-app-state|bootstrap|consistent")
