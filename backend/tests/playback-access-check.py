import json
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"
AUTH = "Bearer vanta-local-dev-token"
UPLOAD_ID = "upl-0228ae47576448f78a3cdaec06a8465b"
MANIFEST_PATH = "processed/crt-deepsaint/7b3542fb-6783-4cc7-9a20-bb33014c4645/hls/master.m3u8"

creator = urllib.request.Request(
    BASE + "/api/v1/media/" + MANIFEST_PATH,
    headers={"Authorization": AUTH},
)
assert urllib.request.urlopen(creator).status == 200

try:
    urllib.request.urlopen(BASE + "/api/v1/media/" + MANIFEST_PATH)
    blocked = 200
except urllib.error.HTTPError as exc:
    blocked = exc.code

grant = json.load(
    urllib.request.urlopen(
        urllib.request.Request(
            BASE + f"/api/v1/playback/uploads/{UPLOAD_ID}/session",
            method="POST",
        )
    )
)
manifest = urllib.request.urlopen(BASE + grant["manifestUrl"])
body = manifest.read().decode()
segment = next(line for line in body.splitlines() if line and not line.startswith("#"))
segment_status = urllib.request.urlopen(BASE + segment).status
print(
    f"{blocked}|{grant['visibility']}|{manifest.status}|"
    f"{str('playbackToken=' in body).lower()}|{segment_status}"
)
