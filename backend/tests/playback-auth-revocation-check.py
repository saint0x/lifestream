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
        headers["Content-Type"] = "application/json"
        data = json.dumps(body).encode()
    request = urllib.request.Request(BASE + path, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(request) as resp:
            raw = resp.read().decode()
            return resp.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode()
        return exc.code, json.loads(raw) if raw else None


status, created = req(
    "/api/v1/me/sessions",
    method="POST",
    token=HOST,
    body={"label": "playback-auth-revoke", "scopes": ["user"], "expiresInDays": 1},
)
assert status == 200, created

ephemeral = "Bearer " + created["accessToken"]
session_id = created["session"]["id"]

status, grant = req("/api/v1/playback/content/flm-afterglow/session", method="POST", token=ephemeral)
assert status == 200, grant
manifest_url = grant["manifestUrl"]

with urllib.request.urlopen(BASE + manifest_url) as resp:
    assert resp.status == 200

status, _ = req(f"/api/v1/me/sessions/{session_id}", method="DELETE", token=HOST)
assert status == 204, status

denied = 0
try:
    urllib.request.urlopen(BASE + manifest_url)
except urllib.error.HTTPError as exc:
    denied = exc.code

assert denied == 401, denied
print("playback-auth-revocation|grant|revoked")
