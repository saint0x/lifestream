import json
import urllib.request

BASE = "http://127.0.0.1:8080"


def get_json(path, method="GET"):
    request = urllib.request.Request(
        BASE + path,
        headers={"Accept": "application/json"},
        method=method,
    )
    with urllib.request.urlopen(request) as response:
        raw = response.read().decode()
        return response.status, json.loads(raw) if raw else None


streams = get_json("/api/v1/live/streams")
assert streams[0] == 200 and len(streams[1]) >= 1, streams
stream = next(item for item in streams[1] if item["slug"] == "gridline-endurance")
assert stream["playbackReady"] is True, stream
assert stream["playbackSessionUrl"] == f"/api/v1/playback/live/{stream['id']}/session", stream

detail = get_json("/api/v1/live/streams/gridline-endurance")
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

segment_url = next(
    line for line in manifest_body.splitlines() if line and not line.startswith("#")
)
assert "playbackToken=" in segment_url, segment_url

with urllib.request.urlopen(BASE + segment_url) as segment_response:
    segment_body = segment_response.read()
    assert segment_response.status == 200, segment_response.status
    assert len(segment_body) > 0, len(segment_body)
    assert segment_response.headers["Content-Type"] == "video/mp2t"

print("live-playback|stream|manifest|segment")
